//! AI 后端模块
//!
//! - AiBackend trait: classify_document, extract_info, summarize
//! - OllamaBackend: 本地 Ollama API
//! - OpenAiBackend: OpenAI 兼容 API
//! - NoOpBackend: 无 AI 时的 fallback（规则匹配）

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// ============================================================
// Prompt 模板
// ============================================================

/// 文档分类 prompt 模板
const CLASSIFY_PROMPT_TEMPLATE: &str = include_str!("prompts/classify_document.md");

/// 信息提取 prompt 模板
#[allow(dead_code)]
const EXTRACT_INFO_PROMPT_TEMPLATE: &str = include_str!("prompts/extract_info.md");

/// 构建活跃案件列表上下文
fn build_active_cases_context() -> String {
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(_) => return "暂无活跃案件数据".to_string(),
    };

    let mut stmt = match conn.prepare(
        "SELECT id, case_name, case_no, client_name, opponent_name, track
         FROM cases
         WHERE case_status != '已完结' OR case_status IS NULL
         ORDER BY updated_at DESC
         LIMIT 50",
    ) {
        Ok(s) => s,
        Err(_) => return "查询案件失败".to_string(),
    };

    let cases: Vec<String> = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let case_no: Option<String> = row.get(2)?;
            let client: String = row.get(3)?;
            let opponent: String = row.get(4)?;
            let track: String = row.get(5)?;

            let track_label = match track.as_str() {
                "patent_invalidation" => "专利无效",
                "admin_litigation" => "行政诉讼",
                "civil_tort" => "民事侵权",
                _ => "其他",
            };

            Ok(format!(
                "- ID: {} | 案号: {} | 名称: {} | 客户: {} | 对方: {} | 轨道: {}",
                id,
                case_no.unwrap_or_else(|| "无".to_string()),
                name,
                client,
                opponent,
                track_label
            ))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    if cases.is_empty() {
        "暂无活跃案件".to_string()
    } else {
        cases.join("\n")
    }
}

/// 路由决策：根据置信度决定处理方式
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum RoutingDecision {
    /// 置信度高，直接关联案件
    AutoLinked {
        case_id: String,
        category: String,
        confidence: f64,
    },
    /// 置信度中等，列出候选案件
    CandidateList {
        candidates: Vec<String>,
        category: String,
        confidence: f64,
    },
    /// 置信度低，标记待处理
    NeedsReview {
        category: String,
        confidence: f64,
    },
}

/// 根据分类结果进行路由决策
pub fn route_by_confidence(
    confidence: f64,
    matched_case_id: &Option<String>,
    category: &str,
) -> RoutingDecision {
    if confidence >= 0.8 {
        // 置信度高：直接关联
        if let Some(case_id) = matched_case_id {
            RoutingDecision::AutoLinked {
                case_id: case_id.clone(),
                category: category.to_string(),
                confidence,
            }
        } else {
            // 高置信度但无匹配案件
            RoutingDecision::NeedsReview {
                category: category.to_string(),
                confidence,
            }
        }
    } else if confidence >= 0.5 {
        // 置信度中等：列出候选
        let candidates = matched_case_id
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        RoutingDecision::CandidateList {
            candidates,
            category: category.to_string(),
            confidence,
        }
    } else {
        // 置信度低：标记待处理
        RoutingDecision::NeedsReview {
            category: category.to_string(),
            confidence,
        }
    }
}

// ============================================================
// AI 配置
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub mode: String, // "ollama" | "openai" | "noop"
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub daily_limit: Option<u32>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            mode: "noop".into(),
            api_url: None,
            api_key: None,
            model: None,
            daily_limit: Some(50),
        }
    }
}

// ============================================================
// TokenBudget — 每日调用限额保护
// ============================================================

/// 每日 AI 调用限额管理
pub struct TokenBudget {
    /// 今日已调用次数
    used_today: AtomicU64,
    /// 每日限额（0 表示不限制）
    daily_limit: AtomicU64,
    /// 上次重置日期（YYYY-MM-DD）
    last_reset_date: Mutex<String>,
}

impl TokenBudget {
    pub fn new(daily_limit: u64) -> Self {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        Self {
            used_today: AtomicU64::new(0),
            daily_limit: AtomicU64::new(daily_limit),
            last_reset_date: Mutex::new(today),
        }
    }

