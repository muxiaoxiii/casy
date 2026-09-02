# 全球对标灵感落地计划（Gemini 报告 10 项 → Casy 实装）

> **日期**: 2026-09-01 · **状态**: 执行中
> **来源**: 用户提供的 Gemini 全球对标调研报告（Amplenote/Things3/Linear/OmniFocus/LawToolBox/Capacities/Hookmark/LiquidText/DEVONthink/Cursor）
> **裁决**: 用户明确指示 **不受 V5 特性冻结约束**，10 项全部实施。
> **执行方式**: 主代理做 Phase 0 骨架预接线（schema/命令注册/契约），7 个并行子代理按工作流实现，主代理集成 + review + walkthrough。

---

## 一、工作流划分与文件所有权（防冲突）

| 工作流 | 对应灵感 | 后端文件（独占） | 前端文件（独占） |
|---|---|---|---|
| W1 | #9 AI Diff 视图 + #10 `@` 引用上下文 | `commands/ai_routes.rs`、`src-tauri/src/ai/**` | `src/modules/ai/**`、`src/core/ai/**` |
| W2 | #3 Defer Date + #2 安静通知中心 | `commands/tasks.rs`（仅 defer 相关追加）、`commands/notifications.rs` | `src/modules/tasks/**`、`src/modules/notifications/**`（新建）、`App.vue` 顶栏铃铛 |
| W3 | #4 期限规则自定义 + 留痕 | `commands/deadline_rules.rs`、`deadline/engine.rs`（只读扩展） | `src/modules/settings/components/DeadlineRulesSettings.vue`（新建） |
| W4 | #6 跨模块双链 | `commands/linking.rs` | `src/modules/docs/**`、`src/modules/knowledge/**` 反链面板 |
| W5 | #8 本地 OCR + Smart Rules | `commands/smart_rules.rs`、`parse/pdf_extractor.rs`、`background_jobs.rs` | `src/modules/files/**`、`src/modules/settings/components/SmartRulesSettings.vue`（新建） |
| W6 | #5 对象化实体 | `commands/persons.rs` | `src/modules/persons/**`（新建） |
| W7 | #7 事实白板 | `commands/whiteboard.rs` | `src/modules/whiteboard/**`（新建） |

**共享文件纪律（只有主代理可改）**：`db/schema.rs`、`commands/mod.rs`、`lib.rs`、`router/index.js`、`types/commandMap.ts`、`locales/*`（新 UI 一律内联中文文案，不动 i18n 文件）、`CaseDetailView.vue`（集成期由主代理加入口）。

## 二、Schema v20（Phase 0 一次性落地）

| 对象 | 变更 |
|---|---|
| `tasks` | `+ defer_until TEXT`（推迟日：到期前从今日/焦点隐藏） |
| `notifications` | 新建：应用内通知中心（id/type/title/body/payload_json/created_at/read_at/dismissed_at），Inbox-Zero 语义：处理即消失 |
| `deadline_rule_audit` | 新建：规则变更留痕（rule_id/action/before_json/after_json/actor/created_at） |
| `links` | 新建：通用跨模块双链（source_type+source_id → target_type+target_id+anchor(页码)+label），**以内部 ID 引用文件，OS 重命名不断链** |
| `persons` + `case_persons` | 新建：对象化实体（kind: judge/client/opposing_counsel/court/contact；preferences 等单一事实源）+ 案件多对多挂载 |
| `smart_rules` | 新建：规则引擎定义（match_field: filename/ocr_text，match_pattern，action_type: tag/move/urgent，action_payload） |
| `case_files` | `+ ocr_text TEXT`（ocr_status 列 v19 已存在：pending/processing/completed/failed） |
| `whiteboards` + `fact_nodes` | 新建：白板（按案件）+ 事实节点（file_id+page+excerpt+坐标，点击回跳出处） |

## 三、新命令契约（Phase 0 预注册，骨架返回空实现）

