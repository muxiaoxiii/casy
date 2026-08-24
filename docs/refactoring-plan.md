# Casy 重构方案（基于设计哲学的重新规划）

> **版本**: v1.7（K-3 归因落地 · AI UI 验证三层体系 · D-15 修订：mock 上移服务层）
> **日期**: 2026-08-21
> **状态**: 待评审
> **上位文档**: `docs/casy-design-philosophy.md`（唯一总纲）
> **v1.1 变更**: 依据用户裁决调整产品主线——**先把 Casy 做成好用的个人任务管理+GTD+日程软件，其次才叠加律师工作台**。路线图从单线 R0-R4 改为 A（产品手感）/B（平台地基）/C（律师业务冻结）三线。
> **v1.2 变更**: 依据用户要求新增 **A-UI 工作流**——UI 全面升级，目标「流畅、灵动、好用」。定位：**流畅是性能预算问题；灵动是有目的的动效系统**——与哲学"克制即优雅"不冲突，对标 Things3 的完成动画与 Linear 的键盘流（motion 服务于理解，而非装饰）。
> **v1.3 变更**: ①吸收内核评审结论，B 线新增 **B3 内核收口**（K-1～K-4），并新增 §八执行排期给出与 A 线的交汇顺序——核心结论：K-2（ServicesMap/defineTool）是 A1-1 的最佳前置；②A1-1 实现形态细分为决策点 **D-9**（泛化＋垂直拆表 vs 单宽表），D-5"单一顶级实体"结论维持不变；③应评审意见将 §七克制清单改为**三级制**（外部资源红线／有条件重开／原则保留），除移动端与自建云中继外逐项给出再评估、重开条件与成本。
> **v1.4 变更**: 目标升级为生产级、可商业出售。B 线新增 **B4 生产化与发布工程**（R-1～R-8：应用身份/updater/签名公证/CI/安全加固/数据保障/崩溃上报/合规包/授权体系），其中 **R-1 bundle identifier 为不可逆决策，须在任何用户数据落盘前定案**；新增决策点 D-11（商业形态与目标市场——同时决定 C 线解冻策略）、D-12（签名与更新预算）、D-13（遥测与隐私立场）；§八排期第 1 周插入 R-1。
> **v1.5 变更**: 三项裁决落定并执行——①**D-9 ✅ 泛化＋垂直拆表**：project 精简主表 + `case_legal_details` 侧表；②**K-3 ✅ 归因写既有 audit_events**：`actor='ai'` 已在 schema CHECK 枚举中，payload 携带 run_id 关联 ai_runs（ai_runs 记过程、audit_events 记结果），无需新表；③**R-1/D-14 ✅ identifier 定案 `top.muxiaoxi.casy`** 并已写入 tauri.conf.json——identifier 为永久唯一字符串，与域名是否续费解耦；④**D-11 ✅ 目标市场=个人法律人士**（执业律师/法务/实习律师/助理），不做企业级：商业模式建议买断制+BYOK AI（无持续云服务成本支撑订阅）、R-8 定为离线许可+设备绑定、C 线解冻维持"A 达标后"，收件箱 AI 分类/期限引擎列为解冻首批付费差异化项。§八第 3 周裁决会缩减为仅裁 D-3(ts-rs)。
> **v1.6 变更**: ①**D-3 ✅ 裁决（用户确认一步到位，不做 ts-rs 中间试点）**：直接引入 specta 生态——全部命令 DTO 加 `#[derive(specta::Type)]` 构建期导出 TS 到 `src/types/bindings/`，CI `git diff --exit-code` 防过期，tauriBridge 增加 CommandMap 泛型映射使 202 个 Tauri 命令的参数/返回获得类型检查（tasks/calendar/inbox 三域先行，随 B1 落地）；②新增 **D-15 调用通道边界**：tauri-specta 的绑定直连（前端生成函数绕过 bridge）与浏览器 mock 预览层、双路径铁律冲突，默认排除出路线图——除非未来裁决放弃纯前端预览模式；③R-1 验证通过：release 构建产出 Casy.app / DMG / updater 签名产物（.tar.gz + .sig），新 identifier 生效；密钥管理方式记入 devlog。
> **v1.7 变更**: ①**K-3 ✅ 已落地**：事件层两个真实消费者跑通——`task:completed → reminder_recompute_now`（新 Rust 命令，事件驱动重算不等周期）、`tool:executed → audit_events`（新命令 record_ai_tool_audit，actor='ai'，turn_id 关联工具循环、digest 仅参数键名脱敏 §11.9）；原定消费者「case:updated→时间线刷新」因时间线属冻结 C 线域（前端无消费点）改为解冻后落地。②**AI UI 验证三层体系（回应用户裁决：放弃浏览器预览依赖可以，但 AI 必须能自检 UI——UI 是本软件关键资产）**：L1 类型层=vue-tsc+specta 单源（已有）；L2 快速视觉层=B2-T1 服务层 FakeService 注册表+Vite 浏览器模式+Playwright 截图供多模态 agent 读图评审（秒级迭代，无需 Rust 编译）；L3 真相层=R-9 tauri-driver E2E 冒烟+截图回归（真实后端）。③**D-15 修订**：mock 从 bridge/IPC 层上移到 Service 接口层后，绑定直连不再有架构障碍，但边际价值低（CommandMap 已覆盖），维持不采用。
> **调研基点**: commit `187561f`（rebase 暂停检出）+ `61bec8c`（main 完整线谱）+ 两轮子代理审计（代码现状 / GTD 体验差距）

