//! MCP Server 模块（设计哲学 §11.11）
//!
//! 完整 MCP 协议实现：JSON-RPC 2.0 over HTTP SSE
//! 对外暴露 Casy 数据的只读接口，供外部 AI 工具查询。
//! 写操作不绕过确认：外部通道的写工具调用进入 mcp_pending_writes 待确认队列，
//! 由用户在应用内确认（approve_mcp_write）后才真正执行（视为 L3 确认完成）。
//!
//! 协议参考：https://spec.modelcontextprotocol.io/2025-03-26/

pub mod server;

use serde::{Deserialize, Serialize};

/// 写操作工具：MCP 外部通道下一律进入待确认队列，不直接执行
const WRITE_TOOLS: &[&str] = &["case_create_task", "task_update_status"];

// ═══════════════════════════════════════════════════════════
// MCP 协议数据结构
// ═══════════════════════════════════════════════════════════

/// JSON-RPC 2.0 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// JSON-RPC 2.0 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP Initialize 请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: McpClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: McpClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientCapabilities {
    #[serde(default)]
    pub tools: Option<serde_json::Value>,
    #[serde(default)]
    pub resources: Option<serde_json::Value>,
    #[serde(default)]
    pub prompts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

/// MCP Initialize 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: McpServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: McpServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerCapabilities {
    pub tools: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

/// MCP Tool 定义（标准格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// MCP Tool Call 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallParams {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// MCP Tool Call 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// MCP Resource 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// MCP Resource 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
}

// ═══════════════════════════════════════════════════════════
// MCP Server 实现
// ═══════════════════════════════════════════════════════════

