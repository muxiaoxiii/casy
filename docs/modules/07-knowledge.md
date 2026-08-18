# 模块 07 · 知识库

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `01-cases.md`（案件引用）/ `04-inbox.md`（知识沉淀）/ `14-data-layer.md`（FTS/嵌入）

---

## 一、职责边界

### 1.1 做什么

- 知识条目（`knowledge_items`）CRUD 与检索。
- 版本管理（`knowledge_versions`）与差异对比。
- 知识关联（`knowledge_relations`：知识↔案件、知识↔法条、知识↔知识）。
- 语义嵌入（`knowledge_embeddings`）与混合检索（FTS5 + 向量）。
- 知识库统计与文档生成引用。

### 1.2 不做什么

- **不负责**收件箱分类（见 04；收件箱归档时调用本模块的写入能力）。
- **不负责**案件数据（见 01；本模块只建立引用关系）。
- **不负责**文书生成（见 08；文书可以引用知识库块）。

---

## 二、数据模型

| 表 | 用途 | 关键约束 |
|---|---|---|
| `knowledge_items` | 知识主表 | 分类 `category`；未来演进 6 职能分类（灵感/方法/参考/问题/经验/日志） |
| `knowledge_versions` | 版本快照 | `diff_knowledge_versions` 提供对比 |
| `knowledge_relations` | 关联关系 | 支持 `link_knowledge_to_case` / `link_knowledge_to_law` |
| `knowledge_embeddings` | 语义向量 | 与 FTS5 混合检索 |
| `knowledge_relations` 扩展 | 块级引用（目标态） | `parent_id` / `block_type`（块级模型） |

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `list_knowledge` / `create_knowledge` / `update_knowledge` / `delete_knowledge` | CRUD |
| `search_knowledge` | 混合检索（FTS + 语义） |
| `knowledge_stats` | 知识库统计 |
| `list_knowledge_versions` / `diff_knowledge_versions` / `diff_knowledge_with_current` | 版本与差异对比 |
| `create_knowledge_from_selection` | 从选中文本创建知识 |
| `link_knowledge_to_case` / `link_knowledge_to_law` | 建立关联 |

---

## 四、关键流程

### 4.1 检索

```text
search_knowledge(keyword)
  → FTS5 全文命中 + 语义向量近邻
  → 合并去重 → 按相关度排序返回
```

### 4.2 从收件箱沉淀

```text
收件箱项标记 knowledge_mark（04）
  → create_knowledge（或 create_knowledge_from_selection）
  → 关联来源案件/法条（可选）
  → 写入 knowledge_versions 初始版本
```

### 4.3 版本对比

```text
list_knowledge_versions(id) → 选择两个版本
  → diff_knowledge_versions(v1, v2) → 字段级差异
  → 用于文书引用溯源与协作审阅
```

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 01 案件 | `link_knowledge_to_case` 建立知识↔案件引用 | 只建引用，不改案件 |
| 04 收件箱 | `knowledge_mark` → 沉淀入口 | 知识内容在 07，收件箱只触发 |
| 08 文书 | 文书引用知识块（目标态块级引用） | 文书工坊只读知识 |
| 13 AI | 隐性关联学习（通道 B） | AI 洞察经确认后沉淀为知识（设计哲学 §3.2） |
| 14 数据层 | FTS5 / 嵌入存储 | 检索基础设施在 14 |

---

## 六、演进方向（目标态）

1. **6 职能分类**：按"能做什么"分类（灵感/方法/参考/问题/经验/日志），替代主题分类（设计哲学 §8.2）。
2. **块级引用**：`parent_id` + `block_type`，支持"引用《专利法》第 58 条"而非整篇。
3. **知识图谱**：知识↔案件↔任务关系网络可视化。
4. **标签动态化**：知识标签=关联的案件/任务/领域，不维护标签宇宙。

---

## 七、验收标准

1. 混合检索返回结果有明确相关度排序。
2. 版本对比能展示字段级差异。
3. 关联建立后可从知识反查案件（`knowledge_relations` 双向）。
4. 知识沉淀入口完整（收件箱 → 知识 → 关联案件）。