---

## 〇、接手诊断摘要

### 0.1 仓库处于 pull --rebase 中断态（一切动工的前提）

reflog 还原：`61bec8c`（main 终点，schema v14 / ctx 服务通路 / 96 测试）之后一次 `git pull --rebase` 把 27 个本地提交向旧远端基点 `0725361` 重放，完成 6 个后中断；**剩余 21 个未重放提交包含全部最先进成果**（后端全量接线、插件内核真实实现、cordis 服务注入、视图迁移收口、Forecast 日历、时间模型 due_time、MCP/CalDAV）。暂停期间又有人在其上叠加了 9 个基于旧代码的提交（Slate 主题、GTD 七透视等）。

**关键安全事实**：本地 `main` 与 `origin/main` 都仍指向 `61bec8c`，无丢失风险。已建双备份分支 `rescue/paused-work`(187561f) 与 `rescue/main-full`(61bec8c)。恢复步骤见 B0。

### 0.2 两个版本共有的真实工程债

- 后端：无 domain 层、commands 直写 SQL ×267、每命令新开 SQLCipher 连接 ×188（PRAGMA key 每次重跑）、错误全部折叠 String、双路径路由/Confirmer 只有查询接口没有强制执行点、集成测试 0 条
- 前端：TS 化残留 ~1400 行 JS、四个千行级视图、112 处 emoji、build 不跑 vue-tsc、无 lint/test 门禁

### 0.3 GTD/日程体验差距（对标 Things3/OmniFocus/Fantastical，2026-08-21 审计）

**已具备的骨架（值得肯定）**：7 内置透视+自定义透视；多通道捕获（全局热键 Cmd+I/E/N/T、Cmd+Shift+V 剪贴板、托盘、文件夹监听）；收件箱 AI 推荐+一键落地；日历月/周/日/Forecast 四视图+任务拖拽到时间格改期；snooze 预设；厘清（triage）流程。

**关键缺口（按对手感伤害排序）**：

| # | 缺陷 | 证据 | 影响 |
|---|------|------|------|
| 1 | **完成任务必弹"实际耗时"弹窗** | TasksView.vue:340-348 | 违背一键完成铁律，每天几十次打断 |
| 2 | **零乐观更新**：所有变更 await IPC 后整表重拉 | TasksView 等 8 处 | 操作跟手性差，本地优先却感觉像远程应用 |
| 3 | **日历无独立事件实体**：事件全是案件期限/庭审的 JOIN 投影；"快速建日程"实际只进收件箱（App.vue 内注释自认 TODO） | commands/calendar.rs 全文；App.vue saveQuickCapture | 无法安排与案件无关的生活/工作日程，Fantastical 式排期不可能 |
| 4 | **周视图时间轴缺时间字段**：后端事件结构无 time，开庭/口审在周视图不可见 | calendar.rs:10-17 + CalendarView.vue:791 过滤 !e.time | 日历核心信息缺失 |
| 5 | **零系统通知**：tauri notification 插件已注册但全项目无一处调用 | grep plugin-notification = 0 命中 | 提醒只在应用内存在，离开窗口即失效 |
| 6 | NL 自然语言解析**三套重复实现且互不一致**；解析出的时分从不写入 dueTime（恒 null）；startBucket 写入非法值 'upcoming' | TasksView:724 / App.vue:256 / CalendarView:388；types/index.ts:258 | 捕获时说"下午3点"无效 |
| 7 | **无重复任务**、**无子任务/检查项** | grep rrule/subtask = 0；schema 无对应列 | 律师日常大量例行事项无法承载 |
| 8 | Review 默认值缺陷：新任务 next_review_date=下周日 | tasks.rs:113-120 | 周日一到回顾透视被全部未完成任务淹没 |
| 9 | ⌘K 全局搜索是摆设输入框；Areas 无管理 UI；无 undo | App.vue:417-421 等 | 组织层残缺 |

**耦合度结论**：任务链路 case_id 可空，**可以**作为纯个人 GTD 使用 ✓；但日历、顶栏今日统计、blocked 语义、Onboarding 强耦合案件实体 ✗——这正是"先把 GTD 做好"要拆的耦合。

---

## 一、产品主线：三条线的裁决

> 哲学依据：§1.4「任务管理比案件管理更长远更重要——任务是软件每天被打开的理由，案件是每次打开的上下文」。用户 v1.1 裁决与此完全一致，落地为：

