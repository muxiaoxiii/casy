# Casy 全模块对标开源标杆 — 设计调研

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 调研基准  
> **说明**: 逐模块对标该领域最优秀的开源实现，推导 Casy 各模块的改造方向。与 `gtd-design.md`（任务/日历）配套。

---

## 一、调研结论速览

| Casy 模块 | 对标标杆 | 技术栈匹配度 | 核心可借鉴 |
|-----------|---------|-------------|-----------|
| **多维表格**（案件列表/收件箱/知识库表格） | AppFlowy (flowy-database) | ★★★★★ Rust + SQLite | Cell/Row/Field/TypeOption 抽象、多视图（表格/看板/日历） |
| **知识库** | SiYuan（思源笔记） | ★★★☆☆ Go 后端 | 块级引用 + 双向链接 + SQL 查询知识图谱 |
| **任务/日历** | Things 3 / OmniFocus | ★★★★★（分析见 gtd-design.md） | 时间双轨、时间桶、顺序项目、Forecast |
| **文档编辑器** | TipTap（Casy 已用） | ★★★★★ Vue 原生 | 已是正确选择，无需换 |

---

## 二、多维表格 — AppFlowy flowy-database 深度分析

### 2.1 为什么是 AppFlowy

AppFlowy 是"Notion + Airtable 的开源替代"，**74k+ GitHub stars**，AGPL-3.0。核心引擎 Rust、本地 SQLite 存储——**与 Casy 技术栈几乎一致**（Rust + SQLite + 桌面端）。它的数据库模块（flowy-database）是开源世界里最接近 Casy 需要的多维表格实现。

### 2.2 flowy-database 核心抽象

AppFlowy 的数据库模块用四个基础实体建模一切：

```
┌─────────────────────────────────────────────────────────┐
│  Database（数据库）                                      │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  View（视图）：同一份数据，多种展示                    │ │
│  │  ├── Table 表格视图                                  │ │
│  │  ├── Board 看板视图（按字段分组）                     │ │
│  │  ├── Calendar 日历视图                               │ │
│  │  └── Gallery 画廊视图                                │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                          │
│  Field（字段=列） ←→ 每个 Field 有 TypeOption            │
│  │   ├── Text 文本                                       │
│  │   ├── Number 数字                                     │
│  │   ├── Checkbox 复选框                                 │
│  │   ├── Select / MultiSelect 单选/多选                  │
│  │   ├── Date 日期                                       │
│  │   ├── Relation 关联（连到另一张表）                   │
│  │   └── Formula 公式                                    │
│  │                                                       │
│  Cell（单元格）── 每行每列交叉点的原始数据                │
│  Row（行）── 多个 Cell 打包成一行，看板里=一张卡片        │
│                                                          │
│  Sort / Filter / Group（排序/筛选/分组）                 │
│  └── 都是"基于 Field 的规则"，与视图解耦                  │
└─────────────────────────────────────────────────────────┘
```

### 2.3 关键技术结论

**结论 F1：字段类型可扩展（TypeOption 机制）**
每个 FieldType 映射到一个 `TypeOption`（实现 `TypeOption` trait）。新增字段类型=新增一个 TypeOption，**不用动数据库结构**。Casy 的 cases 表是硬编码列，加字段要改 schema+迁移。AppFlowy 的做法是"列是数据不是 schema"。

> **对 Casy**：动态字段系统（`list_field_groups` 已有雏形）应该向这个方向演进——案件字段不该是 SQL 列，而该是"字段定义 + 单元格值"的键值模型。但这需要大规模重构，属于长期方向。

**结论 F2：视图与数据解耦**
同一份 Row/Cell 数据，可以同时用 Table/Board/Calendar 三种视图看。视图只是"数据的展示配置"，不含数据本身。

> **对 Casy**：现在的案件列表、看板、日历是三个独立功能，底层其实都是案件数据。理想状态是同一份案件数据 + 三种视图切换。Casy 的看板（KanbanView）已经做到"同一份 cases 按状态分列"，方向对了。

**结论 F3：Sort/Filter/Group 是规则不是状态**
排序、筛选、分组都是"基于某个 Field 的条件描述"，可以组合、保存、切换。不是每次硬编码。

> **对 Casy**：CaseFilterBar 已有筛选+分组+排序雏形，但硬编码。应抽象成"筛选规则"可保存复用（已有 savedFilters 雏形）。

**结论 F4：Relation 字段（表间关联）**
AppFlowy 支持 Relation 字段——一个单元格可以指向另一张表的行。这是多维表格的核心能力：案件↔当事人、案件↔任务、案件↔文书 都该是 Relation 而不是字符串。

> **对 Casy**：目前关联靠 `linked_case_id` 等硬编码外键。关系网络（CaseNetworkView）已有雏形，但应该基于"Relation 字段"而非单独的关系表。

### 2.4 其他多维表格开源项目参考

| 项目 | 定位 | 与 Casy 相关性 |
|------|------|---------------|
| **NocoDB** | Airtable 开源替代，Web 端 | 弱（Web 优先，Casy 是桌面端） |
| **Baserow** | Airtable 开源替代，Web 端 | 弱（同上） |
| **Teable** | Airtable 替代，中国 OSS，性能强 | 弱（Web） |
| **Grist** | 关系型表格 + 公式 | 中（公式引擎思路可借鉴） |
| **SeaTable** | 关系型表 | 中 |

**结论**：桌面端 + Rust + SQLite 场景下，**AppFlowy 的 flowy-database 是最佳参考**，NocoDB/Baserow 的 Web 架构参考价值有限。

