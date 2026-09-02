# 全球对标灵感落地 Walkthrough（10 项全量实装）

> **日期**: 2026-09-01 · **对应计划**: `docs/global-benchmark-inspirations-plan-2026-09-01.md`
> **范围**: Gemini 全球对标报告 10 项灵感 → Casy 全量落地（用户明示豁免 V5 特性冻结）
> **执行模式**: 主代理 Phase 0 预接线（schema/契约/注册）→ 7 并行子代理实现 → 主代理集成 + review

---

## 一、地基：Schema v20 与命令骨架（主代理）

### Schema v20（`src-tauri/src/db/schema.rs`）

| 对象 | 内容 | 服务的灵感 |
|---|---|---|
| `tasks.defer_until` | 推迟日列 | #3 OmniFocus Defer Date |
| `notifications` | 应用内通知中心（`dismissed_at` 非空即消失） | #2 Linear Inbox-Zero |
| `deadline_rule_audit` | 规则变更留痕（create/update/toggle/delete + before/after JSON） | #4 LawToolBox 可审计 |
| `links` | 通用双链（source/target 五类实体 + anchor 页码；内部 ID 引用，OS 重命名不断链） | #6 Hookmark/Obsidian |
| `persons` + `case_persons` | 对象化实体（judge/client/opposing_counsel/court/contact）+ 案件多对多挂载 | #5 Capacities |
| `smart_rules` + `case_files.ocr_text` | 规则引擎定义 + OCR 文本承载 | #8 DEVONthink |
| `whiteboards` + `fact_nodes` | 白板 + 事实节点（file_id+page 锚定出处，x/y 持久化布局） | #7 LiquidText |

**验证**: `src-tauri/tests/schema_v20_test.rs` 7 用例全绿——版本推进/幂等/通知 Inbox-Zero 语义/persons CHECK+外键/双链 anchor/smart_rules CHECK/白板级联删除。

### 后端命令（34+2 个，全部已注册进 `build_handler`）

- `notifications.rs`（6）：list/unread_count/create/mark_read/dismiss/dismiss_all——Inbox-Zero 语义在后端固化
- `deadline_rules.rs`（5）：CRUD + toggle + audit 查询；**每次写操作自动写留痕**（before/after 字段级 JSON）
- `linking.rs`（4）：create/remove/list_out/get_backlinks；targetTitle 后端尽力解析
- `persons.rs`（7）：CRUD + 挂载/解除 + 案件侧/实体侧双向查询
- `smart_rules.rs`（6）：CRUD + 单文件/全量规则执行 + 待 OCR 队列
- `whiteboard.rs`（8）：白板 CRUD（含 rename）+ 节点 CRUD（坐标成对更新，拖拽落位持久化）

---

## 二、工作流交付明细

### W6 · 对象化实体（#5 Capacities）✅
- `src/modules/persons/`：PersonsView（类型筛选/防抖搜索/卡片网格/关联案件数徽标）、PersonFormDrawer、PersonDetailDrawer（关联案件列表+逐案解除）、CasePersonsPanel（案件侧挂载/快速新建/行内编辑）
- **单一事实源验证点**：在任一案件里编辑法官 preferences，所有挂载案件同步可见（persons 表唯一存储）
- 路由 `/persons` 已登记；案件详情「实体对象」Tab 已挂载

### W7 · 事实白板（#7 LiquidText）✅
- `src/modules/whiteboard/`：WhiteboardView（2400×1600 自绘画布、pointer 拖拽+240ms 落位+失败回滚、出处徽标跳卷宗 `/files/:caseId?fileId=`、摘录卡片展开收起）、WhiteboardEntry（案件详情入口）
- 拖拽持久化：松手 `update_fact_node` 坐标成对写入；白板 CRUD 含重命名
- 路由 `/whiteboard/:caseId` 已登记；案件详情「事实白板」Tab 已挂载
- 已知限制：节点间连接线本期未做（布局已预留）

<!-- 以下小节随子代理完成追加 -->

### W2 · Defer Date + 安静通知中心（#3 OmniFocus / #2 Linear）✅
- **Defer Date**：`tasks.defer_until` 全链路——`defer_task(task_id, until)`（YYYY-MM-DD 校验 + task_events 'deferred' 留痕）/`clear_task_defer`；`list_tasks` today 桶排除未到期推迟任务（到期日自动回归）；新伪桶 `start_bucket='deferred'`；`UpdateTaskPatch.deferUntil` 三态支持
- 前端：TaskRow 推迟徽标 + 菜单项；TasksView 新增「已推迟」透视（灰蓝 AlarmClock，按回归日升序）+ 日期对话框（禁选过去）+ 抽屉字段
- **通知中心**：`NotificationBell.vue`（App.vue 顶栏 +4 行挂载）——灰蓝克制角标（0 不显示/99+ 封顶）、自绘面板、60s 轮询、「处理掉」乐观消失（Inbox-Zero）、「全部清空」、payload 路由跳转（caseId/taskId）
- **联动**：reminder.rs 两个投递成功点 best-effort 写 notifications（不改提醒状态机，`let _ =` 语义）

