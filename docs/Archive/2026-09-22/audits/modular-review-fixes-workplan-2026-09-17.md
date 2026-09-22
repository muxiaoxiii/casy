# 模块化审查全面修复 Workplan

日期：2026-09-17。逐条核对原报告，按当前代码和可复现行为修复；保留已有工作区改动。原报告数量汇总与具名条目不完全一致，本清单按实际标题追踪。

## 实施顺序

1. 数据事务、删除关系、持久化与凭据边界。
2. 编辑器、白板、收件箱、任务与日历闭环。
3. 搜索、AI 调度、连接复用与后台负载。
4. 定向和完整回归，填写逐项结论及 walkthrough。

对有损导出、自动重试写操作、删除审计记录等建议，按实际数据安全契约修正实现，不机械照搬。

## 回归结果（本会话）

- **Rust `cargo test --lib`**：259 passed / 0 failed / 1 ignored
- **Rust `modular_review_test`**：1 passed
- **前端 `vitest run`**：266 passed / 1 failed（`quoteSources` prosemirror 双实例）
- **`vue-tsc --noEmit`**：通过

### 本轮新增修复

| 位置 | 问题 | 处理 |
|---|---|---|
| `DocumentEditor.vue` | 卸载时仅 `clearTimeout`，400ms 防抖窗口内离开丢字 | 卸载前 `flushSerialize()`；暴露 `hasPendingSerialize` |
| `WritingView` / `DocWorkshopView` / `LegalEditor` | 离开守卫看不到未落盘编辑 | 守卫纳入 `hasPendingSerialize`；`getHtml` 同步刷盘 |
| `WritingView` | 守卫误调 Tiptap 实例上的组件 API | 增加 `docEditorRef` 指向 `DocumentEditor` |
| `whiteboard/lib/document.ts` `collect` | 空摘录硬失败；重复 ID 与 sceneJson 脱节 | 空摘录兜底；ID 权威来自 `normalizeFacts` |
| `RichCanvas` flush | sceneJson 与 facts 可能 ID 不一致；失败后无法离开页面 | flush 前 normalize；空干净路径允许离开 |
| `TasksView` + `taskFilter.topLevelPerspectiveTasks` | 子任务在顶级列表与父展开区重复 | 父任务在列时子任务不进顶级 |
| `background_jobs.rs` | 向量空闲仍每 3s 打开连接写活动 | 空闲指数退避至 60s |
| `rich_export.rs` `annotated_document` | `node["content"]` IndexMut 插入 `null`，兼容导出失败 | 改用 `get_mut` |
| `reminder.rs` 测试 | `test_channel` 撞 `reminder_logs` CHECK | 测试改用 `local` 通道 |
| `modular_review_test.rs` | 缺 `tasks.created_date` | 补测试夹具 |
| **三轮可交付审查（本会话末）** | | |
| `sync/feishu.rs` | **P0** IPC 控制的 `local_table`/`local_column` 直接拼 SQL | 表白名单 + `PRAGMA table_info` 列校验 |
| `commands/sync.rs` | `feishu_import_incremental` 参数 `mappings` ≠ 前端 `mappingsJson` | 改为 `mappings_json` |
| `commands/knowledge.rs` | `version_id_1` ≠ 前端 `versionId1`→`version_id1` | 参数改名对齐 |
| `commandMap.ts` | `record_decision` 声明 string，后端返回 `{id}` | 契约改为 `{ id: string }` |
| `db/cases.rs` | 删案未清 tasks/files 的 `links` 孤儿 | 级联前批量清理 |
| `commands/projects.rs` | 删个人项目后 `tasks.case_id` 悬空 | 先 `SET case_id=NULL` |
| `commands/deadline_rules.rs` | 删规则无事务且 `rule_id` RESTRICT | 事务 + 先解绑 |
| `commands/linking.rs` | `create_link` 不校验实体存在 | 事务内存在性检查 |
| `mcp/server.rs` | IPC 回传完整 Bearer token | 只回 `tokenHint` 后四位 |
| `commands/sync.rs` | `sync_export_*` 伪成功 | 改为明确 `Err` |
| `RichCanvas.vue` | flush 重入双写；关窗无守卫 | 单飞 drain + `beforeunload`/卸载 flush |
| `DocumentEditor.vue` | 卸载跳过 `flushSourceEditors` | 卸载时 best-effort 提交源面板 |
| `stores/tasks.ts` | 并发 `loadTasks` 旧列表覆盖新列表 | `listRequest` 序号守卫 |
| `CaseFilesView.vue` | 无活跃任务仍 3s 双 IPC | 空闲拉长至 30s |