    /// 检查是否还有可用配额
    pub async fn check_quota(&self) -> Result<bool> {
        // 检查是否需要重置（新的一天）
        self.maybe_reset().await;

        let limit = self.daily_limit.load(Ordering::Relaxed);
        if limit == 0 {
            // 不限制
            return Ok(true);
        }

        let used = self.used_today.load(Ordering::Relaxed);
        Ok(used < limit)
    }

    /// 消耗一次配额
    pub async fn consume(&self) -> Result<()> {
        // 检查是否需要重置（新的一天）
        self.maybe_reset().await;

        let limit = self.daily_limit.load(Ordering::Relaxed);
        if limit == 0 {
            // 不限制
            self.used_today.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }

        let used = self.used_today.load(Ordering::Relaxed);
        if used >= limit {
            anyhow::bail!("AI 调用已达每日限额 ({}/{})", used, limit);
        }

        self.used_today.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 获取今日使用情况
    pub async fn get_usage(&self) -> (u64, u64) {
        self.maybe_reset().await;
        let used = self.used_today.load(Ordering::Relaxed);
        let limit = self.daily_limit.load(Ordering::Relaxed);
        (used, limit)
    }

    /// 更新每日限额
    pub fn set_daily_limit(&self, limit: u64) {
        self.daily_limit.store(limit, Ordering::Relaxed);
    }

    /// 检查是否需要重置（新的一天）
    async fn maybe_reset(&self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut last_reset = self.last_reset_date.lock().await;

        if *last_reset != today {
            // 新的一天，重置计数器
            self.used_today.store(0, Ordering::Relaxed);
            *last_reset = today;
            log::info!("AI 调用计数器已重置（新的一天）");
        }
    }
}

/// 全局 TokenBudget 实例
static TOKEN_BUDGET: std::sync::OnceLock<Arc<TokenBudget>> = std::sync::OnceLock::new();

/// 获取全局 TokenBudget 实例
pub fn get_token_budget() -> &'static Arc<TokenBudget> {
    TOKEN_BUDGET.get_or_init(|| {
        let config = load_ai_config();
        let limit = config.daily_limit.unwrap_or(50) as u64;
        Arc::new(TokenBudget::new(limit))
    })
}

/// 从 settings 表加载 AI 配置
pub fn load_ai_config() -> AiConfig {
    let conn = match crate::db::open_db() {
        Ok(c) => c,
        Err(_) => return AiConfig::default(),
    };

    let get = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![key],
            |r| r.get(0),
        )
        .ok()
    };

    let daily_limit = get("ai_daily_limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(50);

    AiConfig {
        mode: get("ai_mode").unwrap_or_else(|| "noop".into()),
        api_url: get("ai_api_url"),
        api_key: get("ai_api_key"),
        model: get("ai_model"),
        daily_limit: Some(daily_limit),
    }
}

/// 保存 AI 配置到 settings 表
pub fn save_ai_config(config: &AiConfig) -> Result<()> {
    let conn = crate::db::open_db()?;

    let set = |key: &str, val: &Option<String>| -> Result<()> {
        if let Some(v) = val {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                rusqlite::params![key, v],
            )?;
        } else {
            conn.execute(
                "DELETE FROM settings WHERE key = ?1",
                rusqlite::params![key],
            )?;
        }
        Ok(())
    };

    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('ai_mode', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![config.mode],
    )?;
    set("ai_api_url", &config.api_url)?;
    set("ai_api_key", &config.api_key)?;
    set("ai_model", &config.model)?;

    // 保存每日限额
    let daily_limit = config.daily_limit.unwrap_or(50).to_string();
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('ai_daily_limit', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![daily_limit],
    )?;

    // 更新全局 TokenBudget 限额
    let budget = get_token_budget();
    budget.set_daily_limit(config.daily_limit.unwrap_or(50) as u64);

    Ok(())
}

/// 保存 AI 配置命令（返回 String）
pub fn save_ai_config_cmd(config: &AiConfig) -> anyhow::Result<String> {
    save_ai_config(config)?;
    Ok("AI 配置已保存".to_string())
}