```
A 线 · 产品手感（最高优先）── 让 Casy 作为纯个人 GTD+日程好用
    ├── A-FLOW 工作流：不打断 / 组织 / 日历一等公民（M-GTD-1/2、M-CAL-1）
    └── A-UI 工作流：流畅·灵动·好用（M-UI-0 起步，随各里程碑逐表面推进）
B 线 · 平台地基（服务 A，裁剪范围）── git 恢复 + 门禁 + 三域 domain 收口
C 线 · 律师业务（冻结新功能）── 案件/案卷/飞书保持现状可用，A 线达标后解冻
```

执行规则：B0 必须最先做（否则一切开发建立在错误检出上）；此后 A/B 交替推进——每个 A 线里程碑动工前，先补齐其依赖的 B 线项；C 线冻结期内只修致命 bug。

---

## 二、A 线：GTD+日程手感（产品主线）

### M-GTD-1 「不打断」（目标 2 周）

| # | 任务 | 验收 |
|---|------|------|
| A0-1 | 完成一键化：去掉完成时的耗时弹窗阻断；改为完成后轻提示"记录耗时？"（点击展开），并在设置提供"完成时询问耗时"开关（默认关） | 完成任务全程 ≤1 次交互，零弹窗 |
| A0-2 | 乐观更新：toggle/snooze/moveToToday/delete 先改本地再发 IPC，失败回滚并提示；消灭操作后整表重拉 | 操作即时响应；IPC 注入延迟 500ms 仍跟手 |
| A0-3 | Undo 栈：complete/delete/snooze 三类操作支持 Cmd+Z 撤销 | 三类操作均可撤销 |
| A0-4 | 系统通知接通：due_time 到点发系统通知（复用已注册的 notification 插件）+ 应用内 toast 双通道；托盘图标显示今日到期数 | 到点通知在窗口失焦时可达 |
| A0-5 | NL 解析统一：合并三套实现为 `src/shared/nlp/` 单模块；修复 dueTime 恒 null、非法 startBucket；支持"明天下午3点""周五""3天后" | 捕获含时间的文本 → 时间正确落到 due_time |

### M-GTD-2 「组织得起来」（目标 +2 周）

| # | 任务 | 要点 |
|---|------|------|
| A1-1 | 个人项目建模（决策点 D-5） | 推荐：cases 表泛化为 project（加 `kind: legal/personal`），律师案件是 kind=legal 的特例——符合哲学"4 元信息"不新增顶级实体；拒绝平行 projects 表 |
| A1-2 | Areas 管理 UI | 现有 areas 表补 CRUD 界面 |
| A1-3 | Today 拖拽排序 | today_index 字段已有，vue-draggable-plus 已在依赖中，接通即可 |
| A1-4 | 子任务/检查项 | schema 加 `parent_task_id`（保持 SQL 可查询），任务详情内联增删勾选 |
| A1-5 | 重复任务最小集 | RRULE 子集：每天/工作日/每周X/每月X日；完成后生成下一实例 |
| A1-6 | ⌘K 全局搜索接通 | FTS5 已有；结果分栏：任务/项目/知识；键盘上下选择回车跳转 |
| A1-7 | Review 默认值修复 | next_review_date 仅在用户显式设置回顾周期时写入；回顾透视加"本周已完成"正向区 |

### M-CAL-1 「日程是一等公民」（目标 +2 周）

| # | 任务 | 要点 |
|---|------|------|
| A2-1 | 独立日历事件实体 | 新表 `calendar_events`（title/start/end/all_day/category: personal/legal）；这不是违背"数据有限"——日程本是独立事实源，不是案件投影 |
| A2-2 | 周视图时间轴完整 | 修复后端事件结构无 time 的缺陷；开庭/口审（案件层）与个人事件分层渲染 |
| A2-3 | 捕获直达日程 | Cmd+E 真正创建事件（当前只进收件箱的 TODO 兑现）；NL 建日程 |
| A2-4 | Forecast 打磨 | 左月网格+右选中日时间块（哲学 §7 蓝图核对）；拖拽任务⇄时间格双向 |
| A2-5 | 案件层降级为只读图层 | 期限/庭审作为图层渲染在日历上，个人事件独立 CRUD——完成"GTD 先行、法律后挂"的结构解耦 |

### A-UI 工作流：流畅 · 灵动 · 好用

> 设计北极星：Things3 的完成动画、Linear 的键盘流、Fantastical 的日历手势。
> 三条纪律：①动效必须有目的（表达状态变化/空间关系/因果），拒绝装饰性动画；②全部 GPU 友好属性（transform/opacity），交互路径禁 long task；③尊重 `prefers-reduced-motion`。

#### M-UI-0 动效与视觉地基（与 M-GTD-1 并行起步）

