# AI / MCP / 编辑器体验升级计划

更新：2026-09-23。对标 Kaku（Agent 直接改 MD + Diff 审阅）与 WeKnora（知识工具面 + MCP + 可引用检索）。不改变「写操作经提案/确认、本地优先」铁律。

## 范围

| 优先级 | 主题 | 验收 |
| --- | --- | --- |
| P0-A | 知识读取三件套 | `search_knowledge` / `read_document` / `list_documents` 契约完整；Agent 只读可闭环 |
| P0-B | 对外 MCP 拉齐内部工具面 | 案件/任务/知识/日历查询 + 写入仍走 pending；token 鉴权保留 |
| P0-C | 工具级开关 + 审批策略 | 设置按工具启用；写工具可标总是批准/总是询问 |
| P0-D | 检索 citation | 命中带 `fileId/chunk/page/snippet`，chat 可点回来源 |
| P1-A | 笔记 AI 编辑 Diff | 知识正文 AI 修改按 hunk 接受/拒绝，默认不落盘 |
| P1-B | 自动反链上下文 | 提问/编辑前注入 backlinks 一圈 + 相关笔记 |
| E-1 | MD/块编辑器写作体验 | 块菜单、拖柄、快捷键、标题大纲、聚焦写作；能持续写文书 |
| E-2 | 块选择与结构操作 | 多选块、上下移动、转换、合并/拆分行内粘贴不烂 |
| P2-A | 确认事实记忆 | 提案确认后沉淀「已确认事实」，chat 可检索注入 |
| P2-B | 文档与状态 | `UPDATE_PLAN` / `CODE_REVIEW` / 本文件与测试记录 |

## 明确不做

- WeKnora 沙箱技能、浏览器操控、多租户 Wiki、服务端部署
- Kaku CRDT 共写（成本高，律师单人写作为主）
- 自动从卷宗生成 Wiki 页（法律事实生成风险）

## 实施状态（2026-09-23）

| 项 | 状态 | 说明 |
| --- | --- | --- |
| P0-A 知识三件套 | 已实现 | `search_knowledge`（带 citation）、`read_document`（分页+可选 backlinks）、`list_documents` |
| P0-B MCP 拉齐 | 已实现 | 增 `knowledge_read` / `document_list`；写仍走 pending |
| P0-C 工具开关 | 已实现 | `setToolPolicy` / 设置键 `ai_tool_policy`（disabled + writeApproval） |
| P0-D citation | 已实现 | 检索/读取返回 `knowledge:{id}` / `draft:{id}` |
| P1-A 笔记 Diff | 部分 | 保留版本 Diff API；全文 hunk UI 下一步 |
| P1-B 反链上下文 | 已实现 | `read_document withBacklinks` 读取图谱一圈 |
| E-1/E-2 编辑器 | 部分 | 字数/段落状态条、专注写作、块菜单扩展 |
| P2-A 确认事实 | 已实现 | `list_confirmed_facts` / `record_confirmed_fact`（写 L2 确认） |
| P2-B 文档 | 已更新 | 本文件 + UPDATE_PLAN |

## 实施顺序

1. 本计划入库  
2. P0-A → P0-D（知识/检索/citation）  
3. P0-B / P0-C（MCP 与工具开关）  
4. P1-A / P1-B（Diff + 反链）  
5. E-1 / E-2（编辑器体验）  
6. P2-A、文档、全套验证  

每步独立可测；完成后 `cargo test` / `vitest` / `vue-tsc` 不回归。