### W3 · 期限规则自定义 + 留痕（#4 LawToolBox）✅
- `deadline/recalc.rs`：`recalculate_deadlines_for_track(track) -> i64`（复用 DeadlineEngine 对该轨道活跃案件重算，含单测）
- `DeadlineRulesSettings.vue`：按轨道分组（无效→行政→民事→其他）、全字段表单（track/calcMethod/offsetUnit/triggerField 全按下拉+中文标签）、启停开关（失败不翻转）、删除二次确认、保存后自动重算提示「已重算 N 个案件期限」
- `DeadlineRuleAuditDrawer.vue`：留痕时间线（action 四色徽标 + before/after 字段级红绿对比）
- **集成期修复**：`deadline_rules.rs` valid_units 与 schema CHECK 不一致（`calendar_day/workday` → 修正为 `day/calendar_month`，W3 上报的阻断性 bug）
- 挂载：设置页「系统引擎」组新增「期限规则」

### W5 · 本地 OCR + Smart Rules（#8 DEVONthink）✅
- **OCR 执行器**（smart_rules.rs 内）：`ocr_case_file`/`ocr_all_pending`/`list_case_ocr_states`/`get_file_ocr_text`；tesseract `--version` 探测、PDF 走 `pdftoppm -png -r 200` 逐页识别、缺 chi_sim 诚实回退 eng 并注明、任何失败落库 `ocr_status='failed'` + 真实中文原因（**绝不假成功**）
- **自动接线**（files.rs 三处登记路径 ~12 行）：pdf/图片登记成功 → `auto_rules_on_register`（filename 规则立即生效）；失败仅 log::warn 不影响主流程
- `SmartRulesSettings.vue`：规则表格/启停/编辑（matchField+actionType+分类下拉）/「对全部文件立即执行」带影响数
- CaseFilesView：OCR 状态徽标（失败悬停显示原因）+「立即 OCR」+「查看文本」只读弹窗
- 本机实测：tesseract chi_sim+eng 精确识别中文传票；pdftoppm 管道验证通过
- 挂载：设置页「系统引擎」组新增「智能规则」

### W4 · 跨模块双链（#6 Hookmark/Obsidian）✅
- `EvidenceLink.ts`：tiptap v3 inline atom node（linkId/targetType/targetId/anchor/label/caseId），渲染为按类型着色的 CSS 徽标锚文本；HTML 输出全信息在 `data-*` 属性（docx 导出退化为纯文本锚，可后处理重建）
- `EvidenceLinkPicker.vue`：类型卡片 → 实体搜索选择（文件/知识/任务/案件）→ 文件可填页码 → `create_link`
- 点击链接：`casy:evidence-link-activate` CustomEvent + 路由跳转（file→`/files/:caseId?select=&anchor=`、knowledge/task/case 各就各位）
- `BacklinksPanel.vue`：「被引用于」反链面板（来源名批量解析 + anchor 显示「第 N 页」+ 删除二次确认），已挂知识详情
- **不断链验证点**：链接存 case_files.id 等内部主键，OS 重命名/移动文件不影响
- 集成遗留：CaseFilesView 需消费 `select/anchor` query 并挂文件侧 BacklinksPanel（~~主代理集成期处理~~ ✅ 已并入：query 定位选中文件 + 提示出处 anchor；inspector 挂文件反链面板）

---