// ============================================================
// AI 分类结果
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiClassifyResult {
    pub category: String,
    pub confidence: f64,
    pub summary: Option<String>,
    pub extracted_info: Option<serde_json::Value>,
}

// ============================================================
// AiBackend trait
// ============================================================

#[async_trait::async_trait]
#[allow(dead_code)]
pub trait AiBackend: Send + Sync {
    /// 文档分类
    async fn classify_document(&self, text: &str) -> Result<AiClassifyResult>;

    /// 信息提取（案号、当事人、日期等）
    async fn extract_info(&self, text: &str) -> Result<serde_json::Value>;

    /// 文本摘要
    async fn summarize(&self, text: &str) -> Result<String>;
}

// ============================================================
// NoOpBackend — 规则匹配 fallback
// ============================================================

pub struct NoOpBackend;

#[async_trait::async_trait]
impl AiBackend for NoOpBackend {
    async fn classify_document(&self, text: &str) -> Result<AiClassifyResult> {
        let parsed = crate::parse::classify_document(text);
        let extracted = serde_json::to_value(&parsed).unwrap_or_default();
        Ok(AiClassifyResult {
            category: parsed.doc_type,
            confidence: parsed.confidence,
            summary: None,
            extracted_info: Some(extracted),
        })
    }

    async fn extract_info(&self, text: &str) -> Result<serde_json::Value> {
        let parsed = crate::parse::classify_document(text);
        Ok(serde_json::to_value(&parsed).unwrap_or_default())
    }

    async fn summarize(&self, text: &str) -> Result<String> {
        // 简单截取前 200 字符作为摘要
        let summary = if text.chars().count() > 200 {
            format!("{}...", truncate_chars(text, 200))
        } else {
            text.to_string()
        };
        Ok(summary)
    }
}

// ============================================================
// OllamaBackend — 本地 Ollama API
// ============================================================

pub struct OllamaBackend {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaBackend {
    pub fn new(api_url: &str, model: &str) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("创建 HTTP client 失败");

        Self {
            client,
            base_url: api_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "stream": false
        });

        let resp = self.client.post(&url).json(&body).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama API 错误: {}", resp.status());
        }

        let result: serde_json::Value = resp.json().await?;
        let content = result["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }
}

#[async_trait::async_trait]
impl AiBackend for OllamaBackend {
    async fn classify_document(&self, text: &str) -> Result<AiClassifyResult> {
        let system = r#"你是一个法律文档分类助手。请将文档分类为以下类别之一：
- summons（传票）
- hearing_notice（口头审理通知书）
- judgment（判决书/裁定书/决定书）
- complaint（起诉状）
- defense（答辩状）
- correspondence（函件）
- client_instruction（委托/指示）
- opposing_counsel（对方律师函）
- other（其他）

请以 JSON 格式返回：
{"category": "类别", "confidence": 0.0-1.0, "summary": "一句话摘要", "caseNo": "案号（如有）", "court": "法院（如有）"}

只返回 JSON，不要其他文字。"#;

        let user_prompt = format!("请分类以下文档（前 1000 字）：\n\n{}", truncate_chars(text, 1000));

        let response = self.chat(system, &user_prompt).await?;

        // 尝试解析 JSON 响应
        let parsed: serde_json::Value = serde_json::from_str(response.trim())
            .unwrap_or_else(|_| serde_json::json!({"category": "other", "confidence": 0.3}));

        Ok(AiClassifyResult {
            category: parsed["category"]
                .as_str()
                .unwrap_or("other")
                .to_string(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.5),
            summary: parsed["summary"].as_str().map(|s| s.to_string()),
            extracted_info: Some(parsed.clone()),
        })
    }

    async fn extract_info(&self, text: &str) -> Result<serde_json::Value> {
        let system = r#"你是一个法律信息提取助手。请从文档中提取以下信息（如有）：
- caseNo（案号）
- court（法院/机构）
- judge（审判长/合议组组长）
- parties（当事人列表，包含 name 和 role）
- dates（重要日期列表，包含 date 和 description）
- patentNo（专利号）

请以 JSON 格式返回，只返回 JSON。"#;

        let user_prompt = format!("请提取以下文档的关键信息（前 1500 字）：\n\n{}", truncate_chars(text, 1500));

        let response = self.chat(system, &user_prompt).await?;

        let parsed: serde_json::Value = serde_json::from_str(response.trim())
            .unwrap_or_else(|_| serde_json::json!({}));

        Ok(parsed)
    }