/// MCP Server 状态
pub struct McpServer {
    pub initialized: bool,
    pub server_info: McpServerInfo,
    pub tools: Vec<McpToolDefinition>,
    pub resources: Vec<McpResource>,
    /// 是否允许写操作工具（HTTP server 通道下为 false）
    allow_write: bool,
    /// 只读模式下写工具的待确认队列提交器（测试注入替身；None = 写入数据库队列）
    write_submitter: Option<std::sync::Arc<dyn Fn(&str, serde_json::Value) -> Result<String, String> + Send + Sync>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            initialized: false,
            server_info: McpServerInfo {
                name: "casy".to_string(),
                version: "0.1.0".to_string(),
            },
            tools: Self::build_tools(),
            resources: Self::build_resources(),
            allow_write: true,
            write_submitter: None,
        }
    }

    /// 只读模式：供外部 HTTP server 使用，写操作工具进入待确认队列
    pub fn new_readonly() -> Self {
        let mut server = Self::new();
        server.allow_write = false;
        server
    }

    /// 测试用：注入写队列替身，避免触碰真实数据库
    #[cfg(test)]
    pub fn new_readonly_with_submitter(
        submitter: impl Fn(&str, serde_json::Value) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        let mut server = Self::new_readonly();
        server.write_submitter = Some(std::sync::Arc::new(submitter));
        server
    }

    /// 构建工具列表
    fn build_tools() -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "case_query".to_string(),
                description: "查询案件列表或详情。支持按轨道、状态、客户、关键词筛选。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "案件ID（可选，不传则返回列表）" },
                        "filter": {
                            "type": "object",
                            "properties": {
                                "track": { "type": "string", "enum": ["patent_invalidation", "civil_tort", "admin_litigation", "other"] },
                                "status": { "type": "string", "enum": ["进行中", "等待中", "已结案"] },
                                "client": { "type": "string" },
                                "search": { "type": "string" }
                            }
                        }
                    }
                }),
            },
            McpToolDefinition {
                name: "task_query".to_string(),
                description: "查询任务列表，支持按 GTD 透视（收件箱/今天/计划中/随时/等待/回顾/某天）过滤。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "perspective": {
                            "type": "string",
                            "enum": ["inbox", "today", "upcoming", "next", "waiting", "review", "someday"],
                            "description": "GTD 透视"
                        },
                        "case_id": { "type": "string", "description": "按案件过滤" },
                        "completed": { "type": "boolean", "description": "是否包含已完成" }
                    }
                }),
            },
            McpToolDefinition {
                name: "knowledge_search".to_string(),
                description: "搜索知识库，支持 FTS 全文检索 + 语义向量混合检索，可按 6 职能分类过滤。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "keyword": { "type": "string", "description": "搜索关键词" },
                        "category": {
                            "type": "string",
                            "enum": ["inspiration", "method", "reference", "question", "experience", "log"],
                            "description": "职能分类过滤"
                        },
                        "limit": { "type": "integer", "default": 10 }
                    },
                    "required": ["keyword"]
                }),
            },
            McpToolDefinition {
                name: "calendar_events".to_string(),
                description: "查询日历事件（开庭/口审/期限/会议/任务到期）。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "year": { "type": "integer" },
                        "month": { "type": "integer" },
                        "type": {
                            "type": "string",
                            "enum": ["hearing", "deadline", "meeting", "task"],
                            "description": "事件类型过滤"
                        }
                    },
                    "required": ["year", "month"]
                }),
            },
            McpToolDefinition {
                name: "deadline_warnings".to_string(),
                description: "获取期限预警列表，含 R1-R4 分级（温和/明确/强提醒/逾期）。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpToolDefinition {
                name: "dashboard_stats".to_string(),
                description: "获取首页仪表盘统计数据（活跃案件/等待中/已结案/逾期）。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpToolDefinition {
                name: "case_create_task".to_string(),
                description: "为指定案件创建任务（需确认）。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "case_id": { "type": "string", "description": "案件ID" },
                        "task_name": { "type": "string", "description": "任务名称" },
                        "priority": { "type": "string", "enum": ["urgent_important", "urgent", "important", "normal"] },
                        "due_date": { "type": "string", "description": "截止日期 YYYY-MM-DD" }
                    },
                    "required": ["case_id", "task_name"]
                }),
            },
            McpToolDefinition {
                name: "task_update_status".to_string(),
                description: "更新任务状态（完成/标记等待/移至今日）。".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task_id": { "type": "string", "description": "任务ID" },
                        "action": { "type": "string", "enum": ["complete", "waiting", "today"], "description": "操作类型" }
                    },
                    "required": ["task_id", "action"]
                }),
            },
        ]
    }

    /// 构建资源列表
    fn build_resources() -> Vec<McpResource> {
        vec![
            McpResource {
                uri: "casy://cases/all".to_string(),
                name: "所有案件".to_string(),
                description: "所有进行中的案件列表".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            McpResource {
                uri: "casy://tasks/inbox".to_string(),
                name: "收件箱任务".to_string(),
                description: "待厘清的任务".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            McpResource {
                uri: "casy://tasks/today".to_string(),
                name: "今日任务".to_string(),
                description: "今日聚焦任务".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            McpResource {
                uri: "casy://knowledge/all".to_string(),
                name: "知识库".to_string(),
                description: "全部知识条目".to_string(),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }

    /// 处理 MCP 请求
    pub async fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        match request.method.as_str() {
            "initialize" => self.handle_initialize(id, request.params).await,
            "notifications/initialized" => {
                // 客端确认初始化完成，无需响应
                self.initialized = true;
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: None,
                }
            }
            "tools/list" => self.handle_tools_list(id).await,
            "tools/call" => self.handle_tools_call(id, request.params).await,
            "resources/list" => self.handle_resources_list(id).await,
            "resources/read" => self.handle_resources_read(id, request.params).await,
            "ping" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({})),
                error: None,
            },
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            },
        }
    }

    /// initialize
    async fn handle_initialize(&mut self, id: Option<serde_json::Value>, _params: serde_json::Value) -> JsonRpcResponse {
        let result = McpInitializeResult {
            protocol_version: "2025-03-26".to_string(),
            capabilities: McpServerCapabilities {
                tools: serde_json::json!({}),
                resources: Some(serde_json::json!({})),
                prompts: None,
            },
            server_info: self.server_info.clone(),
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap_or_default()),
            error: None,
        }
    }

    /// tools/list
    async fn handle_tools_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({ "tools": self.tools })),
            error: None,
        }
    }

    /// tools/call
    async fn handle_tools_call(&self, id: Option<serde_json::Value>, params: serde_json::Value) -> JsonRpcResponse {
        let call: McpToolCallParams = match serde_json::from_value(params) {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: format!("Invalid params: {}", e),
                        data: None,
                    }),
                };
            }
        };

        // 写操作安全（§11.11）：MCP 外部通道无确认链路，写工具不直接执行，
        // 进入 mcp_pending_writes 待确认队列，返回正常响应（已提交，待应用内确认）
        if !self.allow_write && WRITE_TOOLS.contains(&call.name.as_str()) {
            let submit_result = if let Some(submitter) = &self.write_submitter {
                submitter(&call.name, call.arguments.clone())
            } else {
                let tool = call.name.clone();
                let arguments = call.arguments.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
                    submit_pending_write(&conn, &tool, &arguments).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            };

            return match submit_result {
                Ok(write_id) => {
                    let text = serde_json::to_string_pretty(&serde_json::json!({
                        "status": "pending_confirmation",
                        "write_id": write_id,
                    }))
                    .unwrap_or_default();
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::to_value(McpToolCallResult {
                            content: vec![McpContent {
                                content_type: "text".to_string(),
                                text,
                            }],
                            is_error: None,
                        }).unwrap_or_default()),
                        error: None,
                    }
                }
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: format!("写入待确认队列失败: {}", e),
                        data: None,
                    }),
                },
            };
        }

        let result = execute_tool_by_name(&call.name, call.arguments).await;

        match result {
            Ok(data) => {
                let text = serde_json::to_string_pretty(&data).unwrap_or_default();
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::to_value(McpToolCallResult {
                        content: vec![McpContent {
                            content_type: "text".to_string(),
                            text,
                        }],
                        is_error: None,
                    }).unwrap_or_default()),
                    error: None,
                }
            }
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::to_value(McpToolCallResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: format!("错误: {}", e),
                    }],
                    is_error: Some(true),
                }).unwrap_or_default()),
                error: None,
            },
        }
    }

    /// resources/list
    async fn handle_resources_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({ "resources": self.resources })),
            error: None,
        }
    }

    /// resources/read
    async fn handle_resources_read(&self, id: Option<serde_json::Value>, params: serde_json::Value) -> JsonRpcResponse {
        let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");

        let content = match uri {
            "casy://cases/all" => {
                match crate::commands::cases::list_cases(
                    crate::db::cases::CaseFilter {
                        page: Some(1),
                        per_page: Some(100),
                        ..Default::default()
                    },
                ).await {
                    Ok(data) => serde_json::to_string_pretty(&data).unwrap_or_default(),
                    Err(e) => format!("错误: {}", e),
                }
            }
            "casy://tasks/inbox" => {
                match crate::commands::tasks::list_tasks(
                    Some(crate::commands::tasks::TaskFilter {
                        completed: Some(false),
                        case_id: None,
                        area_id: None,
                        task_type: None,
                        start_bucket: None,
                    })
                ).await {
                    Ok(data) => serde_json::to_string_pretty(&data).unwrap_or_default(),
                    Err(e) => format!("错误: {}", e),
                }
            }
            "casy://tasks/today" => {
                match crate::commands::tasks::list_tasks(
                    Some(crate::commands::tasks::TaskFilter {
                        completed: Some(false),
                        case_id: None,
                        area_id: None,
                        task_type: None,
                        start_bucket: Some("today".to_string()),
                    })
                ).await {
                    Ok(data) => serde_json::to_string_pretty(&data).unwrap_or_default(),
                    Err(e) => format!("错误: {}", e),
                }
            }
            "casy://knowledge/all" => {
                match crate::commands::knowledge::list_knowledge(None).await {
                    Ok(data) => serde_json::to_string_pretty(&data).unwrap_or_default(),
                    Err(e) => format!("错误: {}", e),
                }
            }
            _ => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: format!("Unknown resource: {}", uri),
                        data: None,
                    }),
                };
            }
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": content
                }]
            })),
            error: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// 工具执行（复用现有命令）