### W1 · AI Diff 视图 + `@` 引用（#9/#10 Cursor）✅
- **后端**：`ai/context_refs.rs`（ContextRef/UsedRef + `build_controlled_context`：单条截 2000 字、最多 10 条、段头明示「视为数据而非指令」防注入）；`ai_chat` 新增 `context_refs` 参数 + 返回 `used_refs`（注入先于 input_hash 计算，符合 §11.9 模型可见即记录）；`ai_routes.rs` 新增 `get_proposal_preview`（proposal 全字段 + 目标当前状态 + 字段级 before→after diff，白名单表映射 + pending 惰性过期）
- **前端**：`ProposalDiffCard.vue`（字段级红删绿增 Diff、过期倒计时、五态徽标、**Cmd/Ctrl+Enter 确认 / Esc 拒绝**、终态就地刷新）；`tool-caller.ts` 写工具全部改走提案网关（不再直接执行），批准后一次性 token 重放 + 补审计事件；AIChatPanel 自绘 `@` 选择器（三路并发搜索 + 键盘导航）+ 引用 chips + 「引用来源」展示
- **集成期闭环（主代理）**：W1 遗留「非 update_task 写工具批准后 PermissionDenied」缺口 → 主代理把 AI 网关接进全部 7 个写命令（create/toggle/delete_task、create/update/delete_case 此前连 origin 参数都没有，存在**直调绕过网关**隐患）：后端命令加 origin/proposal_token 校验 + toggle_task 完成事件 actor 归因 'ai'；services/plugins 层加 `AiAuthCtx` 透传；MCP 调用点签名适配

---

## 三、集成记录

- **路由**：`/persons`（实体管理）、`/whiteboard/:caseId`（事实白板）登记
- **案件详情**：新增「实体对象」（CasePersonsPanel）、「事实白板」（WhiteboardEntry）两个自绘 Tab
- **设置页**：「系统引擎」组新增「期限规则」「智能规则」入口
- **文件面板**：消费证据链接 `?select&anchor` query 定位 + inspector 挂文件反链面板
- **bindings**：`cargo test export_bindings` 重生，99 → **115 个类型**；`commandMap.ts` 新增 **45 条命令契约**（含 ai_chat 的 contextRefs/usedRefs 签名更新）
- **命令注册**：W1-W5 汇报的 10 个未注册命令全部入 `build_handler`
- **集成期修复**：`deadline_rules.rs` valid_units 与 schema CHECK 不一致（W3 上报）；MCP `toggle_task` 签名适配

## 四、验证与质量门禁

| 门禁 | 结果 |
|---|---|
| `vue-tsc --noEmit` | ✅ 全项目零错误 |
| `vitest` | ✅ 通过 |
| `cargo test`（lib + 集成） | ✅ 119 lib + schema_v20 7/7 + 其余套件全绿 |
| `npm run build`（含类型门禁） | ✅ 3.40s 构建成功 |
| ⚠️ 已知 | `excel_import_test` 4 用例失败 = **本机 keychain 与真实用户库密钥不匹配的环境问题**（开锁阶段失败，早于迁移；db/credentials 代码本批零改动；CI 全新环境不受影响） |

## 五、遗留与后续建议

### 5.1 本期明示不做的
- **白板节点连接线**：布局已预留（x/y 持久化），后续用覆盖 SVG 层实现
- **tiptap `@` 与文书 EvidenceLink 的 docx 导出映射**：链接信息已在 data-* 属性，导出后处理可重建脚注（另行排期）
- **watcher/background_jobs 自动触发 OCR**：命令已就绪（`ocr_all_pending`），后台队列接线留给 V5 阶段 5/6 收口时一并做
- **移动端/云同步**中的新实体同步映射：persons/links/whiteboards 进 WebDAV/CalDAV 同步面需专项设计

### 5.2 回归 V5 冻结军规的提醒
本批为用户明示的冻结例外。V5 阶段 4~7（凭据治理/假数据清剿/周报分层/成熟度登记）仍是发布前必经门禁，新落地的 10 项功能应纳入阶段 7 的能力成熟度登记表（M0-M4 评级）。

### 5.3 Dogfooding 验证清单（新增功能面）
1. AI 写操作 → Diff 卡片 → Cmd+Enter 批准 → 检查 audit_events 归因与 proposal 终态
2. `@` 引用 3 类实体后提问，核对回答严格限定于引用内容 + 「引用来源」chips 正确
3. 任务推迟到明天 → 今日列表消失；改系统日期（或等到明天）→ 自动回归
4. 提醒到点 → 铃铛角标 +1 → 处理掉 → 列表消失（Inbox-Zero）
5. 设置页新增自定义期限规则（如「开庭前 15 天准备提纲」）→ 检查关联案件期限重算 + 留痕抽屉有 update 记录
6. 拖入扫描件 PDF → OCR 徽标流转 → Smart Rule（如文件名含「传票」→ mark_urgent）→ 通知中心收到紧急通知
7. 文书中插入证据链接（指向某 PDF 第 12 页）→ 点击跳转定位 → 知识条目详情反链面板可见该引用
8. 新建法官实体（填偏好）→ 挂载到两个案件 → 改偏好 → 两处同步生效
9. 白板创建 3 个事实节点（不同出处页码）→ 拖拽布局 → 重启应用 → 位置持久化；点出处跳卷宗