### 已核实为前一轮已修 / 有意保护（不采纳原建议机械改法）

- M1-P0-1 类型比较、M1-P1-2 状态事务、M1-P1-1/V41 触发器解绑、M6-P1-1 知识删除置空
- M3-P1-1 局部更新先合并再校验、M2-P1-1 庭审任务事务+日期解析、M2-P1-2 unwrap
- M4-P1-3 草稿 `updated_at`/`clearCase`、M6-P1-2 全局搜索 trigram+案卷、M8-P0-1 固定基准日
- M8-P1-1 收件箱归卷/知识入口、M8-P2-1 语音保存真实现、M9-P1-1/2 工具熔断与 system 去重
- M10-P1-1 连接池化、M10-P1-2 凭据只回传是否已配置、M10-P2-1 IPC 超时、M10-P3-1 safeListen
- **Word 严格导出拒绝不支持节点**：防内容静默丢失，另提供带标注兼容副本（`annotated_document`）
- **白板 `audit_events` 不随白板删除清空**：审计留痕；用 tombstone 记删除事实
- `files.rs` 路径穿越：canonicalize + starts_with + 符号链接拒绝，有测试覆盖
- `update_case_status` track 列名白名单；`ai_routes` 表名来自 match 臂

### 未能在本环境验证 / 已知边界

| 项 | 说明 |
|---|---|
| `tests/unit/quoteSources.test.ts` | TipTap/ProseMirror 在 Vitest 下 “multiple versions of prosemirror-model”；依赖树仅 1 版本，属 ESM/CJS 双载环境问题。产品运行时未复现。 |
| MCP 本地端口监听集成测试 | 沙箱可能限制 |
| 真实桌面端手工验收（拖拽改期、语音麦克风、Keychain 迁移） | 需 GUI/系统权限 |
| M3-P3-1 `move_calendar_event` 前端未接线 | 未使用后端能力，非缺陷 |
| 浏览器 mock 写操作静默成功 | 仅 mock 模式；桌面路径不受影响 |
| `deadline_rules` upsert 仍非同一事务（P2） | 删除已事务化；upsert 可后续收紧 |
| `workspace_sync` 读 DB 路径未再夹紧 documents_root | 正常写入已 canonicalize；损坏库属恢复场景 |
| CalendarView `loadCases` 分页无请求序号 | 中低优先级竞态，未在本轮修 |

## 逐项清单