// ═══════════════════════════════════════════════════════════

async fn execute_tool_by_name(name: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    match name {
        "case_query" => execute_case_query(args).await,
        "task_query" => execute_task_query(args).await,
        "knowledge_search" => execute_knowledge_search(args).await,
        "calendar_events" => execute_calendar_events(args).await,
        "deadline_warnings" => execute_deadline_warnings().await,
        "dashboard_stats" => execute_dashboard_stats().await,
        "case_create_task" => execute_case_create_task(args).await,
        "task_update_status" => execute_task_update_status(args).await,
        _ => Err(format!("未知工具: {}", name)),
    }
}

async fn execute_case_query(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let id = args.get("id").and_then(|v| v.as_str());
    if let Some(case_id) = id {
        let case = crate::commands::cases::get_case(case_id.to_string()).await?;
        serde_json::to_value(case).map_err(|e| e.to_string())
    } else {
        let filter_value = args.get("filter").cloned().unwrap_or(serde_json::json!({}));
        let filter: crate::db::cases::CaseFilter =
            serde_json::from_value(filter_value).unwrap_or_default();
        let result = crate::commands::cases::list_cases(filter).await?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    }
}

async fn execute_task_query(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let perspective = args.get("perspective").and_then(|v| v.as_str()).unwrap_or("next");
    let filter = crate::commands::tasks::TaskFilter {
        completed: Some(args.get("completed").and_then(|v| v.as_bool()).unwrap_or(false)),
        case_id: args.get("case_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        area_id: None,
        task_type: None,
        start_bucket: match perspective {
            "today" => Some("today".to_string()),
            "someday" => Some("someday".to_string()),
            _ => None,
        },
    };
    let result = crate::commands::tasks::list_tasks(Some(filter)).await?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

async fn execute_knowledge_search(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let keyword = args.get("keyword").and_then(|v| v.as_str()).ok_or("缺少 keyword")?;
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10).max(0) as usize;
    let filter = crate::commands::knowledge::KnowledgeFilter {
        category: args.get("category").and_then(|v| v.as_str()).map(|s| s.to_string()),
        case_id: None,
        search: Some(keyword.to_string()),
        law_name: None,
    };
    let mut result = crate::commands::knowledge::list_knowledge(Some(filter)).await?;
    result.truncate(limit);
    serde_json::to_value(result).map_err(|e| e.to_string())
}

async fn execute_calendar_events(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let year = args.get("year").and_then(|v| v.as_i64()).ok_or("缺少 year")? as i32;
    let month = args.get("month").and_then(|v| v.as_i64()).ok_or("缺少 month")? as u32;
    let events = crate::commands::calendar::get_calendar_events(year, month).await?;
    serde_json::to_value(events).map_err(|e| e.to_string())
}

async fn execute_deadline_warnings() -> Result<serde_json::Value, String> {
    let warnings = crate::commands::reminder::get_deadline_warnings_with_levels().await?;
    serde_json::to_value(warnings).map_err(|e| e.to_string())
}

async fn execute_dashboard_stats() -> Result<serde_json::Value, String> {
    let stats = crate::commands::cases::get_dashboard_stats().await?;
    serde_json::to_value(stats).map_err(|e| e.to_string())
}

async fn execute_case_create_task(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let case_id = args.get("case_id").and_then(|v| v.as_str()).ok_or("缺少 case_id")?;
    let task_name = args.get("task_name").and_then(|v| v.as_str()).ok_or("缺少 task_name")?;
    let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("normal");
    let due_date = args.get("due_date").and_then(|v| v.as_str());

    let data = serde_json::json!({
        "taskName": task_name,
        "caseId": case_id,
        "priority": priority,
        "deadline": due_date,
        "taskType": "action",
        "startBucket": "anytime"
    });
    crate::commands::tasks::create_task(data).await
}

async fn execute_task_update_status(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let task_id = args.get("task_id").and_then(|v| v.as_str()).ok_or("缺少 task_id")?;
    let action = args.get("action").and_then(|v| v.as_str()).ok_or("缺少 action")?;

    match action {
        "complete" => {
            crate::commands::tasks::toggle_task(task_id.to_string(), None).await?;
            Ok(serde_json::json!({ "success": true }))
        }
        "waiting" => {
            crate::commands::tasks::update_task(serde_json::json!({
                "id": task_id, "taskType": "waiting"
            }))
            .await?;
            Ok(serde_json::json!({ "success": true }))
        }
        "today" => {
            crate::commands::tasks::update_task(serde_json::json!({
                "id": task_id, "startBucket": "today"
            }))
            .await?;
            Ok(serde_json::json!({ "success": true }))
        }
        _ => Err(format!("未知操作: {}", action)),
    }
}

// ═══════════════════════════════════════════════════════════
// Tauri 命令接口（供 commands/mod.rs 调用）
// ═══════════════════════════════════════════════════════════

/// MCP 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub tool: String,
    pub arguments: serde_json::Value,
}

