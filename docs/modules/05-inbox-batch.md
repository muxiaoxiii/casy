# 模块 05 · 收件箱批处理

> 版本: v1.1
> 日期: 2026-08-14
> 状态: 现状校准 + 下一轮实现约束
> 关联: `00-README.md` / `04-inbox.md`（队列主表 `inbox_items`）/ `architecture.md` §7.3、§13

## 一、文档定位

本文档不再把批量处理写成“完全未实现的新提案”。当前仓库已经有一版可运行的批处理实现，本文档的作用是：

1. 说明当前实现到底做到了什么。
2. 明确当前实现没有做到什么，避免误判为已完成能力。
3. 给出与现有代码兼容的下一轮改进约束。

本文件只讨论收件箱“批量分类/提取队列”，不扩展为自动归档、自动办案、自动写案卷系统。

## 二、现状摘要

### 2.1 当前已实现能力

源码中已经存在以下命令和处理器：

- `start_inbox_batch`
- `pause_inbox_batch`
- `resume_inbox_batch`
- `cancel_inbox_batch`
- `get_inbox_progress`
- `retry_inbox_item`
- `retry_inbox_case`

后端实现位于 `src-tauri/src/commands/inbox.rs`，核心是进程内单例 `InboxProcessor`。

### 2.2 当前处理范围

当前批处理只做这些事：

1. 从 `inbox_items` 中装载 `status IN ('pending', 'failed')` 的条目。
2. 对每条记录执行 AI 分类或规则分类。
3. 回写 `ai_category`、`ai_confidence`、`ai_extracted`。
4. 成功时把状态置为 `processed`。
5. 失败时根据错误类型回到 `pending` 或标为 `failed`。

当前批处理不会自动完成以下动作：

- 不会执行 `process_inbox_item` 中那套完整自动路由。
- 不会自动创建案件。
- 不会自动归档到案件文件夹。
- 不会自动生成任务、日志、知识库记录。

因此，`processed` 的正确含义是“已完成批量分类/提取”，不是“已处理完业务归档”。

## 三、统一数据模型

### 3.1 主表

队列主数据统一复用 `inbox_items`，不引入第二套收件箱实体。

批处理当前依赖的关键字段如下：

| 字段 | 用途 |
|---|---|
| `id` | 队列项主键 |
| `source_type` | 来源排序与展示 |
| `title` | UI 展示 |
| `content_text` | 分类输入 |
| `ai_category` / `ai_confidence` / `ai_extracted` | 分类输出 |
| `status` | 队列状态 |
| `retry_count` | 重试计数 |
| `last_error` | 最近失败原因 |
| `processing_started_at` | 开始处理时间 |
| `processed_at` | 成功处理时间 |
| `linked_case_id` | 失败按案件重试时使用 |

### 3.2 状态机

批处理相关的 `inbox_items.status` 在 v8 迁移后统一为：

```text
pending -> processing -> processed
failed  -> pending -> processing

processed -> filed / archived
pending   -> ignored
```

说明：

- `pending`：待分类或待重试。
- `processing`：当前批处理中。
- `processed`：已完成分类/提取，等待人工确认或后续归档。
- `failed`：不可自动重试，等待人工处理或显式重试。
- `filed` / `archived` / `ignored`：归档或放弃后的业务状态，不属于批处理运行态。

约束：

- 批处理文档不得再把 `filed` 当作批处理成功态。
- 批处理文档不得再把 `dismissed`、`manual`、`folder_watch` 等旧状态/旧来源写成当前事实。

## 四、当前处理流程

### 4.1 入队

当前没有独立“建队列”命令；入队依赖收件箱主流程：

- 手工新增 `add_inbox_item`
- 文件夹监听 `watcher.rs`
- IMAP 导入
- 其他已有收件箱入口

这些入口写入 `inbox_items` 后，默认状态进入 `pending`。

### 4.2 出队与执行

当前 `start_batch()` 的实际流程是：

