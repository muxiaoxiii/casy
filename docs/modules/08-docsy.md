# 模块 08 · 文书工坊（Docsy）

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `01-cases.md`（案件字段注入）/ `07-knowledge.md`（知识引用）/ `09-files.md`（导出落盘）

---

## 一、职责边界

### 1.1 做什么

- 文书模板管理（`list_docsy_templates`）与模板渲染（`render_docsy_template`）。
- DOCX 导出（`export_docx`）。
- 草稿管理（`drafts` CRUD：`create_draft` / `list_drafts` / `get_draft` / `update_draft` / `delete_draft`）。
- 文档生成 Copilot 窗口的数据支撑（模板 + 案件字段 + 知识引用）。

### 1.2 不做什么

- **不负责**案件数据（见 01；渲染时注入案件字段）。
- **不负责**知识库内容（见 07；文书可引用知识块）。
- **不负责**文件的物理归档（见 09；导出落盘由 09 管理）。

---

## 二、数据模型

### 2.1 `drafts`（文书草稿）

| 字段 | 说明 |
|---|---|
| `id` / `case_id` | 主键 + 案件关联 |
| `title` / `content` | 标题与正文 |
| `status` | 草稿状态（草稿/定稿） |
| `created_at` / `updated_at` | 时间戳 |

### 2.2 模板体系

- Docsy 模板（`docsy_engine/`）：内置答辩状/上诉状/代理词/意见陈述等模板。
- 模板变量：案件字段（`{{case_name}}` / `{{court}}` / 当事人 / 专利号等）。

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `list_docsy_templates` | 模板列表 |
| `render_docsy_template` | 用案件数据渲染模板 → 生成文书内容 |
| `export_docx` | 渲染结果导出为 .docx |
| `create_draft` / `list_drafts` / `get_draft` / `update_draft` / `delete_draft` | 草稿 CRUD |

---

## 四、关键流程

### 4.1 文书生成

```text
用户在 Copilot 窗口选择模板
  → render_docsy_template(template_id, case_id)
  → 注入案件字段（01） + 知识引用（07）
  → 生成草稿（写入 drafts，status=draft）
  → 用户编辑（TipTap）
  → 导出 export_docx → 落盘（09）
```

### 4.2 草稿生命周期

```text
create_draft（可空标题，自动时间戳）
  → 编辑保存（update_draft）
  → 定稿/导出
  → 删除
```

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 01 案件 | 渲染时注入案件字段 | 只读案件数据 |
| 07 知识 | 引用知识块（目标态块级引用） | 文书只读知识 |
| 09 文件 | 导出 .docx 落盘、`last_doc_path` 回写 | 文件物理路径归 09 |
| 13 AI | Copilot 窗口：AI 起草草稿（L2 草稿确认） | AI 产出必须经用户确认后定稿 |
| 11 开放 | Skill Runner 引入法律检索/格式化 skill | 按需加载（设计哲学 §11.11） |

---

## 六、演进方向（目标态）

1. **块级引用**：TipTap 文档嵌入知识库块，打通文书工坊与知识库（设计哲学 §9.3）。
2. **Skill Runner 化**：Copilot 窗口升级为 skill 入口（文书生成/法律检索/格式化/翻译按需加载，设计哲学 §11.11）。
3. **AI 草稿确认**：AI 生成草稿 → 用户编辑 → 定稿，全流程留痕（L2 确认，设计哲学 §11.4）。
4. **导出状态栏**：字数统计 + 导出 Word 状态 + 最后保存时间（技术债务 2.7/2.8）。

---

## 七、验收标准

1. 模板渲染注入案件字段准确（空字段有占位提示）。
2. 导出 DOCX 与渲染预览一致。
3. 草稿 CRUD 完整，可回溯版本。
4. Copilot 窗口 AI 生成必须显式进入草稿态。