/// 获取 MCP 工具定义列表
pub fn get_tools() -> Vec<McpToolDefinition> {
    McpServer::build_tools()
}

/// 执行 MCP 工具调用
pub async fn execute_tool(call: McpToolCall) -> Result<serde_json::Value, String> {
    execute_tool_by_name(&call.tool, call.arguments).await
}

// ═══════════════════════════════════════════════════════════
// 写操作待确认队列（设计哲学 §11.11 安全约束）
//
// 外部 AI 经 MCP 通道提交的写操作先落 mcp_pending_writes，
// 用户在应用内确认（approve_mcp_write 命令）后才真正执行；
// 提交/批准/拒绝均写 audit_events（actor='mcp'）留痕。
// ═══════════════════════════════════════════════════════════

/// 待确认写记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPendingWrite {
    pub id: String,
    pub tool: String,
    /// arguments JSON 原文
    pub arguments: String,
    pub status: String,
    pub result: Option<String>,
    pub created_at: Option<String>,
    pub resolved_at: Option<String>,
}

/// 写 MCP 审计事件（actor='mcp'）
pub fn write_mcp_audit(
    conn: &rusqlite::Connection,
    write_id: &str,
    event_type: &str,
    payload: &serde_json::Value,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO audit_events (id, aggregate_type, aggregate_id, event_type, payload, actor)
         VALUES (?1, 'mcp_pending_write', ?2, ?3, ?4, 'mcp')",
        rusqlite::params![
            crate::db::new_id(),
            write_id,
            event_type,
            serde_json::to_string(payload).unwrap_or_default(),
        ],
    )?;
    Ok(())
}