| # | 任务 | 要点 |
|---|------|------|
| U-1 | Design Tokens v2 定版 | 以 rescue 里的 Slate 石墨主题为基线评审定版：色彩（语义色+中性色阶）/字阶/间距韵律/圆角/阴影/焦点环，收敛为 CSS variables 单一来源；暗色主题变量预留 |
| U-2 | Motion Tokens | 时长三档 120/180/240ms + 统一缓动（ease-out-quart 主曲线、spring 用于拖拽落位）；全局 `motion.css` 工具类 + `<Transition>` 预设封装 |
| U-3 | 核心表面自绘组件库启动（决策点 D-8） | TaskRow / TaskList / PerspectiveTabs / QuickCapture / CalendarGrid / TimeGrid 六个核心表面完全自绘；Element Plus 仅保留表单/设置/弹窗类页面——生产力核心界面要有 bespoke 手感 |
| U-4 | 键盘流基建 | 全局 focus 管理与快捷键注册中心；j/k 移动选择、Space/x 完成、E 排程、数字键切透视；焦点环样式统一 |

#### 随里程碑推进的 UI 项

| 伴随里程碑 | UI 交付 |
|-----------|---------|
| M-GTD-1 | **完成动画**（圆形填充+划线，Things3 式）；捕获面板弹出/收起过渡；toast 与 undo 提示动效规范；乐观更新的即时视觉反馈（pending→confirmed 微动效） |
| M-GTD-2 | 列表 FLIP 重排动画（拖拽排序/透视切换位移过渡）；子任务展开收起；⌘K 面板出入场 |
| M-CAL-1 | 日历拖拽手感：拖起 lift 阴影、目标格高亮呼吸、落位 spring 回弹；周视图时间块拖拽创建事件的划线手势 |

#### 流畅的性能预算（"流畅"的本质是性能）

| 指标 | 预算 |
|------|------|
| 点击到视觉响应 | <100ms（本地 SQLite 操作应 <10ms） |
| 启动到可交互 | <1.5s；今日面板数据并行加载 |
| 交互路径 long task | 无 >50ms 任务（DevTools performance 录制验收） |
| 大列表 | >200 条虚拟滚动 |
| 动效帧率 | 60fps；仅 transform/opacity 合成 |

#### UI 验收清单

- [ ] 完成任务有完成动画；列表重排有 FLIP 过渡；面板出入场一致
- [ ] 捕获→今日排三个任务→完成→回顾，全流程不碰鼠标
- [ ] `prefers-reduced-motion` 下所有动效优雅降级
- [ ] 核心表面无 Element Plus 默认蓝残留；A 线范围内 emoji=0
- [ ] 拖拽全程 60fps，落位有 spring 回弹

### A 线总验收（"好用"的可操作定义）

**Dogfooding 清单**：连续 7 天把 Casy 当作唯一任务管理工具真实使用，期间以下任一条不成立即打回：
1. 完成一个任务永远是一次交互
2. 捕获一条含时间的想法 ≤5 秒且时间正确
3. 日历上能安排一次与案件无关的日程并准时收到系统提醒
4. 周一早晨能在 10 分钟内完成本周回顾（不被噪音淹没）
5. 全程不碰任何案件相关功能也毫无违和

---

## 三、B 线：平台地基（从原 R0-R4 裁剪，服务 A 线）

### B0 — 仓库止血 + 工程门禁（≈3 天，最先执行）

```bash
# ① 备份分支已建：rescue/paused-work(187561f) / rescue/main-full(61bec8c)
# ② 中止 rebase → main 回到 61bec8c（本地=远程，无需强推）
git rebase --abort
# ③ 逐个评审 rescue/paused-work 的 9 个提交择优 cherry-pick：
#    预计吸收：Slate 主题(83bbd48)、STATUS-v4(7469c96)、安全清理(0ffd1e0)
#    对撞评审：GTD 七透视改动 vs 61bec8c 的 OmniFocus 版取优
#    预计丢弃：调试标记(7c51425)；冲突原则：架构以 61bec8c 为准
# ④ 绿基线验证：npm run build && cd src-tauri && cargo test
```

工程门禁当天生效：scripts 补 `typecheck/lint/test`，build = `vue-tsc && vite build`，`cargo clippy -D warnings`；文档收口（STATUS 以 61bec8c 重写为单一 `Casy-STATUS.md`、修复悬空引用、README 同步）。

### B1 — 后端收口（裁剪为三域，随 A 线穿插）

范围缩减为 **tasks/calendar/inbox 三域**（A 线战场）：
1. `errors.rs`：CasyError(thiserror) + `{code,message,details}` 错误码，替换 `Result<_,String>`
2. 连接池：`State<DbPool>` 托管，废除三域内散落 open_db（其余域暂缓）
3. 三域 DomainCommand：事务内完成 落库 + audit_events + 缓存字段维护；calendar_events 新表迁移走此通道
4. 类型化契约：优先 ts-rs codegen（决策点 D-3），先覆盖三域 DTO

feishu/sync/docsy/cases 域的原 R1 内容（上帝文件拆分等）**移交 C 线解冻后执行**。

### B2 — 前端收敛（随 A 线穿插）