1. 如果处理器已在运行，则直接返回。
2. 装载当前所有 `pending`/`failed` 项。
3. 初始化总数、计数器、取消和暂停标记。
4. 为每个条目创建一个 `tokio::spawn` 任务。
5. 通过 `Semaphore` 控制同时处理的数量。
6. 每个条目先把数据库状态改为 `processing`。
7. 执行 AI 或规则分类。
8. 更新分类结果和最终状态。
9. 发送 `ProcessingProgress`。

### 4.3 当前进度模型

当前对外暴露的进度结构体是：

```rust
pub struct ProcessingProgress {
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
    pub active: usize,
    pub current_item: Option<String>,
    pub running: bool,
}
```

当前语义：

- `processed`：成功完成分类/提取的条目数。
- `failed`：最终进入 `failed` 的条目数。
- `active`：当前正在执行的条目数。
- `running`：处理器是否仍在运行。

当前尚未暴露：

- `paused`
- `cancelled`
- `retrying`
- 当前批次 ID

## 五、并发、暂停、取消、重试

### 5.1 并发模型

当前实现：

- 默认最大并发 `8`。
- 最低并发 `1`。
- 每条记录由独立 task 处理。
- `Semaphore` 决定并发上限。

当前动态调节逻辑：

- 响应耗时大于 `5s`，并发减 `1`。
- 响应耗时小于 `1s` 且当前并发低于 `8`，并发加 `1`。
- 预留了限流降半的逻辑入口，但当前调用路径没有把“真实 429 限流状态”传进去。

### 5.2 暂停与恢复

当前实现：

- `pause_inbox_batch` 把内存标记 `paused = true`。
- `resume_inbox_batch` 把内存标记 `paused = false`。
- 启动循环在提交新任务前轮询 `paused`，间隔 `200ms`。

当前限制：

- 已经开始执行的任务不会被中断。
- 暂停是“停止继续派发新任务”，不是冻结全部活动任务。

### 5.3 取消

当前实现：

- `cancel_inbox_batch` 只设置 `cancel = true` 并清除 `paused`。
- 取消主要阻止后续新任务继续排队。

当前限制：

- 已拿到 permit 并开始执行的任务不会被强制中止。
- 当前没有“取消后统一回收 `processing` 状态”的补偿步骤。

### 5.4 重试

当前实现分两类：

1. 自动重试：错误字符串匹配到 `timeout`、`429`、`rate`、`network`、`connection`、`500/502/503` 时，把条目写回 `pending`，并 `retry_count + 1`。
2. 人工重试：`retry_inbox_item` / `retry_inbox_case` 将 `failed` 条目重置为 `pending`。

当前限制：

- 自动重试没有使用 `max_attempts` 一类的硬上限。
- 重试后立即回到主队列，缺少退避时间和死信队列。

## 六、当前实现与目标之间的差距

以下内容此前常被写成“设计已覆盖”，但源码尚未满足：

### 6.1 按案件串行

设计目标曾写“同一案件串行处理”，但当前实现没有按 `linked_case_id` 或 `ai_suggested_case_id` 分组；所有队列项都会独立抢占全局并发。

正确表述：

- 现状：全局并发，无案件内串行保证。
- 目标：引入“同一案件串行，不同案件并行”的调度规则。

### 6.2 真正的动态并发

当前代码会修改 `max_concurrency` 原子值，但已创建的 `Semaphore` permit 数量不会随之重建，因此动态调节只部分生效。

正确表述：

- 现状：存在动态调节意图，但不是完整闭环。
- 目标：调节策略必须能真实影响后续 permit 分配。

### 6.3 崩溃恢复

现状：

- 队列状态落在数据库里。
- 运行中的批次、暂停态、取消态只在内存里。

目标：

- 应用重启后，能够识别超时 `processing` 项并安全回收为 `pending`。
- 重启后无需恢复原 task 句柄，但要恢复“可继续处理”的队列状态。

### 6.4 批处理与业务归档脱节

现状：

- 批处理调用的是 `process_queue_item()`，并不执行 `process_inbox_item()` 的自动路由逻辑。

目标：