/// 提交一条待确认写（MCP 只读通道收到写工具调用时），返回 write_id
pub fn submit_pending_write(
    conn: &rusqlite::Connection,
    tool: &str,
    arguments: &serde_json::Value,
) -> anyhow::Result<String> {
    let write_id = crate::db::new_id();
    conn.execute(
        "INSERT INTO mcp_pending_writes (id, tool, arguments, status)
         VALUES (?1, ?2, ?3, 'pending')",
        rusqlite::params![write_id, tool, serde_json::to_string(arguments).unwrap_or_default()],
    )?;
    write_mcp_audit(
        conn,
        &write_id,
        "mcp_write_submitted",
        &serde_json::json!({ "tool": tool, "arguments": arguments }),
    )?;
    Ok(write_id)
}

fn map_pending_write(row: &rusqlite::Row) -> rusqlite::Result<McpPendingWrite> {
    Ok(McpPendingWrite {
        id: row.get(0)?,
        tool: row.get(1)?,
        arguments: row.get(2)?,
        status: row.get(3)?,
        result: row.get(4)?,
        created_at: row.get(5)?,
        resolved_at: row.get(6)?,
    })
}

const PENDING_WRITE_COLS: &str = "id, tool, arguments, status, result, created_at, resolved_at";

/// 列出所有待确认（status='pending'）的写操作，按提交时间升序
pub fn list_pending_writes(conn: &rusqlite::Connection) -> anyhow::Result<Vec<McpPendingWrite>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM mcp_pending_writes WHERE status='pending' ORDER BY created_at ASC",
        PENDING_WRITE_COLS
    ))?;
    let rows = stmt
        .query_map([], map_pending_write)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 按 id 读取一条待确认写记录
