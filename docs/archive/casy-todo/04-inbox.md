# 模块 04 · 收件箱（大口袋）

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `05-inbox-batch.md`（批处理）/ `01-cases.md`（归档目标）/ `07-knowledge.md`（知识沉淀）

---

## 一、职责边界

### 1.1 做什么

- 多来源捕获（文件/邮件/短信/速记/粘贴/IMAP）写入 `inbox_items`。
- 单件处理（`process_inbox_item`）：AI 分类 / 规则回退 → 回写分类结果。
- 厘清与归档（`file_inbox_item` / `confirm_inbox_action` / `dismiss_inbox_item`）。
- 大口袋快速判定（`quick_judge_inbox_item`）、AI 增强分析（`ai_analyze_inbox_item`）。
- 文书送达类处理（法院短信/文书下载，`detect_service_delivery` 等）。
- 文件夹监听（`watcher.rs`）与复制进度（`copy_file_with_progress`）。

### 1.2 不做什么

- **不负责**批量分类队列的并发调度（见 05，批处理是独立模块）。
- **不负责**任务系统（厘清"转行动"交给 03）。
- **不负责**知识库内容本身（归档时调用 07 的写入能力）。

---

## 二、数据模型

### 2.1 `inbox_items` 主表

| 分组 | 字段 | 说明 |
|---|---|---|
| 来源 | `source_type` / `source_path` / `source_url` / `source_time` | `source_type` 枚举 `file / email / sms / note / paste / imap` |
| 内容 | `title` / `content_text` / `content_html` | 分类输入 |
| AI 结果 | `ai_category` / `ai_confidence` / `ai_extracted` / `ai_suggested_case_id` | 分类/提取/案件建议 |
| 状态 | `status` | `pending / processing / filed / dismissed`（批处理扩展后含 `processed / failed / archived / ignored`，见 05） |
| 归档 | `user_category` / `linked_case_id` / `filed_to` / `filed_as` / `knowledge_mark` | 归档目标与知识标记 |
| 关联 | `parent_id` / `tasks.inbox_source_id` | 父子收件项、来源任务回溯 |
| 快速判定 | `quick_category` / `quick_confidence` | 第一层即时判断结果（文件名/类型规则） |
| 处理状态 | `ai_analyzed` / `retry_count` / `last_error` / `processing_started_at` / `processed_at` | 批处理与重试依赖字段 |

### 2.2 `inbox_recommendations`（推荐操作）

- 为收件项预计算的"推荐操作"（归档到哪案/标记知识/转任务），由 `quick_judge` 或 AI 生成，供 `confirm_inbox_action` 一键执行。

### 2.3 `case_files`（文件归档记录）

- 收件箱文件归档到案件后写入，关联 `case_id` + 原始来源 + 物理路径（详见 09）。

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `add_inbox_item` | 手动新增收件项（速记/粘贴/笔记） |
| `list_inbox_items` | 收件箱列表（默认最近 100 条） |
| `process_inbox_item` | 单件处理：AI 分类（无 AI 回退规则）→ 回写分类字段 → 部分自动路由 |
| `file_inbox_item` | 归档：复制文件到案件文件夹 + 写 `case_files` |
| `dismiss_inbox_item` | 忽略/放弃 |
| `quick_judge_inbox_item` | 大口袋第一层即时判断（0ms 规则） |
| `confirm_inbox_action` | 确认推荐操作并执行（归档/转任务/转知识） |
| `ai_analyze_inbox_item` | AI 增强分析（用户主动触发） |
| `parse_holiday_notice` | 国务院节假日通知解析（AI → 日期 → 提示确认） |
| `copy_file_with_progress` | 文件拷贝 + 进度回传 |
| `detect_service_delivery` / `match_service_delivery_case` / `parse_service_url` / `download_service_delivery` / `process_service_delivery` | 法院文书送达类处理 |
| 批处理命令（`start/pause/resume/cancel/get_inbox_progress/retry_inbox_item/retry_inbox_case`） | 见 05 |

---

## 四、关键流程

### 4.1 捕获入队

```text
来源入口（watcher / IMAP / add_inbox_item / 剪贴板 / 拖拽）
  → 写入 inbox_items（status=pending，记录 source_type/source_time）
  → 默认进入"待厘清"视图
```

> 所有来源共用同一张主表，不另建第二套收件箱实体（architecture.md §7.3）。

### 4.2 单件处理（process_inbox_item）

```text
1. 读取 content_text
2. 有 AI → process_inbox_with_ai；无 AI → 规则分类回退
3. 回写 ai_category / ai_confidence / ai_extracted / ai_suggested_case_id
4. 依据分类与置信度执行一部分自动路由（失败记日志，不回滚分类结果）
```

> 分类与路由不是一回事；自动路由失败只记日志，不能回滚分类结果（architecture.md §7.2）。

### 4.3 厘清与归档

```text
用户判定"这是什么"（行动/等待/委派/某天/资料）
  → 转任务（03）/ 等待（03）/ 归档到案件（01+09）/ 沉淀知识（07）/ 丢弃
  → 更新 inbox_items.status 与关联字段
```

> 大口袋的定位：先捕获、后整理。判定的入口保持极简（自动时间戳标题、回车即入袋）。

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 05 批处理 | `inbox_items.status` 队列状态；`retry_inbox_item` 等 | 批处理只推进到"已分类/待归档"，不越界归档 |
| 01 案件 | `linked_case_id` / `filed_to`（归档到案件） | 归档时只写关联，不改案件主数据 |
| 09 文件 | `file_inbox_item` → `case_files` + 物理复制 | 物理文件归 09 管理 |
| 07 知识 | `knowledge_mark` / 沉淀到知识库 | 知识写入调用 07 命令 |
| 03 任务 | 厘清"转行动" → `create_task` | 不自动建任务 |
| 11 邮件 | IMAP 拉取 → 收件箱入队 | 见 11 |
| 13 AI | 分类/提取调用 AI 抽象层 | 只写 `ai_*` 字段，不直改业务表 |

---

## 六、演进方向（目标态）

1. **捕获通道增强**：全局快捷键（⌘I 速记 / ⌘T 任务 / ⌘E 日程 / ⌘N 笔记）、剪贴板监听、截图入袋、浏览器剪藏、微信/飞书转发（设计哲学 §10）。
2. **厘清判定面板**：左原始收件箱 + 右判定面板（这是什么/关联案件/类型/优先级/截止），AI 建议待确认（设计哲学 §10.2）。
3. **批处理事件推送**：从轮询进度演进为事件流（见 05 目标 B）。
4. **AI 产出确认**：收件箱 AI 分类结果进"确认区"，用户确认后落地（设计哲学 §11.4）。

---

## 七、验收标准

1. 所有来源入口写入同一 `inbox_items` 表。
2. 无 AI 时规则分类可用，处理不中断。
3. 归档操作可追溯（`filed_to` / `case_files` 完整）。
4. 批处理与单件处理共享同一状态口径，无二义。