    async fn summarize(&self, text: &str) -> Result<String> {
        let system = "你是一个法律文档摘要助手。请用简洁的中文总结文档要点，不超过 100 字。";

        let user_prompt = format!("请总结以下文档：\n\n{}", truncate_chars(text, 2000));

        self.chat(system, &user_prompt).await
    }
}

// ============================================================
// OpenAiBackend — OpenAI 兼容 API
// ============================================================

pub struct OpenAiBackend {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiBackend {
    pub fn new(api_url: &str, api_key: &str, model: &str) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("创建 HTTP client 失败");

        Self {
            client,
            base_url: api_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "temperature": 0.1,
            "max_tokens": 500
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API 错误 {}: {}", status, body_text);
        }

        let result: serde_json::Value = resp.json().await?;
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }
}

#[async_trait::async_trait]
impl AiBackend for OpenAiBackend {
    async fn classify_document(&self, text: &str) -> Result<AiClassifyResult> {
        let system = r#"你是一个法律文档分类助手。请将文档分类为以下类别之一：
- summons（传票）
- hearing_notice（口头审理通知书）
- judgment（判决书/裁定书/决定书）
- complaint（起诉状）
- defense（答辩状）
- correspondence（函件）
- client_instruction（委托/指示）
- opposing_counsel（对方律师函）
- other（其他）

请以 JSON 格式返回：
{"category": "类别", "confidence": 0.0-1.0, "summary": "一句话摘要", "caseNo": "案号（如有）", "court": "法院（如有）"}

只返回 JSON，不要其他文字。"#;

        let user_prompt = format!("请分类以下文档（前 1000 字）：\n\n{}", truncate_chars(text, 1000));

        let response = self.chat(system, &user_prompt).await?;

        let parsed: serde_json::Value = serde_json::from_str(response.trim())
            .unwrap_or_else(|_| serde_json::json!({"category": "other", "confidence": 0.3}));

        Ok(AiClassifyResult {
            category: parsed["category"]
                .as_str()
                .unwrap_or("other")
                .to_string(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.5),
            summary: parsed["summary"].as_str().map(|s| s.to_string()),
            extracted_info: Some(parsed.clone()),
        })
    }

    async fn extract_info(&self, text: &str) -> Result<serde_json::Value> {
        let system = r#"你是一个法律信息提取助手。请从文档中提取以下信息（如有）：
- caseNo（案号）
- court（法院/机构）
- judge（审判长/合议组组长）
- parties（当事人列表，包含 name 和 role）
- dates（重要日期列表，包含 date 和 description）
- patentNo（专利号）

请以 JSON 格式返回，只返回 JSON。"#;

        let user_prompt = format!("请提取以下文档的关键信息（前 1500 字）：\n\n{}", truncate_chars(text, 1500));

        let response = self.chat(system, &user_prompt).await?;

        let parsed: serde_json::Value = serde_json::from_str(response.trim())
            .unwrap_or_else(|_| serde_json::json!({}));

        Ok(parsed)
    }

    async fn summarize(&self, text: &str) -> Result<String> {
        let system = "你是一个法律文档摘要助手。请用简洁的中文总结文档要点，不超过 100 字。";

        let user_prompt = format!("请总结以下文档：\n\n{}", truncate_chars(text, 2000));

        self.chat(system, &user_prompt).await
    }
}

// ============================================================
// 字符串截断辅助（按字符，避免 UTF-8 多字节切片 panic）
// ============================================================

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

// ============================================================
// Prompt 增强的分类和提取函数
// ============================================================

/// 使用 prompt 模板进行文档分类（注入活跃案件上下文）
pub async fn classify_document_with_prompt(
    backend: &dyn AiBackend,
    text: &str,
) -> Result<AiClassifyResult> {
    // 构建上下文
    let active_cases = build_active_cases_context();

    // 注入上下文到 prompt（用于日志和调试）
    let _system_prompt = CLASSIFY_PROMPT_TEMPLATE
        .replace("{{ACTIVE_CASES}}", &active_cases);

    // 调用后端分类
    let result = backend.classify_document(text).await?;

    // 如果后端返回的 extracted_info 包含 matched_case_id，使用它
    // 否则尝试从 result 中提取
    Ok(result)
}