- TS 化收官：main/router/composables → .ts，strict 进 build
- 数据通路单一化：视图→ctx 服务→bridge→Rust；Pinia 只留 UI 态（决策点 D-4）
- 千行视图拆分与 emoji 整改：**先只做 tasks/calendar/inbox/App 外壳范围**，其余随 C 线解冻
- **B2-T1 服务层 FakeService 注册表（v1.7 新增，AI UI 验证 L2）**：mock 从 bridge/IPC 层上移到 Service 接口层——Fake 实现与真服务同接口、bootstrap 按 runtime 选择注入，生产调用链零 mock；现有 mockData 资产转为 Fake 数据源。Vite 浏览器模式 + Playwright 截图 → 多模态 agent 读图评审，秒级 UI 迭代无需 Rust 编译

### B3 — 内核收口（v1.3 新增，前端内核，服务 A/B 全线）

> 来源：ctx 插件内核全量评审（TS 62 错清零后的结构性结论）。四项均为行为等价重构（K-3 归因除外，属新增审计能力），可与 A 线穿插推进；**K-2 是 A1-1 的前置**（见 §八交汇规则）。

| # | 任务 | 要点 | 规模 |
|---|------|------|------|
| K-1 | 确认策略上收（安全收口） | `CasyTool` 增加声明式 `policy { write, level, title }`；`executeTool` 统一计算 effective_level 并强制 `requestConfirm`——AI 写操作的确认从"各工具自觉"变为"内核强制"，删不可漏；删除 cases(L3)/files(L2)/knowledge(L2)/sync(L2×2) 四插件的手写确认块 | 0.5d |
| K-2 | 类型双轨合一 + 单一事实来源 | 新增 `defineTool<P>()`：把 JSON Schema 与参数类型绑定，插件内 37 处边界断言收敛为全局唯一一处；导出 `ServicesMap`，ctx 属性声明/provide/inject 全部 keyof 化——此后 `ctx.cases ⇒ ctx.projects` 的改名是单点改动 | 1d |
| K-3 | **事件激活 + AI 归因 ✅ 已落地（v1.7）** | 两个真实消费者跑通：①`task:completed → reminder_recompute_now`（新 Rust 命令，事件驱动重算不等引擎周期）；②`tool:executed → record_ai_tool_audit` 写既有 audit_events（`actor='ai'`，turn_id 关联工具循环、digest 仅参数键名脱敏 §11.9；与 ai_runs 外键关联待 ai_chat 返回结构化 run_id 后替换）。原定「case:updated→时间线刷新」属冻结 C 线域，解冻后落地 | 1d |
| K-4 | **内核修缮 ✅ 已落地（v1.7）** | ①`fork()` 以包装对象替代实例方法 monkey-patch（dispose 先出栈再释放，行为等价）；②`plugin()` 安装前快照 services/tools 键集，失败时按快照回滚——新 provide 的服务走 unprovide 执行 dispose 清理、新注册工具删除，部分安装不留残骸；③skills 取舍采纳推荐项：新建 `src/core/skills/today_briefing`（只读聚合当日任务/日程/预警），registerSkill→executeSkill 链路打通 | 0.5d |

验收纪律：每项收尾 `vue-tsc --noEmit` 零输出 + `npm run build` 绿；K-2 完成判据：`grep -c "as string\|as Record" src/core/plugins` 归零。

### B4 — 生产化与发布工程（v1.4 新增：可商业出售的门槛）

> 目标升级为生产级、可商业出售。盘点结论（2026-08）：`identifier` 仍为脚手架默认值 `com.tauri.dev`、无 updater、无签名公证、无 CI、CSP 为 null、无 LICENSE/EULA。其中 **R-1 不可逆**——bundle ID 决定签名身份、更新通道与用户数据目录路径，任何真实用户数据落盘前必须定案，否则日后更改等于换了一个应用。

| # | 任务 | 要点 | 规模 |
|---|------|------|------|
| R-1 | 应用身份与更新基线（不可逆项，最优先） | 定案 bundle identifier（D-11 关联）；semver 版本策略；接入 tauri-plugin-updater + 更新签名密钥对；更新源用静态托管（GitHub Releases / 对象存储）——属分发通道，不触碰"自建云中继"红线 | 1d |
| R-2 | 签名与公证 | macOS Developer ID + notarytool 流程；Windows Authenticode 证书采购与签名脚本；未签名产物在 Gatekeeper/SmartScreen 直接被拦 | 1d＋证书采购 |
| R-3 | CI 流水线 | GitHub Actions 三平台矩阵：typecheck + lint + cargo test + build + bundle；tag 触发发布流水线（草稿 Release + 签名产物 + 更新清单） | 1d |
| R-4 | 安全加固 | CSP 从 null 改白名单；cargo audit / npm audit 进 CI；capabilities 复核（现状已最小化✓） | 0.5d |
| R-5 | 数据保障 | 备份/恢复向导（本地优先应用的"不丢数据"承诺）；诊断包导出（日志+版本+脱敏配置），降低支持成本 | 2d |
| R-6 | 崩溃上报 | 本地崩溃日志兜底必做；上报走 opt-in（随 D-13 隐私立场定） | 1d |
| R-7 | 合规包 | EULA / 隐私政策文本；第三方许可清单（cargo-about 类工具）；**AI 云端模式的保密披露声明**——律师场景下这不是负担而是卖点（K-3 归因审计同此） | 1d |
| R-8 | 授权体系 | 离线许可证密钥 + 设备绑定 + 宽限期；或 BYOK 免费 + Pro 分层。**依赖 D-11 商业形态裁决后才动工** | 3d |
| R-9 | **E2E 冒烟与截图回归（v1.7 新增，AI UI 验证 L3）** | tauri-driver（WebDriver）驱动真实应用：启动→导航→断言→截图；作为 AI 改动后的真相校验与发布前回归，随 R-3 CI 就位后接入。三层体系：L1 类型层（vue-tsc+specta）→ L2 快速视觉层（B2-T1 Fake+截图）→ L3 真相层（本项） | 2d |