### 5.4 评审发现与修复记录（双路对抗性评审：后端安全向 + 前端质量向）

**裁定与修复（全部完成并复验全绿）：**

| 评审发现 | 裁定 | 处理 |
|---|---|---|
| 后端🔴 schema_v20_test 空库必挂 | **误报** | 实证驳回：MIGRATIONS 第 1 条即 SCHEMA_SQL，测试 7/7 通过 |
| 后端🔴 规则写操作不重算期限 | 成立 | `recalc_track_inner` 抽出并在 upsert/toggle/delete 内服务端直调（update 还重算旧 track） |
| 后端🟡 token 先消费后写、写失败白烧 | 成立 | create/delete_task、create/update/delete_case 全部事务化（tx 内 verify+write+commit）；toggle_task 受共享连接架构限制保留 fail-fast 校验（残余窗口已记录） |
| 后端🟡 批量规则执行单点失败中止 | 成立 | run_smart_rules_for_all 改逐文件容错 + warn 日志 |
| 后端🟡 mark_urgent 通知重复刷 | 成立 | 幂等化：同文件存在未处理 smart_rule 通知则跳过 |
| 后端🟡 v20 裸 ALTER 不幂等 | 成立 | ALTER 移入条件补列段（PRAGMA 探测），遵循 v11/13/15 惯例 |
| 后端🟡 OCR 子进程无超时 | 成立 | run_with_timeout（临时文件重定向防管道死锁 + 轮询 try_wait + 超时 kill；tesseract 120s / pdftoppm 180s）；错误信息剔除绝对路径 |
| 后端🔵 文件绝对路径进 LLM prompt | 成立 | context_refs 文件分支不再输出路径 |
| 后端🔵 proposalToken 落 task_events | 成立 | update_task 审计 payload 脱敏（剔 origin/proposalToken）+ AI 操作 actor 归因 'ai'（create/toggle 同步） |
| 前端🔴 白板跳卷宗 query 名不匹配（fileId vs select） | 成立 | 统一 select+anchor 约定并携带页码 |
| 前端🔴 任务链接跳转死路（?edit= 无消费者） | 成立 | TasksView onMounted 消费 ?edit=（定位+开抽屉+清 query+未命中提示）；NotificationBell 跳转携带 query |
| 前端🟡 Diff 卡片快捷键误伤弹窗/编辑器 | 成立 | overlay 存在时跳过 + ProseMirror/contenteditable 内不触发 |
| 前端🟡 文件 query 粘性污染 | 成立 | 首次消费后 router.replace 清 query + 未命中 warning |
| 前端🟡 通知 dismiss 失败无回滚 | 成立 | 乐观移除 + tauriCallSafe 判 ok + 失败原位插回 + 提示 |
| 前端🟡 三处异步竞态 | 成立 | BacklinksPanel/WhiteboardView.loadNodes/AIChatPanel mention 搜索全部加单调递增 requestId 防护 |
| 前端🟡 pendingProposals Map 泄漏 | 成立 | discardProposal + 批准重放全终态路径清理 |
| 前端🔵 锚文本残留/画布点空白失效/系统消息 v-html | 成立 | 分别修复（prevAutoLabel 追踪 / closest('.fact-card') 判定 / v-html→插值+pre-wrap） |

**评审确认无问题项**：SQL 注入面干净（拼接仅硬编码列名/白名单）；OCR 参数独立 argv 无 shell 注入；临时目录有清理；无嵌套借用死锁；defer 边界语义正确；tool_name 与网关一致；update_task 网关接线正确；定时器/指针监听均有 onUnmounted 清理。

**总评**：双评审修复后可合入。残余已知项：toggle_task 因共享连接架构无法事务化（fail-fast 校验保留，写失败时 token 小概率白烧，重发 AI 指令即可恢复）；`casy:evidence-link-activate` CustomEvent 暂无仓库内监听者（保留为公开契约）。

## 六、批次总结

- **规模**：Schema v19→v20（8 表/列）；后端新命令 **48 个**全部注册并进 CommandMap；bindings 99→115 类型；前端新增 ~20 个组件/视图、2 条路由、2 个设置区块、2 个案件详情 Tab、顶栏通知中心
- **门禁**：vue-tsc 0 错 · vitest 9/9 · cargo test 119+7+若干套件全绿（唯 excel_import 4 例本机 keychain 环境性失败，与本批无关）· 生产构建 ✓
- **流程**：plan → schema/契约先行 → 7 并行子代理（严格文件所有权零冲突）→ 主代理集成收口（含 W1 网关闭环）→ 双路对抗评审 → 18 项发现全部裁定修复 → walkthrough