pub fn get_pending_write(
    conn: &rusqlite::Connection,
    id: &str,
) -> anyhow::Result<Option<McpPendingWrite>> {
    conn.query_row(
        &format!("SELECT {} FROM mcp_pending_writes WHERE id=?1", PENDING_WRITE_COLS),
        rusqlite::params![id],
        map_pending_write,
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other.into()),
    })
}

/// 回写确认结果（approved/rejected/executed/failed），返回影响行数
pub fn resolve_pending_write(
    conn: &rusqlite::Connection,
    id: &str,
    status: &str,
    result: Option<&str>,
) -> anyhow::Result<usize> {
    let n = conn.execute(
        "UPDATE mcp_pending_writes
         SET status=?2, result=?3, resolved_at=datetime('now','localtime')
         WHERE id=?1",
        rusqlite::params![id, status, result],
    )?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 最小内存库：mcp_pending_writes + audit_events
    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V12_SQL).unwrap();
        conn.execute_batch(
            "CREATE TABLE audit_events (
               id TEXT PRIMARY KEY, aggregate_type TEXT NOT NULL, aggregate_id TEXT NOT NULL,
               event_type TEXT NOT NULL, payload TEXT,
               actor TEXT DEFAULT 'user' CHECK(actor IN ('user','ai','system','mcp','skill')),
               created_at TEXT DEFAULT (datetime('now','localtime'))
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_submit_and_list_pending_writes() {
        let conn = setup_test_db();
        let id1 = submit_pending_write(&conn, "case_create_task", &serde_json::json!({"case_id":"c1","task_name":"t"})).unwrap();
        let id2 = submit_pending_write(&conn, "task_update_status", &serde_json::json!({"task_id":"t1","action":"complete"})).unwrap();

        let pending = list_pending_writes(&conn).unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].id, id1);
        assert_eq!(pending[1].id, id2);
        assert_eq!(pending[0].status, "pending");
        assert!(pending[0].arguments.contains("case_id"));

        // 提交留痕 audit_events
        let audits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE event_type='mcp_write_submitted' AND actor='mcp'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(audits, 2);
    }

    #[test]
    fn test_resolve_pending_write() {
        let conn = setup_test_db();
        let id = submit_pending_write(&conn, "task_update_status", &serde_json::json!({})).unwrap();

        let n = resolve_pending_write(&conn, &id, "rejected", None).unwrap();
        assert_eq!(n, 1);

        let w = get_pending_write(&conn, &id).unwrap().unwrap();
        assert_eq!(w.status, "rejected");
        assert!(w.resolved_at.is_some());

        // 不再出现在 pending 列表
        assert!(list_pending_writes(&conn).unwrap().is_empty());

        // 不存在的 id
        assert!(get_pending_write(&conn, "no-such").unwrap().is_none());
        assert_eq!(resolve_pending_write(&conn, "no-such", "rejected", None).unwrap(), 0);
    }
}