门槛提升效应（对既有项）：K-3 归因审计 → 销售卖点；B1 错误码 → 支持成本问题；A-UI 性能预算 → 商业口碑约束；乐观更新三态一致性 → 数据完整性承诺。B4 不抢 A 线带宽：仅 R-1 提前插入第 1 周，其余挂在 M-GTD-2 之后、首个对外 beta 之前集中执行。

---

## 四、C 线：律师业务（冻结期政策）

- **冻结范围**：案件三轨状态机、期限引擎、飞书同步、WebDAV、文书工坊、收件箱 AI 分类——不再新增功能，只修致命 bug
- **保留义务**：现有数据兼容不受 A/B 线迁移破坏（每次迁移跑案件域回归用例）
- **解冻条件**：A 线 Dogfooding 清单通过
- **解冻后的方向预告**：案件作为 kind=legal 项目回到视图层；期限引擎接入提醒通道；飞书同步续建；CalDAV（caldav.rs 雏形已在库）；**docsy_engine 模块更名**（消除与产品名的同名混淆，触及案件域导入链路故随 C 线一并做）；原 R3/R4（M0 验收取证、Confirmer 强制点、推荐引擎、报表、蒸馏）按哲学 §13 恢复推进

---

## 五、关键决策点汇总

| # | 决策 | 建议 |
|---|------|------|
| D-1 | 完成 git 恢复（abort + cherry-pick）授权 | 建议：授权。备份分支已建，全程可逆 |
| D-2 | rescue 9 提交吸收尺度 | 视觉/文档吸收；任务视图改动对撞评审取优；调试提交丢弃 |
| D-3 | **DTO codegen ✅ v1.6 裁决：specta 类型模式一步到位** | 全部命令 DTO 加 `#[derive(specta::Type)]` 构建期导出 TS 到 `src/types/bindings/`；CI `git diff --exit-code` 防类型过期；tauriBridge 增加 CommandMap 泛型映射，使 202 个 Tauri 命令的参数/返回获得编译期检查（tasks/calendar/inbox 三域先行，随 B1 落地）。不做 ts-rs 中间试点（避免二次迁移）；绑定直连见 D-15 边界 |
| D-4 | Pinia store 去留 | 最小集保留（UI 态/偏好），数据态全面走 ctx 服务 |
| D-5 | **个人项目建模（v1.1 新增，影响 schema）** | 推荐：cases 泛化为 project 加 kind 字段；备选：新建平行 projects 表（违反"数据有限"，不建议） |
| D-6 | 耗时弹窗处置 | 设置开关默认关 + 完成后轻提示（保留学习闭环数据来源，见哲学 §11.6） |
| D-7 | 日历独立事件表 | 引入 calendar_events（日程是独立事实源）；案件期限/庭审保留为只读图层 |
| D-8 | **Element Plus 保留范围（v1.2 新增）** | 推荐：六个核心 GTD 表面自绘（TaskRow/List/PerspectiveTabs/QuickCapture/CalendarGrid/TimeGrid），EP 留在设置/表单/弹窗；备选：全面替换 EP（工程量 ×3，不建议）；保守：全部 EP 换肤（灵动上限低） |
| D-9 | **项目表实现形态（v1.3 细分 D-5）✅ v1.5 裁决：泛化＋垂直拆表** | project 精简主表承载通用列，`case_legal_details` 侧表承载法律专列，兼得"单一顶级实体"与 DTO 整洁（ts-rs 生成 Project 类型不被 ~40 个法律列污染；个人项目行不背空栏）。裁决理由：主流场景是个人/生活项目 + DTO 洁净直接决定前端类型质量。迁移注意 SQLite DROP COLUMN 限制，用重建表方式，随 A1-1 落地 |
| D-10 | 动效微库预算（v1.3） | 建议：CSS Transition/WAAPI/FLIP 优先；出现确证覆盖不了的动效时允许引入 <10KB gzip 微库（motion-v 类）；GSAP/Motion One 维持排除 |
| D-11 | **商业形态与目标市场（v1.4 新增）✅ v1.5 裁决** | 目标客户=**个人法律人士**：执业律师、法务、实习律师、律师助理；不做企业级。连锁确定项：①C 线解冻维持"A 达标后"，收件箱 AI 分类/期限引擎列为解冻首批付费差异化项；②商业模式建议买断制+BYOK AI（本地优先无持续云成本，订阅无价值支撑）；③R-8 授权体系=离线许可密钥+设备绑定+宽限期；④企业级功能（多租户/SSO/团队权限）不进入射程，WebDAV/飞书定位个人场景 |
| D-12 | 签名与更新预算（v1.4） | 建议：接受 Apple Developer（$99/年）+ Windows 代码签名证书（数百刀/年）成本；更新源静态托管（GitHub Releases 或对象存储）。未签名=商业不可售 |
| D-13 | 遥测与隐私立场（v1.4） | 建议默认：零遥测、崩溃日志仅本机、上报严格 opt-in 且可一键关闭——本地优先+保密是卖点，立场要写进隐私政策 |
| D-14 | **应用标识符（v1.4 提出）✅ v1.5 定案并已执行** | `identifier = top.muxiaoxi.casy`，已写入 src-tauri/tauri.conf.json。说明：identifier 是借反向域名惯例保证唯一的永久字符串，与应用域名是否续费**解耦**——签名/更新/数据目录均不要求域名可解析；更新源 URL 独立可迁移。发布后永不再改 |
| D-15 | **前端调用通道边界（v1.6 提出，v1.7 修订）** | 用户裁决：放弃浏览器预览依赖可以，但必须保证 AI 能自检 UI。修订：①mock 从 bridge/IPC 层**上移到 Service 接口层**（FakeService 同接口、bootstrap 选择注入）——bridge 保持唯一写入口，生产链零 mock；②绑定直连因此不再有架构障碍，但边际价值低（CommandMap 已覆盖），维持不采用；③AI UI 验证走三层体系（B2-T1 + R-9） |