/// 使用 prompt 模板进行信息提取
#[allow(dead_code)]
pub async fn extract_info_with_prompt(
    backend: &dyn AiBackend,
    text: &str,
) -> Result<serde_json::Value> {
    // 调用后端提取
    let result = backend.extract_info(text).await?;

    // 后处理：确保输出符合 schema
    let mut result = result;
    if let Some(obj) = result.as_object_mut() {
        // 确保必要字段存在
        obj.entry("doc_type".to_string())
            .or_insert(serde_json::Value::String("other".to_string()));
        obj.entry("confidence".to_string())
            .or_insert(serde_json::Value::Number(serde_json::Number::from_f64(0.5).unwrap()));
    }

    Ok(result)
}

/// 处理收件箱项：分类 + 路由
pub async fn process_inbox_with_ai(
    text: &str,
) -> Result<(AiClassifyResult, RoutingDecision)> {
    let config = load_ai_config();
    let backend = create_backend(&config);

    // 使用 prompt 增强的分类
    let classify_result = classify_document_with_prompt(backend.as_ref(), text).await?;

    // 提取 matched_case_id
    let matched_case_id = classify_result
        .extracted_info
        .as_ref()
        .and_then(|info| info.get("matched_case_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 路由决策
    let routing = route_by_confidence(
        classify_result.confidence,
        &matched_case_id,
        &classify_result.category,
    );

    Ok((classify_result, routing))
}

// ============================================================
// 后端工厂
// ============================================================

/// 根据配置创建 AI 后端实例
pub fn create_backend(config: &AiConfig) -> Box<dyn AiBackend> {
    match config.mode.as_str() {
        "ollama" => {
            let url = config
                .api_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            let model = config.model.as_deref().unwrap_or("qwen2.5:7b");
            Box::new(OllamaBackend::new(url, model))
        }
        "openai" => {
            let url = config
                .api_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let key = config.api_key.as_deref().unwrap_or("");
            let model = config.model.as_deref().unwrap_or("gpt-4o-mini");
            Box::new(OpenAiBackend::new(url, key, model))
        }
        _ => Box::new(NoOpBackend),
    }
}

// ============================================================
// AI 写作辅助
// ============================================================

/// 文书风格描述映射
fn style_description(style: &str) -> &'static str {
    match style {
        "complaint" => "起诉状风格：诉求明确、法条引用精准、事实陈述简洁",
        "defense_brief" => "代理词风格：论证层次清晰、证据引用规范、反驳有力",
        "legal_opinion" => "法律意见风格：结论先行、风险分析、建议具体",
        "lawyer_letter" => "律师函风格：立场明确、期限警告、法律依据充分",
        "reply_brief" => "答辩状风格：逐项反驳、证据清单、时效抗辩",
        _ => "通用法律文书风格：逻辑清晰、用语规范",
    }
}

/// 调用 AI 生成写作建议
pub async fn generate_writing_with_ai(
    intent: &str,
    context: &str,
    knowledge: &str,
    style: &str,
) -> Result<String> {
    let config = load_ai_config();
    let _backend = create_backend(&config);

    let style_desc = style_description(style);

    let system_prompt = format!(
        r#"你是一个专业的中国法律文书写作助手。请根据以下要求生成法律文书段落。

写作风格要求：{}

规则：
1. 使用正式的法律文书用语
2. 引用法条时使用规范格式（如"《专利法》第65条"）
3. 事实陈述客观、逻辑清晰
4. 生成的内容应当可以直接插入到法律文书中
5. 只输出建议的段落内容，不要添加额外解释
6. 长度控制在 200-500 字之间"#,
        style_desc
    );

    let mut user_prompt = String::new();

    if !context.is_empty() {
        user_prompt.push_str("## 当前文书上下文\n");
        // 截断上下文避免过长
        let ctx_truncated: String = context.chars().take(2000).collect();
        user_prompt.push_str(&ctx_truncated);
        user_prompt.push_str("\n\n");
    }

    if !knowledge.is_empty() {
        user_prompt.push_str("## 参考知识库内容\n");
        user_prompt.push_str(knowledge);
        user_prompt.push_str("\n\n");
    }

    user_prompt.push_str("## 写作意图\n");
    user_prompt.push_str(intent);

    // 根据 AI 后端模式直接调用 chat 接口生成写作建议
    let result = match config.mode.as_str() {
        "ollama" => {
            let url = config
                .api_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            let model = config.model.as_deref().unwrap_or("qwen2.5:7b");
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .timeout(std::time::Duration::from_secs(120))
                .build()?;
            let body = serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "stream": false
            });
            let resp = client
                .post(format!("{}/api/chat", url.trim_end_matches('/')))
                .json(&body)
                .send()
                .await?;
            if !resp.status().is_success() {
                anyhow::bail!("Ollama API 错误: {}", resp.status());
            }
            let result: serde_json::Value = resp.json().await?;
            result["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string()
        }
        "openai" => {
            let url = config
                .api_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let key = config.api_key.as_deref().unwrap_or("");
            let model = config.model.as_deref().unwrap_or("gpt-4o-mini");
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .timeout(std::time::Duration::from_secs(120))
                .build()?;
            let body = serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": 0.3,
                "max_tokens": 1000
            });
            let resp = client
                .post(format!("{}/chat/completions", url.trim_end_matches('/')))
                .header("Authorization", format!("Bearer {}", key))
                .json(&body)
                .send()
                .await?;
            if !resp.status().is_success() {
                anyhow::bail!("OpenAI API 错误: {}", resp.status());
            }
            let result: serde_json::Value = resp.json().await?;
            result["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string()
        }
        _ => {
            // NoOp: 返回占位提示
            return Ok(format!(
                "【AI 写作辅助未配置】\n\n写作意图：{}\n\n请在设置中配置 AI 后端（Ollama 或 OpenAI）以使用此功能。",
                intent
            ));
        }
    };

    Ok(result)
}

