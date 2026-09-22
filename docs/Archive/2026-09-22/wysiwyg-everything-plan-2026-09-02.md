# 知识库与文书生成 WYSIWYG 化计划

> **日期**: 2026-09-02 · **状态**: 执行中
> **起因**: 用户要求知识库与文书生成模块全部改为所见即所得（WYSIWYG），不再直面 Markdown/HTML 源码。

## 现状盘点

| 模块 | 现状 | 目标 |
|---|---|---|
| 知识笔记本 `KnowledgeNotebookView` | CodeMirror Markdown 源码编辑（编辑/分栏/预览三模式） | **tiptap WYSIWYG 为默认编辑模式**，保留「源码」模式兜底 |
| 旧知识视图 `KnowledgeView` 块表单 | 纯 textarea | 紧凑 WYSIWYG |
| 文书工坊/写作 `DocWorkshopView`/`WritingView` | tiptap WYSIWYG ✅ | 已是，不动 |
| 文书生成 `DocumentGenView` | 生成结果只读渲染（v-html）/ 纯文本 tab | 增加「所见即所得编辑」：生成稿直接在 tiptap 中编辑（HTML 原生，无需 MD 桥） |

## 技术方案

- **依赖**（已装）：`marked`（MD→HTML）+ `turndown`（HTML→MD）+ `@types/turndown`
- **MD 桥** `src/shared/markdown/mdBridge.ts`：
  - 入方向：marked 自定义 tokenizer 把 `[[标题]]` 转为 WikiLink 节点 HTML（与 `docs/extensions/WikiLink.ts` 的 parseHTML 规则对齐）
  - 出方向：turndown 自定义规则把 WikiLink 节点还原为 `[[标题]]`
  - 保真要求：标题/粗斜体/行内码/代码块/列表/任务列表/引用/链接/表格 往返无损；不支持语法的 HTML 透传不丢失
- **编辑器组件** `src/modules/knowledge/components/MarkdownWysiwygEditor.vue`：tiptap v3 + StarterKit + Table/Highlight/TaskList + WikiLink（复用 docs/extensions，补全源=笔记标题）；v-model 为 Markdown 字符串（防抖序列化）；`compact` 模式供块表单复用
- **测试**：`tests/unit`（vitest）MD 往返用例 ≥8 条（含 wiki 链接、嵌套列表、表格、代码块）

## 分工（2 个并行子代理）

- **Agent K（知识库）**：mdBridge + MarkdownWysiwygEditor + KnowledgeNotebookView 集成（WYSIWYG 为默认编辑，「源码」切 CodeMirror，分栏=WYSIWYG+预览）+ KnowledgeView 块表单 + 往返测试
- **Agent D（文书生成）**：DocumentGenView 生成稿 WYSIWYG 编辑（复用 LegalEditor 或内嵌 tiptap，保存回写 + 导出走编辑后内容）

## 纪律（同既有批次）
文件所有权隔离；禁动 schema/mod.rs/lib.rs/router/commandMap/locales 与其他模块；文案内联中文、禁 emoji、CSS 变量暗色兼容、motion tokens；`vue-tsc` 只修自己文件；完成后汇报文件清单与集成点。

## 门禁
`vue-tsc` 零错 + vitest 全绿（含新往返测试）+ `cargo test` 不回归 + `npm run build` 通过 → 更新 walkthrough。
