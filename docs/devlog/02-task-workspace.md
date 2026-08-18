# 任务工作台（5 透视）实现记录

> **日期**: 2026-08-18  
> **状态**: 已完成  
> **影响范围**: 前端任务视图、任务 Store、后端任务命令

---

## 一、实现概述

任务工作台从原来的四象限视图重构为 5 个 GTD 透视，实现了「先捕获，后整理」的设计原则。

---

## 二、5 个 GTD 透视

### 2.1 收件箱（Inbox）

**数据源**: `start_bucket = 'inbox'` 的未完成任务

**用途**: 捕获的原始想法、待厘清的任务

**交互**:
- 点击任务卡片 → 打开编辑抽屉
- 下拉菜单 → 厘清（移动到其他透视）

### 2.2 下一步行动（Next Actions）

**数据源**: `blocked = 0` 的任务（顺序项目中）或无案件的 action 任务

**用途**: 当前可执行的任务

**设计原则**:
- 顺序项目中只有 `blocked = 0` 的任务可见
- 完成当前步骤 → 自动解锁下一步

### 2.3 等待（Waiting）

**数据源**: `task_type = 'waiting'` 的未完成任务

**用途**: 等待他人响应的任务

**显示信息**:
- 等待谁（法院/对方/客户）
- 已等待天数
- 跟进日期

### 2.4 今日（Today）

**数据源**: `start_bucket = 'today'` 或 `start_date <= 今天` 的任务

**用途**: 今天需要完成的任务

**排序**: 按 `today_index` 手动排序

**交互**:
- 拖拽重排（未来实现）
- 从其他透视移入

### 2.5 回顾（Review）

**数据源**: `next_review_date <= 今天` 的任务

**用途**: 需要定期回顾的任务

**设计原则**: 案件 `next_review_date` 到期自动进回顾透视

### 2.6 某天（Someday）

**数据源**: `start_bucket = 'someday'` 的任务

**用途**: 暂时搁置但未来可能执行的任务

---

## 三、前端实现

### 3.1 TasksView.vue

**重构内容**:
- 从四象限视图改为 5 透视标签页
- 新增捕获对话框（⌘T 快捷键）
- 新增厘清对话框
- 新增编辑抽屉（支持所有 GTD 字段）

**UI 组件**:
- 透视标签栏（带计数徽章）
- 任务卡片（显示 GTD 元数据）
- 捕获/厘清/编辑对话框

### 3.2 tasks.ts Store

**新增类型**:
```typescript
export type TaskType = 'action' | 'waiting' | 'delegated' | 'someday'
export type StartBucket = 'inbox' | 'anytime' | 'someday' | 'today'
export type Context = 'office' | 'phone' | 'court' | 'computer' | 'outside'
```

**新增 Getters**:
- `inboxTasks` - 收件箱任务
- `nextActions` - 下一步行动
- `waitingTasks` - 等待任务
- `todayTasks` - 今日任务
- `reviewTasks` - 回顾任务
- `somedayTasks` - 某天任务
- `taskStats` - 各透视任务数量统计

**新增 Actions**:
- `triageTask()` - 厘清任务
- `moveToToday()` - 移动到今日
- `markAsWaiting()` - 标记为等待
- `toggleFlag()` - 切换旗标
- `reorderToday()` - 重排今日列表

---

## 四、后端实现

### 4.1 tasks.rs 命令更新

**list_tasks**:
- 新增过滤器：`area_id`, `task_type`, `start_bucket`
- 返回所有 GTD 字段

**create_task**:
- 支持所有 GTD 字段
- 自动记录 `task_event`（created）

**update_task**:
- 支持更新所有 GTD 字段
- 自动记录 `task_event`（moved）

**toggle_task**:
- 记录 `task_event`（completed/created）

### 4.2 areas.rs 新增命令

**list_areas** - 获取所有领域
**get_area** - 获取单个领域
**create_area** - 创建领域
**update_area** - 更新领域
**delete_area** - 删除领域（检查关联任务）
**get_area_stats** - 获取领域统计

---

## 五、设计原则遵循

### 5.1 原则二：时间双轨

- `start_date` + `due_date` 实现双轨
- 透视由 `start_bucket` 和 `task_type` 推导

### 5.2 原则三：先捕获，后整理

- 收件箱透视接收所有新捕获的任务
- 厘清对话框将任务移动到正确透视

### 5.3 原则四：数据有限，视图无限

- 5 个透视是同一份任务数据的不同视图
- 透视由字段推导，不是独立存储

---

## 六、交互细节

### 6.1 快捷键

- `⌘T` - 快速捕获任务

### 6.2 任务卡片显示

- 任务名称
- 任务类型标签（行动/等待/委派/某天）
- 关联案件
- 关联领域
- 截止日期（逾期红色/即将到期琥珀色）
- 等待信息（等待谁 + 天数）
- 上下文标签（@办公室/@电话/@法院）
- 预估时间

### 6.3 操作菜单

- 编辑
- 厘清（仅收件箱）
- 移至今日
- 标记等待
- 删除

---

## 七、后续工作

1. **P2**: 实现拖拽排序（今日列表）
2. **P2**: 实现顺序项目自动解锁
3. **P3**: 实现 AI 推荐今日任务
4. **P3**: 实现时间预估校准

---

## 八、技术细节

### 8.1 数据库查询

- 使用 `COALESCE` 处理可选字段更新
- 使用 `task_events` 表记录行为数据
- 新增索引覆盖常用查询

### 8.2 前端状态管理

- 使用 Pinia store 管理任务状态
- 使用 computed 属性实现透视过滤
- 使用 Element Plus 组件库

### 8.3 类型安全

- 定义 GTD 类型（TaskType, StartBucket, Context）
- 使用 TypeScript 接口定义任务结构