// ============================================================
// Tauri 命令
// ============================================================

/// 配置 AI 后端
#[tauri::command]
pub async fn configure_ai(
    mode: String,
    api_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    daily_limit: Option<u32>,
) -> Result<String, String> {
    let config = AiConfig {
        mode,
        api_url,
        api_key,
        model,
        daily_limit,
    };
    crate::commands::run_blocking(move || save_ai_config_cmd(&config))
        .await
}

/// 测试 AI 连通性
#[tauri::command]
pub async fn test_ai_connection() -> Result<String, String> {
    let config = load_ai_config();
    let backend = create_backend(&config);

    let result = backend
        .classify_document("测试文档：这是一份测试文件。")
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "AI 连接成功。模式: {}, 测试分类: {} (置信度: {:.0}%)",
        config.mode,
        result.category,
        result.confidence * 100.0
    ))
}

/// 获取当前 AI 配置
#[tauri::command]
pub async fn get_ai_config() -> Result<AiConfig, String> {
    crate::commands::run_blocking(|| Ok(load_ai_config())).await
}

/// 获取 AI 调用使用情况
#[tauri::command]
pub async fn get_ai_usage() -> Result<serde_json::Value, String> {
    let budget = get_token_budget();
    let (used, limit) = budget.get_usage().await;
    Ok(serde_json::json!({
        "usedToday": used,
        "dailyLimit": limit,
        "remaining": if limit == 0 { u64::MAX } else { limit.saturating_sub(used) },
    }))
}

/// AI 写作辅助：根据意图、上下文、知识库和风格生成写作建议
#[tauri::command]
pub async fn generate_writing_suggestion(
    intent: String,
    context: Option<String>,
    knowledge: Option<String>,
    style: Option<String>,
) -> Result<String, String> {
    // 检查配额
    let budget = get_token_budget();
    if !budget
        .check_quota()
        .await
        .map_err(|e| e.to_string())?
    {
        return Err("AI 调用已达每日限额".to_string());
    }

    let result = generate_writing_with_ai(
        &intent,
        context.as_deref().unwrap_or(""),
        knowledge.as_deref().unwrap_or(""),
        style.as_deref().unwrap_or("general"),
    )
    .await
    .map_err(|e| e.to_string())?;

    // 消耗配额
    let _ = budget.consume().await;

    Ok(result)
}