---

## 三、知识库 — SiYuan（思源笔记）深度分析

### 3.1 为什么是 SiYuan

SiYuan（45k+ stars，AGPL-3.0）是开源知识管理里**块级双向链接**做的最彻底的。Casy 的知识库目前是 CRUD + FTS5 + 语义向量，缺的是"知识互联"——法条↔判例↔笔记↔案件的关系网络。

### 3.2 核心架构

```
块级模型（Block = 段落/标题/列表项/表格）
├── 每个块有唯一 ID
├── 每个块有 Type（NodeParagraph 等）
├── 每个块有 ParentID / RootID（层级）
├── 每个块有 IAL（内联属性，键值对）
└── 块之间可以互相"引用/嵌入"（transclusion）

双向链接实现：
└── 全局引用索引 → 块被引用时自动建立反向链接

数据存储：
├── SQLite → 块索引、属性、关系（高性能查询）
├── Git → 内容版本控制、增量备份
├── 文件系统 → 原始数据
└── 内存缓存 → 热点加速
```

### 3.3 关键技术结论

**结论 S1：块级引用（不是页面级链接）**
Casy 的知识库是"整条知识"级别。SiYuan 把知识拆到**段落/列表项级别**——法条的一个条款、判例的一段说理，都是独立可引用的块。

> **对 Casy**：知识库的 `link_knowledge_to_case` / `link_knowledge_to_law` 已有雏形，但粒度是整条。理想是块级——"引用《专利法》第58条"而不是"引用整篇专利法"。

**结论 S2：SQL 查询知识库**
SiYuan 支持直接用 SQL 查询块和关系（`SELECT * FROM blocks WHERE ...`）。这是知识库的"高级检索"能力。

> **对 Casy**：知识库已有 FTS5 + 混合检索，但缺少结构化查询（按类型/按关联/按属性）。可以补充。

**结论 S3：知识图谱可视化**
SiYuan 内置图视图，展示块与块、文档与文档的网络关系。

> **对 Casy**：CaseNetworkView 是"案件关系图"，但没有"知识图谱"。知识库的语义关联可视化是提升方向。

### 3.4 其他知识库参考

| 项目 | 定位 | 与 Casy 相关性 |
|------|------|---------------|
| **Outline** | 团队知识库 wiki | 中（团队协作方向，Casy 是单机） |
| **Logseq** | 大纲式双向链接 | 中（双链理念一致） |
| **Obsidian** | 本地 Markdown + 图谱 | 中（图谱可视化可借鉴） |
| **Trilium** | 层级笔记 + 脚本 | 低 |

---

## 四、文档编辑器 — TipTap 评估

Casy 已用 TipTap (ProseMirror)。这是正确选择：

- Vue 3 原生支持（`@tiptap/vue-3`）
- Suggestion 扩展（Casy 已用：案件字段/法条/当事人自动补全）
- 块级编辑能力接近 Notion/AppFlowy 的编辑器
- 对比：AppFlowy 用 Flutter 自研编辑器、SiYuan 用自研编辑器——都是大工程。TipTap 是 Vue 生态最佳方案

**结论**：编辑器无需换。升级方向是**块级引用**——让 TipTap 文档里能嵌入知识库块（仿 SiYuan 的 transclusion），这是文书工坊和知识库打通的关键。

---

## 五、综合改造建议（按优先级）

### P0 — 立即可做（不改 schema，前端增强）

| 项 | 对标 | 做法 |
|----|------|------|
| 任务工作台 GTD 化 | Things/OmniFocus | 见 gtd-design.md P0-P1 |
| 案件列表视图切换 | AppFlowy F2 | 案件列表支持 表格/看板 一键切换（数据共享，视图解耦） |
| 知识库图谱视图 | SiYuan S3 | 知识库加语义关联图（已有向量检索，补可视化） |

### P1 — 需迁移（改 schema）

| 项 | 对标 | 做法 |
|----|------|------|
| tasks/cases 迁移 v9 | GTD | 见 gtd-design.md P0 |
| 知识块级化 | SiYuan S1 | knowledge_items 增加 `parent_id`、`block_type`，支持块级引用 |
| 关系字段化 | AppFlowy F4 | 案件关联从硬编码外键演进为 Relation 语义 |

### P2 — 长期方向（大重构）

| 项 | 对标 | 做法 |
|----|------|------|
| 动态字段系统 | AppFlowy F1 | cases 从固定 SQL 列演进为"字段定义 + 单元格"模型 |
| 文档块引用 | SiYuan S1 | TipTap 文档支持嵌入知识库块 |
| 知识 SQL 查询 | SiYuan S2 | 知识库补充结构化查询接口 |

---

## 六、与 gtd-design.md 的衔接

- `gtd-design.md` 解决：任务/日历的 GTD 化（Things/OmniFocus）
- 本文档解决：多维表格/知识库/编辑器的对标（AppFlowy/SiYuan/TipTap）
- 两者共同构成 Casy 的"全模块改造蓝图"

**实施路线**：先做 GTD 化（P0-P1，价值最高且已设计完），再做多维表格视图增强，最后知识库块级化。

---

## 七、参考文献

1. AppFlowy 官方文档：How to add a new property to appflowy database（flowy-database 架构）
2. AppFlowy GitHub（74k stars，AGPL-3.0）
3. SiYuan 官方文档与架构分析（块级引用、SQL 查询、图谱）
4. NocoDB / Baserow / Teable / Grist / SeaTable 对比调研
5. Outline vs SiYuan 对比
