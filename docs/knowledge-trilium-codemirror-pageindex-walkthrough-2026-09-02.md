# Casy 知识库 Trilium / CodeMirror / PageIndex 升级 Walkthrough

日期：2026-09-02

## 目标

在上一轮“笔记列表 + Markdown 编辑”基础上，补齐四条真正决定知识库可用性的能力：

1. CodeMirror 6 级 Markdown 编辑体验；
2. 可追踪、可导航的双向链接；
3. 可查看差异、可恢复的版本历史；
4. OCR Markdown 与 PageIndex 结构树的知识沉淀。

同时引入 Trilium 的框架思想：层级笔记、一个关系型数据库中的内容与关系、可持续版本化，以及面向大型知识库的导航方式。

## 对标判断

Trilium 当前核心特征包括任意深度笔记树、全文搜索、无缝版本历史、属性与关系、关系图和 Markdown 导入导出。官方项目使用 AGPL-3.0，因此本轮与 Jot 一样，只采用产品结构和交互原则，没有复制 Trilium 源码。

- 项目与功能说明：<https://github.com/TriliumNext/Trilium>
- 版本历史设计：<https://triliumnext.github.io/Docs/Wiki/note-revisions.html>
- 内部链接与 Note Map：<https://docs.triliumnotes.org/user-guide/note-types/text/links>
- CodeMirror 官方扩展能力：<https://codemirror.com/docs/extensions/>

## 一、CodeMirror 6 编辑器

新增独立的 Vue 封装组件 `MarkdownCodeMirror.vue`，替换原生 `textarea`。

已具备：

- Markdown 语法解析与高亮；
- 行号、活动行和代码折叠；
- 撤销/重做历史；
- 搜索与替换；
- 括号补全、缩进和 Tab 支持；
- 自动换行；
- `Cmd/Ctrl + S` 快捷保存；
- `[[笔记标题]]` 自动补全；
- 与 Casy 主题变量一致的编辑器、搜索面板和补全菜单样式；
- 外部状态切换笔记时，能够替换文档而不制造错误的保存事件。

编辑、分栏预览和纯预览三种模式继续保留。

## 二、Trilium 式层级笔记与双链

### 层级笔记

- 笔记属性中增加“上级笔记”。
- 当前笔记工具栏增加“新建子笔记”。
- 中栏按 `parentId` 构造树状顺序并显示层级缩进。
- 后端更新笔记时校验：
  - 上级笔记必须存在；
  - 不能把自己设为上级；
  - 不能形成祖先循环。

### 双链

- 正文保存时解析 `[[笔记标题]]`。
- 精确匹配同名知识条目并同步到通用 `links` 表。
- 删除正文中的 Wiki 链接时，相应自动关系同步删除。
- 手动关联与 Wiki 自动关联可以并存。
- 右栏“双链”面板同时展示：
  - 当前笔记指向哪些笔记；
  - 哪些知识、文书、任务、案件或文件引用了当前笔记；
  - 可导航、可解除的关联。

浏览器回归中，从“专利无效程序时间节点汇总”写入 `[[最高法知识产权案件裁判要旨]]` 后，来源笔记显示 1 条出链，目标笔记显示 1 条反链。

## 三、版本历史

- 每次正文真正变化时才考虑建立快照。
- 自动保存采用 5 分钟编辑会话窗口：同一窗口只保留一个会话前快照，避免每次停顿都制造版本。
- 历史面板显示时间、原因和正文长度。
- 选择历史版本后调用现有差异命令，展示逐行新增、删除和相同内容。
- 恢复历史版本前，后端先保存当前正文为 `before_restore` 快照，因此恢复操作可再次撤销。
- 后端校验历史版本必须属于当前笔记，防止跨笔记错误恢复。
- 恢复完成后重新同步 Wiki 双链，避免正文和关系表不一致。

## 四、OCR / PageIndex 内容沉淀

新增两个后端命令：

- `list_knowledge_document_sources`：列出已经完成 OCR/PageIndex 的 PDF；
- `import_pageindex_to_knowledge`：将指定文件沉淀为知识树。

沉淀结构：

```text
[卷宗] 原文件名.pdf
├── PageIndex 一级节点
│   └── PageIndex 二级节点
└── PageIndex 一级节点
```

具体规则：

- 根笔记正文保存完整 OCR Markdown；如果 Markdown 文件不可读，则回退到数据库中的逐页 Page IR。
- 根笔记记录案件名、原文件名和可搜索 PDF 路径。
- PageIndex 节点保存标题、摘要和真实页码范围，并按原 PageIndex 父子关系生成子笔记。
- 根笔记和所有子笔记继续关联原案件。
- 根笔记通过通用链接指向原始文件；结构节点通过知识链接指向卷宗根笔记。
- 同一个文件重复沉淀会返回既有知识树，不重复制造内容。
- 单份文件最多沉淀 500 个结构节点，避免异常目录造成无上限写入。
- 新增“卷宗”知识分类和右栏“沉淀”面板。

## 五、Schema 与契约修正

- 新命令已加入 Tauri 命令注册。
- Specta 绑定生成了 `KnowledgeDocumentSourceDto` 和 `PageIndexImportResultDto`。
- 新命令加入前端 `CommandMap`，知识服务不再依靠裸 `unknown` 返回值。
- 发现已有 `MIGRATION_V22_SQL`，但 `CURRENT_SCHEMA_VERSION` 仍停在 21，导致两项迁移幂等测试失败；已统一为 22。

## 验证

- `npm run typecheck`：通过。
- `npm run test:unit`：2 个测试文件、9 项测试全部通过。
- `npm run build`：通过。
- `cargo check`：通过。
- `cargo test --lib`：120 项全部通过。
- 浏览器交互验证：
  - CodeMirror 编辑与快捷保存正常；
  - Wiki 出链和目标反链正常；
  - 历史列表和逐行差异正常；
  - OCR 模拟来源可沉淀为 1 个根笔记和 2 个 PageIndex 子笔记；
  - 沉淀后全部笔记从 5 变为 8，“卷宗”分类从 0 变为 3；
  - 1280px 窗口四栏布局没有裁切内部导航。

## 已知边界

1. 当前 Markdown 仍是源码编辑器，不是 Trilium 的 WYSIWYG 富文本模式；这是有意选择，符合用户要求的 Markdown 笔记模式。
2. Wiki 链接目前按标题精确匹配；同名笔记消歧、别名和重命名自动重写尚未实现。
3. 版本快照目前保存正文，不包含标题、标签和父级变更的完整结构快照。
4. OCR/PageIndex 沉淀后不会自动随原 PDF 重新 OCR 而覆盖知识树；后续应实现“来源有新版本”提示和人工确认刷新，避免静默覆盖律师笔记。
5. `npm audit --omit=dev` 仍报告 `html-to-docx -> image-size` 的两项既有高危拒绝服务漏洞。修复建议会强制降级 `html-to-docx` 到不兼容版本，未在本轮擅自执行。