---

## 六、风险与回滚

| 风险 | 缓解 |
|------|------|
| git 操作误伤 | 双 rescue 分支已兜底；abort 可逆；cherry-pick 冲突一律人工裁决 |
| A 线改动触碰案件耦合点（今日统计/blocked 语义） | 每处解耦先补案件域回归用例再动手；C 线冻结期发现破坏立即回灌修复 |
| 乐观更新与 IPC 失败不一致 | 统一封装 optimistic wrapper：pending/confirmed/rolled-back 三态 |
| calendar_events 新表迁移风险 | 走既有 user_version 迁移框架；迁移前自动 .bak |
| 门禁上线暴露存量问题 | 一次性清理或显式 allow 登记，禁止静默忽略 |
| 文档再次失实 | devlog 规矩：每里程碑同步 STATUS，"已完成"必须附验证证据 |

---

## 七、边界决策三级制（v1.3 改版，回应"过严"评审意见）

> 原"明确不做的事"重组为三级。**红线**只保留引入外部资源/新产品形态的两项；其余各项转为**有条件重开**（附条件、时机与成本）或**原则保留**（举证责任规则，而非禁令）。重开不需要推翻本文档：满足条件即自动生效。

### 7.1 红线（维持绝对不做）

- ❌ 移动端 —— 引入全新产品形态与交互体系，属方向级裁决
- ❌ 自建云端中继 —— 引入常驻基础设施与运维负担（哲学 v2.3 已裁决 CalDAV 优先）

### 7.2 有条件重开（原禁令 → 条件 + 时机）