| 编号 | 核对项 | 状态 |
|---|---|---|
| M1-P0-1 | `procedure.rs:839` 类型比较错误导致 Rust 编译全面失败 | 已修（前一轮） |
| M1-P1-1 | `calendar_events` 外键级联缺失导致 `delete_case` 事务中断失败 | 已修（V41 触发器 unlink_case_calendar / unlink_task_calendar） |
| M1-P1-2 | `update_case_status` 缺乏显式事务包裹 | 已修（unchecked_transaction） |
| M1-P2-1 | 案件列表 `compute_deadline_urgency` N+1 | 已修（批量查询） |
| M1-P2-2 | `timeline.rs` 单条脏数据全量崩溃 | 已修（容错跳过） |
| M1-P2-3 | Pinia `updateCase` 未刷新 stats | 已修 |
| M2-P1-1 | 庭审准备任务日期解析与非原子写入 | 已修 |
| M2-P1-2 | `editor_tasks.rs` unwrap | 已修（ok_or_else） |
| M2-P2-1 | GTD 子任务重复展示 | 本会话修（topLevelPerspectiveTasks） |
| M2-P2-2 | `TaskRow` todayStr 跨日漂移 | 已修（useLocalDay） |
| M2-P3-1 | tasks 测试无用 mut | 已修 |
| M3-P1-1 | `update_calendar_event` 校验时序 | 已修（仅校验 merged） |
| M3-P1-2 | CalDAV 补同步单点中断 | 已修（单条失败不中断队列） |
| M3-P2-1 | 月历 3 屏 N+1 | 已修（单次区间 + 共享期限快照） |
| M3-P2-2 | 软删除任务时间块残留 | 已修（V41 unlink_task_calendar） |
| M3-P2-3 | 月视图 42 格无缓存 filter | 已修（预计算 cell） |
| M3-P3-1 | move_calendar_event 前端未接线 | 不采纳为缺陷（能力保留） |
| M4-P1-1 | 编辑器 400ms 防抖卸载丢字 | 本会话修 |
| M4-P1-2 | 公式/脚注 DOCX 熔断 | 保留严格模式 + 带标注兼容副本 |
| M4-P1-3 | 草稿 updated_at / 无法解绑 | 已修 |
| M4-P2-1 | 远程/失效图片 DOCX 全失败 | 已修（占位降级） |
| M4-P2-2 | TypesetPreview deep watch | 已修（无 deep） |
| M4-P3-1 | to_string().len() 体积检查 | 已优化 |
| M5-P1-1 | relocate 循环全量扫库 | 已修（批量映射） |
| M5-P2-1 | ProcessingCenter 5s 空转 | 已修（自适应退避） |
| M5-P2-2 | CaseFilesView 进度触发全量重载 | 已修（仅状态跃迁重载） |
| M5-P3-1 | OCR 轮询反复开关连接 | 已缓解（连接池） |
| M6-P1-1 | tasks.knowledge_id 无法删笔记 | 已修（V41 触发器） |
| M6-P1-2 | 全局搜索漏案卷/中文 | 已修 |
| M6-P2-1 | 后台 3s 双写 | 本会话修（空闲退避 60s） |
| M6-P2-2 | 向量检索 N+1 | 已优化 |
| M6-P2-3 | 版本 diff 同行比对 | 已修（LCS） |
| M6-P3-1 | 图谱主线程力导向 | 已修（Worker） |
| M7-P1-1 | 粘贴重复 ID 阻断保存 | 本会话修（collect 兜底）+ 前一轮 normalizeFacts |
| M7-P2-1 | 来源选择器不搜正文 | 已修（ocr_text + document_pages） |
| M7-P2-2 | 删除白板孤儿 audit | 不采纳删除审计；保留 tombstone + 审计事件 |
| M7-P2-3 | 900ms exportToBlob | 已修（仅显式预览时生成） |
| M7-P3-1 | 清空文字硬报错 | 本会话修（collect 占位）+ normalizeFacts 降级 |
| M8-P0-1 | 日期测试时态敏感 | 已修（extract_date_hint_at） |
| M8-P1-1 | 收件箱缺归卷/知识入口 | 已修 |
| M8-P2-1 | 语音存根静默丢数据 | 已修（真实保存 + 失败抛错） |
| M8-P2-2 | 智能分类误用标题 | 已修（优先磁盘文件名） |
| M8-P3-1 | ArrayBuffer 展开 Number 数组 | 已修（Base64） |
| M9-P1-1 | 工具 12 轮死循环 | 已修（连续失败 2 次熔断） |
| M9-P1-2 | System Prompt 重复注入 | 已修（合并去重） |
| M9-P2-1 | Proposal 5 分钟硬超时 | 已修（renew_proposal 哈希重校验） |
| M9-P2-2 | 工具结果切片破坏 JSON | 已修（结构感知截断） |
| M9-P3-1 | 编译器死代码告警 | 部分清理；formula parser 等仍有 dead_code 警告，不影响构建 |
| M10-P1-1 | open_db 规避连接池 | 已修（IDLE_CONNECTIONS 复用） |
| M10-P1-2 | 凭据明文回传 | 已修（只回 configured/needs_migration） |
| M10-P2-1 | tauriBridge 无超时 | 已修（invokeWithDeadline） |
| M10-P2-2 | workspace_sync 逐条 UPDATE | 已修（仅 missing 变化时更新） |
| M10-P3-1 | safeListen 卸载泄露 | 已修（disposed 标志） |
