# Casy 模块设计文档 · 索引

> **版本**: v2.0  
> **日期**: 2026-08-18  
> **定位**: 各模块的细节设计文档目录。本文档说明模块划分、文档依赖关系和阅读顺序。
> **架构**: 插件化架构（详见 `docs/architecture-v3.md`）

---

## 一、模块划分原则

1. **插件化封装**：每个业务模块封装为独立插件（`src/core/plugins/*-plugin.ts`）
2. **工具注册**：每个插件注册一组工具，供 AI 调用
3. **职责内聚**：同一插件内的工具围绕一个业务职责
4. **事件驱动**：插件间通过事件系统通信，松耦合

## 二、模块清单

| 编号 | 模块 | 插件类 | 工具数 | 主文档 |
|---|---|---|---|---|
| 00 | 模块索引（本文档） | — | — | 本文档 |
| 01 | 案件管理 | `CasesPlugin` | 6 | `01-cases.md` |
| 02 | 三轨状态机 | （集成在 CasesPlugin） | — | `02-status-machine.md` |
| 03 | 任务系统 | `TasksPlugin` | 5 | `03-tasks.md` |
| 04 | 收件箱 | `InboxPlugin` | 5 | `04-inbox.md` |
| 05 | 收件箱批处理 | （集成在 InboxPlugin） | — | `05-inbox-batch.md` |
| 06 | 日历与期限 | `CalendarPlugin` | 3 | `06-calendar-deadline.md` |
| 07 | 知识库 | `KnowledgePlugin` | 5 | `07-knowledge.md` |
| 08 | 文书工坊 | （待实现） | — | `08-docsy.md` |
| 09 | 文件管理 | `FilesPlugin` | 3 | `09-files.md` |
| 10 | 同步 | `SyncPlugin` | 4 | `10-sync.md` |
| 11 | 邮件 | （待实现） | — | `11-email.md` |
| 12 | 提醒 | `ReminderPlugin` | 4 | `12-reminder.md` |
| 13 | AI 智伴 | （集成在核心容器） | — | `13-ai-companion.md` |
| 14 | 数据层 | （底层，非插件） | — | `14-data-layer.md` |
| 15 | 可观测性与设置 | `SettingsPlugin` | 3 | `15-observability-settings.md` |
| 16 | 双向开放 | （目标态） | — | `16-openness.md` |

**工具总计**: 38 个

## 三、文档依赖

```text
architecture.md（顶层架构）
   ├── 01-cases.md ── 依赖 02（状态机）、06（期限）、09（文件）、07（知识）
   ├── 02-status-machine.md（独立）
   ├── 03-tasks.md ── 依赖 01（案件上下文）、06（期限）
   ├── 04-inbox.md ── 依赖 01（归档到案件）、07（知识沉淀）、05（批处理）
   ├── 05-inbox-batch.md（依赖 04 的队列主表）
   ├── 06-calendar-deadline.md（独立，供 01/03/12 消费）
   ├── 07-knowledge.md（独立，供 01/04 引用）
   ├── 08-docsy.md（独立，供 01/07 引用）
   ├── 09-files.md（独立，供 01/04 消费）
   ├── 10-sync.md（跨模块，WebDAV/飞书）
   ├── 11-email.md（供 04 收件箱入队）
   ├── 12-reminder.md（消费 06 期限结果，M1 引入日历同步）
   ├── 13-ai-companion.md（目标态，横切 01/03/04/07/12）
   ├── 14-data-layer.md（底座，全部依赖）
   ├── 15-observability-settings.md（横切）
   └── 16-openness.md（目标态，横切 08/13）
```

## 四、与其他顶层文档的分工

| 文档 | 职责 | 不负责 |
|---|---|---|
| `casy-design-philosophy.md` | 为什么这样做（原则、模块蓝图、UI 规范、路线图） | 技术细节、命令名、字段语义 |
| `architecture.md` | 顶层技术架构（分层、数据模型、状态机口径、离线提醒决策） | 单模块内部流程细节 |
| `modules/*.md`（本文档体系） | 单模块的详细设计（数据模型、命令、流程、约束） | 重复顶层原则（引用即可） |

## 五、写作与维护约束

1. 模块文档中的表名、命令名、状态值必须与 `architecture.md` §5、`schema.rs`、`commands/mod.rs` 一致。
2. 新增命令必须同步更新对应模块文档；新增表必须更新 `14-data-layer.md` 与 `architecture.md` §5。
3. 设计草案（未落库）必须明确标注"目标态"；不得把未实现能力写成现状。
4. 每个模块文档控制在合理篇幅，超限拆分子文档并在本文档登记。
