# 模块 09 · 文件管理

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `01-cases.md`（案件文件夹）/ `04-inbox.md`（文件归档）/ `08-docsy.md`（导出落盘）

---

## 一、职责边界

### 1.1 做什么

- 案件文件夹模板（`case_folder_templates`）与自动建目录。
- 文件命名规则（`file_naming_rules`）。
- 案件文件记录（`case_files`）与增删。
- 文件夹监听（`watcher.rs`）捕获新文件 → 收件箱入队。
- 智能路由（`files/auto_classify`）文件名/类型分类。

### 1.2 不做什么

- **不负责**收件箱分类/厘清（见 04；本模块只提供文件层面的捕获与归档）。
- **不负责**文书生成（见 08；导出落盘调用本模块能力）。
- **不负责**案件业务状态（见 01）。

---

## 二、数据模型

| 表 | 用途 | 关键约束 |
|---|---|---|
| `case_files` | 案件文件记录 | `case_id` + `source_path`（来源）+ 归档路径 + 大小/类型 |
| `file_naming_rules` | 命名规则 | 模板化命名（如 `{case_no}_{doc_type}_{date}`） |
| `case_folder_templates` | 目录模板 | 默认诉讼 12 目录等标准结构 |

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `list_case_files` / `add_case_file` / `delete_case_file` | 案件文件 CRUD |
| `get_folder_template` / `list_folder_templates` / `save_folder_template` / `delete_folder_template` | 目录模板管理 |
| `get_folder_naming_settings` / `save_folder_naming_settings` | 命名规则设置 |

---

## 四、关键流程

### 4.1 案件建档

```text
create_case（01）
  → 按 case_folder_templates 生成案件目录（诉讼12目录/商标/专利）
  → 回写 cases.folder_path
```

### 4.2 文件捕获

```text
watcher.rs 监听案件目录新文件
  → files::auto_classify 即时分类（50ms 内）
  → 写入 inbox_items（source_type='file'）→ 04 收件箱处理
```

### 4.3 文件归档

```text
收件箱 file_inbox_item（04）
  → 按命名规则重命名 + 复制到案件目录（copy_file_with_progress）
  → 写 case_files 记录
  → 更新 inbox_items.filed_to / filed_as
```

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 01 案件 | `folder_path` 回写；案件目录模板 | 目录生成失败不回滚案件 |
| 04 收件箱 | 文件入队 / 归档执行 | 归档动作由 04 触发，文件操作由 09 执行 |
| 08 文书 | 导出落盘 | 只写文件，不涉及文书内容 |
| 11 邮件 | 邮件附件 → 收件箱 | 附件保存路径由 09 约定 |

---

## 六、演进方向（目标态）

1. **安全拷贝分级**：<10MB 直接 OS 拷贝 + 大小校验；>=10MB 流式拷贝 + 后台异步校验（v2.1 设计）。
2. **文件已存在策略**：同名加序号 / 同 hash 跳过 / 手动覆盖备份。
3. **OCR/PDF 工具集成**：扫描件识别、PDF 拆分合并（参考 skills 生态，设计哲学 §11.11）。

---

## 七、验收标准

1. 案件建档自动生成标准目录且可模板化配置。
2. 文件捕获到入队延迟低（即时判断 50ms）。
3. 归档复制有进度条 + 完整性校验。
4. 文件记录可追溯（来源 → 归档位置 → 案件）。