| 原条目 | 再评估结论 | 重开条件 / 时机 | 成本 |
|---|---|---|---|
| 更换框架（Vue ⇒ 其他） | 维持克制：迁移以月计、零终端用户价值、Tauri+Vue3 无瓶颈 | 仅当出现框架级硬阻塞，且该能力无法封装为独立进程/WebView | 极高 |
| 全量更换 UI 库（EP 退场） | **已被 D-8 实质重开一半**：六个核心表面自绘即去 EP 化；全量替换仍不值 | EP 在设置/表单类页面出现绕不过的体验硬伤时，按表面逐个评估 | 高（原估 ×3 成立） |
| 弃用 Pinia | 部分重开：D-4 已裁数据态走 ctx 服务，Pinia 只剩 UI 态（现存 7 store） | B2 数据通路单一化完成后盘点：剩余 store 若全为偏好态且 ≤3 个，保留亦可；萎缩到零则自然移除——顺其自然，不为移除而移除 | 低 |
| 新建平行 projects 表 | 细分为 **D-9** 再裁决：D-5"单一顶级实体"结论维持，但实现形态开放——(a) 单宽表加 kind vs (b) 泛化＋垂直拆表（project 精简主表 + case_legal_details 侧表）。(b) 兼得哲学一致与表整洁，代价是法律视图多一跳 JOIN（C 线冻结期恰好无消费方） | A1-1 动工前随 D-3(ts-rs) 一起裁决 | 中 |
| 动态字段 TypeOption 大改造 | 原文即"P4 再议"，维持延后属性不变 | 到达 P4 时点评估 | — |
| 重写 formula/deadline/docsy/db 底座 | **大爆炸重写不做；绞杀式局部替换开放**——B1 对 db 底座动刀（连接池/错误码/DomainCommand）已验证局部手术模式可行 | C 解冻后按域评估：每次只换一个接缝，新旧并存灰度，附回归用例 | 中～高 |
| 为对称美新增抽象层 | 改为举证责任规则：新增抽象须指名 ≥2 个真实消费者（ServicesMap/defineTool 即据此立项）；拒绝的是无消费者的预防性抽象，不是抽象本身 | 常态原则 | — |
| C 线冻结期内的一切新功能 | 绝对禁令改为**变更预算规则**：允许同时满足 ①该改动是 A/B 线某项的必要组成（如 K-3 归因必然触及 inbox 写路径）②附案件域回归用例 ③单 commit 可回退。纯 C 线功能诱惑仍然不做 | 常态规则 | — |
| 装饰性动效 | 不是禁令是验收标准："每个动画必须能回答它表达了什么状态变化"——并入 A-UI 三条纪律，原文精神保留 | 常态原则 | — |
| 引入重型动画框架（GSAP/Motion One） | 拆分处理：GSAP/Motion One 维持排除；**微库附预算重开**——CSS Transition/WAAPI/FLIP 确证覆盖不了的效果（物理弹簧拖拽、连续手势跟踪），允许 <10KB gzip 的 motion-v 类微库 | 见 D-10，出现被证伪"CSS 可做"的既列动效时 | 低 |

### 7.3 原则保留（默认值，非禁令）

- **数据有限**：新增顶级实体须先论证为何不能是既有实体的 kind/视角——D-9 是此原则的最后一次放宽讨论
- **门禁不豁免**：任何被重开的项同样走 vue-tsc/build/cargo test 门禁与迁移回归用例

---

## 八、执行排期（v1.3 新增：内核轨道 × A 线的交汇顺序）

```
第 1 周   B0 止血+门禁(≈3d) → R-1 应用身份+updater 基线(1d，不可逆项提前) → K-1 策略上收(0.5d) → A0-1 一键完成 / A0-2 乐观更新 开工
第 2 周   A0-3 Undo / A0-4 系统通知 / A0-5 NL 统一  ∥  K-2 类型双轨合一(1d) ∥ R-3 CI 流水线(1d)
第 3 周   K-3 ✅ 已落地（事件激活+audit_events 归因）∥ D-3 ✅ specta 落地启动（DTO derive 扫描+CommandMap 三域先行）→ A1-2 Areas UI / A1-3 拖拽排序（不依赖 schema 先行）
第 4 周   K-4 内核修缮(0.5d) → A1-1 迁移落地（垂直拆表：project 精简主表 + case_legal_details 侧表，user_version 迁移 + .bak + 案件域回归用例）
第 5 周起 A1-4/5/6/7 收尾 M-GTD-2 → Dogfooding 第一轮 → M-CAL-1（按原 A 线节奏与验收执行）
beta 前   R-2 签名公证 + R-4 安全加固 + R-5 数据保障 + R-6 崩溃上报 + R-7 合规包 集中执行；R-8 授权体系按 D-11 裁决执行（离线许可+设备绑定+宽限期）；R-9 E2E 冒烟随 R-3 CI 就位后接入
```

**交汇规则**：
1. **K-2 先于 A1-1**：ServicesMap 就位后 `ctx.cases ⇒ ctx.projects` 是单点改名；过渡期保留 `cases` 作 deprecated 别名一个里程碑，C 线冻结视图不动
2. **A1-1 等 D-3**：ts-rs 定案则 Project DTO 由 Rust 生成、前端零手工同步；未定案则手工改 types/index.ts 并加对齐测试锁定
3. 每个 K/A/R 项独立 commit 可回退；门禁（vue-tsc 零输出 + build 绿 + cargo test）对所有轨道一视同仁
4. **R-1 不可逆且最先**：bundle identifier / 版本策略在任何真实用户数据落盘前定案（D-11 关联）；此后每个 milestone 产物走 CI 打包验证更新链路
5. **生产级门槛映射**：对外 beta 的放行条件 = A 线 Dogfooding 清单通过 + B3 全部完成 + B4 中 R-1～R-7 完成（R-8 可后置于正式发售）

---

## 附：证据基础

- 《Casy 代码现状调研报告》（2026-08-21）：14 项债务 P0×3/P1×4/P2×5/P3×2，附文件:行号
- 《Casy@61bec8c GTD/日程体验差距审计》（2026-08-21）：五阶段×三软件差距矩阵 + Top15 缺陷清单
- 设计基准：`docs/casy-design-philosophy.md` v2.3
- git 取证：reflog、`.git/rebase-merge/*`、`git diff --stat HEAD 61bec8c`
