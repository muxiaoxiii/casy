# Casy 重构方案（基于设计哲学的重新规划）

> **版本**: v1.2（新增 A-UI 工作流）
> **日期**: 2026-08-21
> **状态**: 待评审
> **上位文档**: `docs/casy-design-philosophy.md`（唯一总纲）
> **v1.1 变更**: 依据用户裁决调整产品主线——**先把 Casy 做成好用的个人任务管理+GTD+日程软件，其次才叠加律师工作台**。路线图从单线 R0-R4 改为 A（产品手感）/B（平台地基）/C（律师业务冻结）三线。
> **v1.2 变更**: 依据用户要求新增 **A-UI 工作流**——UI 全面升级，目标「流畅、灵动、好用」。定位：**流畅是性能预算问题；灵动是有目的的动效系统**——与哲学"克制即优雅"不冲突，对标 Things3 的完成动画与 Linear 的键盘流（motion 服务于理解，而非装饰）。
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

---

## 四、C 线：律师业务（冻结期政策）

- **冻结范围**：案件三轨状态机、期限引擎、飞书同步、WebDAV、文书工坊、收件箱 AI 分类——不再新增功能，只修致命 bug
- **保留义务**：现有数据兼容不受 A/B 线迁移破坏（每次迁移跑案件域回归用例）
- **解冻条件**：A 线 Dogfooding 清单通过
- **解冻后的方向预告**：案件作为 kind=legal 项目回到视图层；期限引擎接入提醒通道；飞书同步续建；CalDAV（caldav.rs 雏形已在库）；原 R3/R4（M0 验收取证、Confirmer 强制点、推荐引擎、报表、蒸馏）按哲学 §13 恢复推进

---

## 五、关键决策点汇总

| # | 决策 | 建议 |
|---|------|------|
| D-1 | 完成 git 恢复（abort + cherry-pick）授权 | 建议：授权。备份分支已建，全程可逆 |
| D-2 | rescue 9 提交吸收尺度 | 视觉/文档吸收；任务视图改动对撞评审取优；调试提交丢弃 |
| D-3 | DTO codegen | 引入 ts-rs（构建期生成 TS 类型）；保守替代=手工+对齐测试锁定 |
| D-4 | Pinia store 去留 | 最小集保留（UI 态/偏好），数据态全面走 ctx 服务 |
| D-5 | **个人项目建模（v1.1 新增，影响 schema）** | 推荐：cases 泛化为 project 加 kind 字段；备选：新建平行 projects 表（违反"数据有限"，不建议） |
| D-6 | 耗时弹窗处置 | 设置开关默认关 + 完成后轻提示（保留学习闭环数据来源，见哲学 §11.6） |
| D-7 | 日历独立事件表 | 引入 calendar_events（日程是独立事实源）；案件期限/庭审保留为只读图层 |
| D-8 | **Element Plus 保留范围（v1.2 新增）** | 推荐：六个核心 GTD 表面自绘（TaskRow/List/PerspectiveTabs/QuickCapture/CalendarGrid/TimeGrid），EP 留在设置/表单/弹窗；备选：全面替换 EP（工程量 ×3，不建议）；保守：全部 EP 换肤（灵动上限低） |

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

## 七、本方案明确不做的事（克制清单）

- ❌ 更换框架/UI 库/状态管理库
- ❌ 新建平行 projects 表（D-5 已裁决方向）
- ❌ 移动端、自建云端中继（哲学 v2.3 裁决：CalDAV 优先）
- ❌ 动态字段 TypeOption 大改造（P4 再议）
- ❌ 为对称美新增抽象层；❌ 重写 formula/deadline/docsy/db 底座
- ❌ C 线冻结期内的一切新功能诱惑
- ❌ 装饰性动效（灵动≠花哨：每个动画必须能回答"它表达了什么状态变化"）；不引入重型动画框架（CSS Transition + FLIP 工具即可，不引 GSAP/Motion One）

---

## 附：证据基础

- 《Casy 代码现状调研报告》（2026-08-21）：14 项债务 P0×3/P1×4/P2×5/P3×2，附文件:行号
- 《Casy@61bec8c GTD/日程体验差距审计》（2026-08-21）：五阶段×三软件差距矩阵 + Top15 缺陷清单
- 设计基准：`docs/casy-design-philosophy.md` v2.3
- git 取证：reflog、`.git/rebase-merge/*`、`git diff --stat HEAD 61bec8c`
