# UI 升级实现记录

> **日期**: 2026-08-18  
> **状态**: 已完成  
> **影响范围**: 全局主题、数据看板、任务透视、日历周视图

---

## 一、升级概述

基于 `casy-design-philosophy.md` 设计基准，参考优效日历、OmniFocus、Things 3 的设计模式，对 Casy 前端进行全面 UI 升级。

---

## 二、变更清单

### 2.1 主题系统（Slate 石墨蓝）

**文件**: `src/assets/theme.css`, `src/style.css`

| 变更项 | 旧值 | 新值 |
|--------|------|------|
| 主色 | `#2563EB` (Element Plus 蓝) | `#3E5C9A` (Slate 石墨蓝) |
| 主色 hover | `#1D4ED8` | `#334D82` |
| 主色 soft | `#EFF6FF` | `#EDF1F8` |
| 逾期红 | `#EF4444` | `#B4554F` |
| 琥珀色 | `#F59E0B` | `#B0823A` |
| 成功绿 | `#10B981` | `#4C8067` |
| 信息紫 | `#8B5CF6` | `#6C6A9C` |
| 背景色 | `#FAFAFA` | `#F6F7F9` |
| 边框色 | `#E5E7EB` | `#E0E3E9` |
| 文字主色 | `#18181B` | `#1F2430` |
| 文字次色 | `#52525B` | `#4B5160` |
| 文字弱色 | `#A1A1AA` | `#9BA2AF` |

**设计依据**: 设计哲学 §12.2，"低饱和稳重，专业信任感"

### 2.2 数据看板（新增模块）

**文件**: `src/modules/dashboard/DashboardView.vue`

新增数据可视化看板，包含 4 种纯 SVG 图表：

| 图表 | 类型 | 数据源 |
|------|------|--------|
| 案件状态分布 | 环形图 | `cases` 表按 `caseStatus` 分组 |
| 月度任务趋势 | 折线图 | 近 6 个月任务创建/完成数 |
| 轨道分布 | 水平条形图 | `cases` 表按 `track` 分组 |
| 近期庭审时间线 | 时间线 | `events` 表中 `hearing/deadline` 类型 |

**技术选型**: 纯 SVG 实现，无外部图表库依赖。

**路由**: `/dashboard`  
**导航入口**: 侧栏新增「数据看板」（位于日历和收件箱之间）

### 2.3 任务工作台（GTD 7 透视）

**文件**: `src/modules/tasks/views/TasksView.vue`

新增「计划中」(upcoming) 透视：

| 透视 | 数据源 | 排序 |
|------|--------|------|
| 收件箱 | `start_bucket = 'inbox'` | 创建时间 |
| 今天 | `start_bucket = 'today'` 或 `startDate <= 今天` | `todayIndex` |
| **计划中** (新增) | `taskType = 'action'` 且有 `startDate/dueDate/deadline` | 日期升序 |
| 随时 | `taskType = 'action'` 且 `blocked = 0` | — |
| 等待 | `taskType = 'waiting'` | 等待天数 |
| 回顾 | `nextReviewDate <= 今天` | — |
| 某天 | `start_bucket = 'someday'` | — |

**设计依据**: 设计哲学 §5.2，参考 OmniFocus Forecast 视角

### 2.4 日历周视图时间块

**文件**: `src/modules/calendar/views/CalendarView.vue`

新增周视图时间块功能：

- 时间轴: 7:00 - 21:00（15 个小时格）
- 事件按小时分格显示
- 今日列高亮
- 事件块带颜色编码（左色条 + 半透明背景）

**设计依据**: 设计哲学 §7，参考优效日历时间块布局

### 2.5 修复预置缺失模块

| 文件 | 说明 |
|------|------|
| `src/core/plugin/initializer.ts` | 插件系统初始化器（占位） |
| `src/core/plugin/context.ts` | 插件上下文（占位） |
| `src/core/ai/tool-caller.ts` | AI 工具调用器（占位） |
| `src/stores/settings.ts` | Settings Store |
| `src/modules/settings/components/AISettings.vue` | AI 设置组件 |

---

## 三、设计参照

### 3.1 已有设计资源

| 资源 | 路径 | 说明 |
|------|------|------|
| HTML 原型 v1 | `designs/casy-ui/index.html` | 全功能版，含 5 套主题 |
| HTML 原型 v2 | `designs/casy-html/index-v2.html` | 秩序版，含 GTD 透视 |
| 设计截图 | `designs/casy-ui-screens/` | 11 页 PNG + SVG 截图 |
| 融合设计原型 | `designs/casy-ui-upgrade/` | 融合版，含 D3 可视化 |

### 3.2 设计参照工具

| 工具 | 学什么 | 落地点 |
|------|--------|--------|
| 优效日历 | 时间块布局、周视图、月视图事件点 | 日历周视图 |
| OmniFocus | GTD 透视、Forecast、时间双轨、顺序项目 | 任务工作台 |
| Things 3 | 手动排序、Today 列表、时间桶枚举 | 任务 Today 透视 |

---

## 四、待完成

| 优先级 | 项目 | 设计依据 |
|--------|------|---------|
| P1 | 案件详情页：进度环 + 里程碑嵌套树 | 设计哲学 §6.2 |
| P1 | 首页：智能推荐区 + 统计底栏 | 设计哲学 §11.6 |
| P2 | 收件箱：快速捕获条 + 多通道入口 | 设计哲学 §10 |
| P2 | 知识库：6 职能分类 + 块级引用 | 设计哲学 §8 |
| P3 | 今日面板：AI 推荐引擎接通 | 设计哲学 §11.6 |
| P3 | 日历：日视图 + 时间分配建议 | 设计哲学 §7 |

---

## 五、构建状态

- `npx vite build` ✅ 通过
- 警告: 部分 chunk > 500 kB（建议后续 code splitting）
