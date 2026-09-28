# AI / MCP / 编辑器体验升级计划

更新：2026-09-28。对标 Kaku（Agent 直接改 MD + Diff 审阅）与 WeKnora（知识工具面 + MCP + 可引用检索）。不改变「写操作经提案/确认、本地优先」铁律。

## 范围

| 优先级 | 主题 | 验收 |
| --- | --- | --- |
| P0-A | 知识读取三件套 | `search_knowledge` / `read_document` / `list_documents` 契约完整；Agent 只读可闭环 |
| P0-B | 对外 MCP 拉齐内部工具面 | 案件/任务/知识/日历查询 + 写入仍走 pending；token 鉴权保留 |
| P0-C | 工具级开关 + 审批策略 | 设置按工具启用；写工具可标总是批准/总是询问 |
| P0-D | 检索 citation | 命中带 `fileId/chunk/page/snippet`，chat 可点回来源 |
| P1-A | 历史版本优先 | 完整阅读、选定版本恢复、保留恢复前稿件；差异比较仅作辅助 |
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
| P0-C 工具开关 | 已实现 | 完整设置面板、搜索分类、禁用项可找回、持久化成功后生效；仅控制内部 AI |
| P0-D citation | 已实现 | 检索/读取返回 `knowledge:{id}` / `draft:{id}` |
| P1-A 历史版本 | 已实现待原生验收 | 知识历史全文优先；文书历史 schema v43，冲突保护恢复；Diff 为辅助 |
| P1-B 反链上下文 | 已实现 | `read_document withBacklinks` 读取图谱一圈 |
| E-1/E-2 编辑器 | 部分 | 增六点拖柄、落点线、键盘移动与独立撤销；多块选择等不宣称完成 |
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

## 2026-09-28 全模块 UX 修订

范围已扩展为整个产品的导航、加载/错误/空态、保存反馈、键盘访问和恢复交互。详见 [全模块检查与验证边界](UX_CONSISTENCY_AUDIT_2026-09-28.md)。法律文书优先历史版本，不将 hunk Diff 作为下一阶段主目标。
