# Casy 插件系统补齐为真实实现（架构收口）

> **日期**: 2026-08-19
> **背景**: 用户反馈"代码距离设计哲学太远，每个模块都偏离设计初衷"。
> 经逐层核验（设计哲学 vs 前端 vs 后端 vs 文档），最大偏离是 **v3.0 插件化架构是"文档声称已完成、代码为占位符"**。
> 本轮按用户决策"保留插件架构、补齐为真实现 + 架构清理优先"执行。

---

## 〇、核验结论（偏离清单）

| # | 偏离 | 证据 |
|---|------|------|
| 1 | 插件系统是空壳 | `context.ts` / `initializer.ts` / `tool-caller.ts` 均为 TODO 占位符；9 个插件（1260 行）从未被安装；插件 `import from '../plugin/types'` 而该文件不存在 |
| 2 | AI 对话面板运行即崩 | `AIChatPanel.vue` 调用不存在的 `casyContext.getToolDefinitions()` / `getProviders()` / `aiToolCaller.chatWithTools()` |
| 3 | 文档浮夸 | STATUS-v3/README 声称"CasyContext ✅ / 38 工具 ✅ / 事件系统 ✅"，实际不存在（devlog 已诚实收口，STATUS 未同步） |
| 4 | 后端无通用对话通道 | `AiBackend` 只有单轮 `chat_completion(system, user)`，无多轮 messages、无工具循环命令 |

## 一、实施

### 前端（npm run build 通过）

| 文件 | 变更 |
|------|------|
| `src/core/plugin/types.ts` | **新建**：CasyTool / CasySkill / CasyProvider / CasyPlugin / CasyContext / ConfirmRequest 等类型契约 |
| `src/core/plugin/context.ts` | **重写**：真实 CasyContext（插件/工具/技能/提供商注册、on/emit 事件、画像、effective_policy 前端镜像 + requestConfirm 弹窗；system_minimum=外部写 L3 硬编码） |
| `src/core/plugin/initializer.ts` | **重写**：启动安装 9 插件（38 工具真实注册）+ 从后端 `get_ai_config` 注册 AI 提供商（Ollama/OpenAI/DeepSeek + 默认模型清单） |
| `src/core/ai/tool-caller.ts` | **重写**：`chatWithTools` 多轮工具循环（≤5 轮）——模型输出 JSON 信封 → 解析 → 经插件 executeTool → tauriBridge → Rust 命令 → 结果回喂；`ai_chat` 失败时友好报错 |
| `src/modules/ai/components/AIChatPanel.vue` | 修复：提供商/模型下拉消费真实 providers；监听 `plugins:ready` 刷新（避免初始化时序竞态）；错误处理兼容 string |
| `src/core/mockData.ts` | 浏览器预览模式补 `ai_chat` mock（可走通工具调用演示） |

### 后端（cargo check 0 error / 15 warning 均为存量）

| 文件 | 变更 |
|------|------|
| `src-tauri/src/ai/mod.rs` | 新增 `ChatMessage` 结构；`AiBackend` trait 新增 `chat_messages`（默认退化单轮）；Ollama/OpenAI 后端原生多轮实现；新增 `ai_chat` 命令（mode/api_url/model 覆盖、ai_runs 审计 SHA256 脱敏、每日限额） |
| `src-tauri/src/commands/mod.rs` | 注册 `ai_chat` |

## 二、设计哲学对齐

- **§11.11 智伴层组件化**：插件成为真实工具注册表；AI 只做"判断"（选工具/给参数），确定性执行永不绕过 Rust 命令（写入口唯一）
- **§原则六 双路径铁律**：工具 execute → tauriBridge → 后端命令，插件层不触碰数据库
- **§11.4 Confirmer**：写工具（delete_case 等）走 effective_policy + requestConfirm（L3 需输入"确认"）
- **§11.9 模型可见即记录**：每次对话经 `ai_chat` 写 `ai_runs`

## 二点五、工具调用失败修复（用户反馈"老有 toolcall 失败"）

> 插件层是上一个 AI 会话写的，38 个工具的 execute 参数形状大量与后端真实签名不符。
> 逐条对照 src-tauri/src/commands/*.rs 签名后修复 13 处，全部编译通过。

| 工具 | 原调用（错） | 修复后（对） |
|------|------|------|
| search_cases | `{keyword}` | `{query}`（后端参数名是 query） |
| search_knowledge | `{keyword, limit}` | `{query}` |
| update_task | `{id, data}` | `{data: {...data, id}}`（后端只收 data，id 必须在 data 内） |
| add_inbox_item | `{content_text, source_type, source_path}` | `{sourceType, contentText, sourcePath}`（Tauri 2 要求 JS 侧 camelCase） |
| file_inbox_item | `{id, case_id}` | `{itemId, caseId}` |
| create_reminder_rule | `{name, trigger_type...}` | `{data: {name, triggerType...}}`（后端收 data 包装） |
| start_reminder_engine | `{interval_secs}` | `{intervalSecs}` |
| list_case_files | `{case_id}` | `{caseId}` |
| add_case_file | `{case_id, file_path}` | `{caseId, fileName, filePath, category}`（补必填 fileName） |
| delete_case_file | `{case_id, file_id}` | `{id}`（后端只收 id） |
| save_settings | `{data}` | `{settings}` |
| configure_ai | `{endpoint, api_key}` | `{apiUrl, apiKey}` |
| test_webdav_connection / manual_sync_push / manual_sync_pull | 空参 / 调用了**不存在的命令** | 从设置读 WebDAV 凭据；manual_sync_* 改调真实命令 webdav_push / webdav_pull |

**顺带加固**：工具循环耗尽（5 轮）后若最后回复仍是工具 JSON 信封，不再把原始 JSON 当答案，改为友好汇总。

## 三、遗留（路线图后置，不冒充已实现）

- Ollama 模型自动发现（当前为"已配置模型 + 默认清单"，模型名可在设置页填写）
- 技能（skill）尚无注册实例——接口就绪，等"内部访问外部"（§11.11 ②）具体需求
- 前端事件与后端 `audit_events` 的桥接（当前前端级 pub/sub 与后端领域事件分离，符合"事件与审计分离"设计）
