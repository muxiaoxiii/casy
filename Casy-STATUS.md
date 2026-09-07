# Casy 项目状态

> **2026-09-07 案卷与知识库更新**：已实现可选目录登记、OCR 更新、Markdown 快照、向量索引与文件命名联动；知识库可按案件预览原件正文，编辑器适配四层引用来源主题，启动引导已精简。
> 本轮前端 173 项测试、Rust 完整回归及新增定向测试、桌面/窄屏界面验收通过，新完整 macOS ARM64 安装包已构建并验证。检索选型、MinerU-Popo 复用范围和安装产物见 `docs/audits/workspace-retrieval-editing-2026-09-07.md`。

> **2026-09-07 交付收尾更新**：完整本地 OCR 运行时、medium 模型、Paddle 版面模型、PDF 渲染器、字体和依赖已纳入 macOS 打包，不设 200 MB 上限。
> 完整加密备份及附件路径迁移、编辑崩溃恢复与冲突保护、日历持久重试、PDF/Markdown 对照及区域校订已完成实现与隔离回归。
> 202 项 Rust 库测试及全部集成测试、171 项前端测试通过；中英德法日、竖排、伪造文本层和四页双栏真实模型测试通过。
> 当前公开发行仍缺 Apple 签名公证，其他平台未验收；详细证据、安装产物及边界见 `docs/audits/casy-delivery-completion-2026-09-07.md`。以下为历史阶段记录，不能替代最新验收结果。

> **记录时间**: 2026-09-04 · **状态**: RC（发布候选）· 真实案件测试前隐患收口 · 待 Dogfooding
> **规划**: `docs/refactoring-plan.md` v1.7 · **上位哲学**: `docs/casy-product-design-v3.md` v3.1
> **基线**: vue-tsc 0 ✓ · vitest 120 ✓ · cargo test 172+ lib + 集成 ✓ · build ✓
> **最新批次**: 真实案件测试前收口（2026-09-04）—— 民事+行政并行路由 · PDF 页级 FTS trigram · 上传先归档 · 编辑器/导出/打开路径安全加固
> 详见 `docs/audits/casy-ipc-strict-review-2026-09-04.md` 与 `docs/audits/casy-real-case-readiness-review-2026-09-04.md`

---

## 一、里程碑全景（自 2026-08-21 接手以来）

| 里程碑 | 状态 | 关键产出 |
|---|---|---|
| B0 仓库止血 | ✅ | rebase 事故恢复；门禁体系建立 |
| M-GTD-1 不打断 | ✅ | 一键完成/乐观更新/⌘Z/系统通知/parseWhen |
| M-GTD-2 组织得起来 | ✅ | A1-1~A1-7 全量：项目建模(绞杀式阶段一)/Areas/拖拽排序/子任务/重复任务/⌘K/Review 修复 |
| M-CAL-1 日历完整化 | ✅ | D-7 calendar_events 实体；年/月/周/日四视图；NL 直建日程；双向拖拽改期 |
| A-UI 动效地基+优化轮 | ✅ | Motion Tokens；六表面自绘；令牌化 514 处；空状态/对话框节奏统一；KeyboardCenter |
| B3/B1 内核收口 | ✅ | 确认策略上收；类型双轨合一；事件层激活；bindings 83 类型；错误码全域 |
| B4 生产化 | 🔶 | R-1 身份/updater ✅ · R-3 CI ✅ · R-4 CSP/审计 ✅ · R-5 备份恢复 ✅ · R-6 崩溃日志 ✅ · R-7 合规文本 ✅ · R-2 签名 ⏸️ 待证书 |
| 对标灵感落地（豁免冻结） | ✅ | AI Proposal Diff+@引用 · Defer Date · 通知中心 · 期限规则自定义+留痕 · 双链 · OCR+SmartRules · persons 对象 · 事实白板 |
| 严格 IPC 收口 | ✅ | 所有前端 IPC 字面量命令入 CommandMap；禁止显式泛型、动态命令名、直接 invoke、宽 fallback 类型；Case/Task/Calendar PATCH 改为精确 DTO |
| 真实案件测试前收口 | ✅ | 新建/导入并行案件可落库；卷宗上传先入案件目录；PDF 页文本进入 `document_pages_fts`；搜索特殊字符安全；编辑器引用渲染与导出路径加固 |

## 二、当前形态

- **架构**：cordis 式内核（Context/Service/Fiber）+ 单一数据通路 + AI 无特权通道（executeTool 强制确认 + audit_events 归因）
- **Schema v26**：tasks(+parent/recurrence/defer_until/deleted_at) / projects + case_legal_details / calendar_events / notifications / links / persons+case_persons / smart_rules / whiteboards+fact_nodes / deadline_rule_audit / document_jobs / document_pages_fts
- **类型安全**：specta 绑定 121 类型 + CommandMap 276 键契约注册表；tauriBridge 仅允许已登记命令；前端显式泛型 IPC 调用 0，动态命令名 0，直接 invoke 0
- **CI**：.github/workflows/ci.yml 三平台矩阵 + bindings 漂移校验 + tag 发布草稿

## 三、已知问题 / 待办

### 边界项（需用户）
- R-2 证书采购（Apple $99/年 + Windows Authenticode）→ scripts/signing/ 已就绪
- Dogfooding 第一周 → docs/dogfooding-round1.md 清单就绪

### 技术备忘
- quick-xml 升级需跨 minor 适配 calamine/docsy_engine（本地输入面，接受风险，见 security-audit-r4.md）
- Case/Task/Calendar PATCH 与主要 service 返回类型已完成 DTO 收口；后续重点转向业务组件去 `any`、AI 工具参数运行时校验、错误码结构化
- PDF 页级 FTS 已接入全局搜索；下一步是把页码/坐标/证据引用锚点整合到知识库搜索与 Markdown 编辑器工作流
- CalendarView forecast 的任务投影依赖 get_calendar_events 月度窗口，年视图已单独处理
- docsy_engine 改名建议挂 C 线解冻首批

## 四、验证方式

```bash
npm run typecheck
npm run test:unit
(cd src-tauri && cargo test)
npm run build
.github/workflows/ci.yml   # push 自动三平台矩阵 + bindings 漂移校验
```