- 保持“批量分类/提取”与“人工确认归档”分层。
- 若未来引入批量自动路由，必须作为显式二阶段流程，而不是悄悄扩大现有批处理职责。

## 七、下一轮目标态

本轮文档只批准以下增量改进，不扩大产品范围。

### 7.1 目标 A：补齐状态与恢复语义

要求：

- 启动批处理前，先回收长时间卡在 `processing` 的条目。
- 明确 `processed` 仅表示分类完成。
- 保持 `filed`、`archived`、`ignored` 为批处理后的业务状态。

建议规则：

- 应用启动或 `start_inbox_batch` 前，若 `processing_started_at` 超过阈值且处理器不在运行，则将该条目重置为 `pending`，保留 `last_error = 'stale_processing_recovered'` 或单独记录日志。

### 7.2 目标 B：补齐可观测性

要求：

- 进度结构中新增 `paused` 与 `cancelled`。
- 记录批次开始/结束/取消/恢复日志。
- 前端保留轮询接口，同时允许后续增加事件推送，但不替换既有命令。

推荐结构：

```rust
pub struct ProcessingProgress {
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
    pub active: usize,
    pub current_item: Option<String>,
    pub running: bool,
    pub paused: bool,
    pub cancelled: bool,
}
```

### 7.3 目标 C：补齐重试上限

要求：

- 自动重试必须有上限，例如 `max_retry_count = 3`。
- 超限后进入 `failed`，而不是永久在 `pending`/`processing` 间摆动。

建议规则：

```text
retry_count < 3  -> 可自动重试
retry_count >= 3 -> 直接 failed，等待人工处理
```

### 7.4 目标 D：按案件串行

要求：

- 有明确案件归属的条目，同案串行。
- 无案件归属的条目仍可走全局并发池。

注意：

- 这里的“案件归属”不能只依赖 AI 运行后的 `ai_suggested_case_id`，否则入队前无法分组。
- 第一阶段可用 `linked_case_id` 分组；无 `linked_case_id` 的条目进公共池。

## 八、资源预算

批处理相关预算统一写法如下：

| 项目 | 当前值 | 约束 |
|---|---|---|
| 默认并发 | 8 | 可调，但必须有上限 |
| 最低并发 | 1 | 不允许降到 0 |
| 暂停轮询间隔 | 200ms | 保持轻量 |
| 前端轮询进度 | 当前页面实现为 500ms | 后续可调，但不应小于 200ms |
| AI 每日调用预算 | 默认 50 | 批处理必须受同一 AI 配额控制 |

设计约束：

- 批处理不能绕过 AI `daily_limit`。
- 批处理不能在后台无限重试消耗预算。

## 九、隐私与安全

批处理处理的是收件箱原文，必须遵守以下约束：

- 默认先走本地规则分类；启用 AI 时再调用 AI 后端。
- 不得在文档中假设批处理天然完成脱敏。
- 若未来接入远程模型，必须把是否允许远程发送内容作为显式用户配置或确认点。

## 十、迁移与回滚

### 10.1 当前数据库依赖

批处理现状依赖 v8 迁移新增的字段和状态：

- `retry_count`
- `last_error`
- `processing_started_at`
- `status` 扩展为 `pending/processing/processed/filed/archived/ignored/failed`

### 10.2 后续迁移约束

如果继续扩展批处理表结构：

- 优先继续复用 `inbox_items`，避免新增平行主表。
- 若必须新增批次表，只允许新增“运行记录/审计”表，不能复制收件项实体。
- 迁移文档必须说明旧 `processing` 数据如何回收。

## 十一、验收标准

批处理设计只有在满足以下条件时才算一致且可实施：

1. 文档中的命令名与当前源码完全一致。
2. 文档中的状态值与 `schema.rs` v8 一致。
3. 文档明确区分“已分类”与“已归档”。
4. 文档明确标注当前缺口，不把按案件串行、完整恢复、真实动态并发写成已完成事实。
5. 文档与 `docs/architecture.md` 对状态、恢复、幂等性、资源预算和可观测性的表述一致。