- **W1**: `get_proposal_preview(proposal_id)`（含前后状态 diff 载荷）；chat 命令扩展 `context_refs` 参数
- **W2**: `defer_task(task_id, until)`、`clear_task_defer(task_id)`；`list_notifications / mark_notification_read / dismiss_notification / dismiss_all_notifications`
- **W3**: `list_deadline_rules / upsert_deadline_rule / delete_deadline_rule / toggle_deadline_rule / list_deadline_rule_audit`（upsert 写审计行 + 触发关联案件期限重算）
- **W4**: `create_link / remove_link / list_links_for / get_backlinks`
- **W5**: `ocr_case_file(file_id)`（系统 tesseract 可用则执行，否则 failed 并说明）、`apply_smart_rules(file_id)`、`run_smart_rules_for_pending`；smart_rules CRUD
- **W6**: `list_persons / upsert_person / delete_person / attach_person_to_case / detach_person_from_case / list_case_persons / list_person_cases`
- **W7**: `list_whiteboards / create_whiteboard / list_fact_nodes / create_fact_node / update_fact_node / delete_fact_node`

## 四、各工作流验收要点

1. **W1（Cursor 式受控 AI）**：AI 写操作 → 前端展示左右红绿 Diff（字段级 前→后）；`Cmd+Enter` 确认后携带一次性 auth_token 调 approve；拒绝/过期有明确状态。`@` 弹出选择器（卷宗文件/知识条目/任务），选中项摘要注入对话上下文，AI 回答标注引用来源。
2. **W2（OmniFocus 式推迟 + Linear 式通知）**：任务可设 Defer Date（日期选择器），defer_until 未到期的任务从「今日/焦点」隐藏并保留在延期分组；通知中心：铃铛+未读数，列表处理一条消失一条（dismiss），无红色焦虑角标堆积。
3. **W3（LawToolBox 式规则）**：设置页可增删改自定义期限规则（触发字段/偏移/顺延方向/适用程序）；每次变更写 `deadline_rule_audit` 并自动重算受影响案件期限；UI 展示规则法条依据。
4. **W4（Hookmark/Obsidian 式双链）**：文书编辑器（tiptap）中可插入指向卷宗文件特定页码的链接；知识条目详情显示「被引用于」反链列表面板；链接全部走内部 ID。
5. **W5（DEVONthink 式静默自动化）**：扫描件导入后自动进入 OCR 队列（后台任务，系统有 tesseract 才执行，否则标记 failed 并给出安装提示）；Smart Rules 设置页可配规则（文件名/OCR 文本匹配 → 打标签/改分类/标急）；规则在文件登记与 OCR 完成时自动触发。
6. **W6（Capacities 式对象）**：`/persons` 视图管理法官/客户/对方律师/法院对象（含 preferences 笔记）；案件详情可挂载人员；人员详情显示其全部关联案件（单一事实源，改一处处处生效）。
7. **W7（LiquidText 式白板）**：案件白板视图：从卷宗文本摘录创建事实节点卡片（含 file_id+page+excerpt），自由拖放布局（坐标持久化），点击卡片跳转文件面板对应文件。

## 五、集成与质量门禁（主代理）

1. 路由登记（`/persons`、`/whiteboard/:caseId`）、设置页挂新区块、案件详情挂入口（人员/白板/期限规则）。
2. `cargo test export_bindings` 重生 `bindings.ts`；新命令登记进 `types/commandMap.ts`（类型双轨收口，不留动态调用尾巴）。
3. 门禁：`npm run test`（vue-tsc + vitest + cargo test）+ `npm run build` 全绿。
4. 代码 review：子代理对抗性评审（正确性/安全/SQL 注入/事务/错误码规范），主代理终审修复。
5. 产出 `docs/global-benchmark-inspirations-walkthrough-2026-09-01.md`。

## 六、风险与备注

- **OCR 本体**：纯 Rust OCR 不现实；采用「系统 tesseract 存在则调用，否则显式 failed + 引导」的诚实降级（符合哲学"无 fake 成功"）。
- **白板无新依赖**：不引 vue-flow，用绝对定位 + pointer 事件自绘卡片画布（符合自绘纪律）。
- **冻结军规**：本批为用户明示的例外；完成后 V5 阶段门禁仍需照常收口。
- 通知中心数据源：复用 reminder 到期检查产生 notifications 行（W2 与 reminder.rs 只读交互，不改动其逻辑）。
