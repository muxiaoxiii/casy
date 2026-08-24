# Casy 项目状态

> **记录时间**: 2026-08-24
> **基准**: main @ `831fdfe` 之后（仓库恢复完成后的新主线）
> **规划**: `docs/refactoring-plan.md` v1.2（A/B/C 三线）
> **说明**: 本文件是唯一状态文档；旧版 Casy-STATUS-v3.md 描述的是 rebase 事故前的失实状态，已删除。

---

## 一、仓库状态（B0 恢复完成 ✅）

- 2026-08-21 发现并修复 pull --rebase 中断事故：21 个最新提交曾滞留重放队列。
- 处置：`git rebase --abort` 回到完整线谱 `61bec8c`；暂停期 9 个提交保留在 `rescue/paused-work`，逐个评审后**吸收 2 个**（vite devSourcemap；Slate 主题经评估与主线同源，仅取 devSourcemap），**废弃 7 个**（含复活已删文档的 merge、6 万行旧文件回填、调试标记、已被超越的四象限移除）。
- 备份分支 `rescue/paused-work` / `rescue/main-full` 保留至稳定后清理。

## 二、绿基线与门禁

| 项 | 状态 |
|----|------|
| cargo test | ✅ 103 通过 |
| vite build | ✅（829KB 主 chunk，代码分割列入优化项） |
| vue-tsc strict | 🔄 62 个存量错误，清零中；清零后 build 门禁自动生效（build = vue-tsc && vite build） |
| scripts | ✅ typecheck / test / build 已定义 |

## 三、M-GTD-1「不打断」进度

| 项 | 状态 | 说明 |
|----|------|------|
| A0-1 一键完成 | ✅ | 耗时弹窗默认关闭（设置 ask_actual_minutes 可开启旧行为）；完成 ≤1 次交互 |
| A0-2 乐观更新 | ✅ 核心路径 | complete/delete/snooze 走 `src/core/taskActions.ts` 乐观模块；失败回滚；其余视图推广中 |
| A0-3 Undo | ✅ 核心路径 | ⌘Z 撤销完成/删除/稍后（栈深 20），输入框内不劫持原生撤销 |
| A0-4 系统通知 | ✅ | tauri notification 插件优先，osascript 兜底 |
| A0-5 NL 统一 | ✅ | `src/shared/nlp/parseWhen.ts` 唯一实现；修复 dueTime 恒 null、startBucket='upcoming' 违反 CHECK 约束致捕获静默失败两个真 bug |

## 四、M-UI-0「动效地基」进度

| 项 | 状态 | 说明 |
|----|------|------|
| U-1 Design Tokens | ✅ | 主线 Slate 令牌体系已完备，无需替换（与 rescue 版同色板） |
| U-2 Motion Tokens | ✅ | 时长三档 + ease-out/spring 曲线 + .vfade/.vslide/.vscale 预设 + reduced-motion 降级 |
| Things3 式完成动画 | ✅ | 圆形弹性填充 + 对勾延迟弹出 + 划线 background-size 过渡 |
| U-3 核心表面自绘 | ⏳ 未开始 | TaskRow/List/PerspectiveTabs/QuickCapture/CalendarGrid/TimeGrid |
| U-4 全局键盘中心 | ⏳ 未开始 | 当前 ⌘Z/⌘T 为视图级监听，后续统一注册中心 |

## 五、下一步（按 refactoring-plan v1.2）

1. TS 清零收尾 → build 门禁全绿
2. 乐观更新推广至 HomeView 今日面板等剩余 5 处整表重拉点
3. U-3 TaskRow/TaskList 自绘组件替换 el 组件渲染的任务行
4. M-GTD-2：个人项目建模（D-5）、Areas UI、Today 拖拽排序、子任务、重复任务、⌘K
