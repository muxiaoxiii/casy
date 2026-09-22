# 编辑与排版一体化 Walkthrough

日期：2026-09-17。对应 [Workplan](editor-typesetting-workplan-2026-09-17.md) 与 [前期研究](markdown-editor-reference-study-2026-09-17.md)。本轮在已有脏工作区增量实施，没有重置既有修改，没有提交或发布安装包。

## 交付结果

保留 TipTap，完成内容保真、公式/Mermaid 局部编辑、Rust Typst 排版三个阶段的首版。文书工坊工具栏新增 **A4 预览**；打开后显示真实 Typst 页面，修改正文后自动更新，点击页边定位按钮返回对应编辑块。PDF 导出与预览共享结构化文档、版式参数和编译路径。

这不是“编辑器已经完美”的结论，也不以 Rust 或 Typst 的存在证明 Typora 级体验。此次具体修复的是往返内容损失和缺失的编辑/排版闭环。

## 内容模型与保存

- `semanticMarkdown.ts`、`semanticNodes.ts`：新增行内/块公式、脚注引用和脚注定义。Markdown 解析时优先识别语义，公式中的 `*`、`_` 不再进入普通强调解析，脚注定义不再成为链接 URL。
- 原始标记与公式/脚注源码分别保存。编辑源码会清除旧标记；通过外部命令修改语义属性时，序列化也会校验旧标记，避免把旧公式写回来。无法用常规数学分隔符表达的内容使用编辑器的安全 HTML 语义载体往返。
- H1–H6 均进入标题节点。表格对齐、列宽、合并跨度保留；`data-text-align`、`data-colwidth` 使用受限值，不放开任意 CSS。
- `MarkdownPreservation` 根据不可变 ProseMirror 顶层节点身份保留原块源码；未修改块直接复用，修改块才转换。普通引用链接的定义独立保留，删除其后段落不会顺带删除定义。
- 移除会改写公式/原始块内部空行的全局连续换行压缩。

保真边界：这里保证所支持节点的语义及可映射未修改块的原文，不是任意 Markdown 文件的字节级编辑器。多节点对应的原始 token、已修改的复杂块会重新序列化，普通链接定义可能移到文末。未知 HTML 继续以既有原始节点保留，PDF/Word 遇到不支持节点明确拒绝。

## 局部编辑与预览

`SourceNodeView` 为公式、Mermaid 和脚注提供源码面板、应用/取消、失焦提交、Escape 取消与 Mod+Enter 应用。提交进入同一 ProseMirror transaction/history，支持原有撤销。

源码草稿在应用前留在节点内。定时自动保存只同步已提交正文，不关闭正在输入的源码面板。显式保存、导出和模式切换会先提交草稿；输入法仍在 composition 状态时拒绝提前快照，并提示先完成选字。知识库窗口退出和切换模式也处理这一边界。

`previewScheduler` 使用视区观察延迟渲染、离开视区清理、返回后恢复、节点销毁清理、异步结果版本检查和有限缓存。保留预览占位高度，对视口上方的高度变化补偿滚动位置。KaTeX 和 Mermaid 按需加载；公式关闭 trust，Mermaid 使用 strict 安全模式，输出仍经 DOMPurify。

浏览器验收发现 Mermaid 默认 HTML 标签被 SVG 清洗剔除，已改为 SVG 文本标签。没有照搬 Kuku 对 ProseMirror 内部 `docView` 的输入法补丁。

## Rust 排版与 A4

核心在 `src-tauri/src/docsy_engine/typesetting.rs`：

1. 直接遍历编辑器 JSON，生成转义后的 Typst 内容；不把用户普通文本当 Typst 源代码运行。
2. 内存 World 只允许生成的主文档和显式收集的资源，拒绝其他源码、包或文件访问。图片沿用原生导出的本地图片加载规则。
3. 支持段落、H1–H6、常见文本格式、列表/任务列表、引用、代码块、公式、脚注、表格、图片、分隔线和引用显示文字。表格处理跨度、相对列宽及重复表头。
4. MiTeX 转换公式；Rust Mermaid 使用严格解析及显式中文字体。图表输出为矢量资源。
5. 重复脚注引用共享同一条脚注；未引用的定义仍在原位置保留显示。缺失/重复定义、未知节点、不支持的文本格式、非法链接协议、缺字、缺图及编译错误明确失败。
6. 版式包含 A4、页边距、额外装订边、首行缩进、标题/案号页眉、首页隐藏页眉及总页数页脚。页边距接受 10–40 mm，额外装订边 0–15 mm。
7. 单例字体索引和最近一次编译缓存。键含生成内容、版式与资源字节；PDF 与 SVG 页面来自同一次布局。没有声称 0ms 编译。
8. 限制输入大小、节点数、嵌套深度、图表/公式长度、图片大小和输出页数，避免无限制导入。

`pdf_export.rs` 已从手写 PDF 绘图改为该引擎的适配层；已有原子写文件流程保留。新增 `preview_editor_document` IPC、对应 TypeScript 契约及本地命令测试桥。

字体验收不能只看文字提取：现有可变字体在本机 Typst SVG 路径中出现方框，但 PDF 文字能正常提取。已改用固定提交的静态 `NotoSansCJKsc-Regular.otf`，与现有其他模块使用的字体并存。`prepare-runtime.mjs` 增加固定大小和 Git blob 校验，字体使用既有 Noto OFL。最终 SVG 与 PDF 栅格化均人工检查了中文和图表标签。

## 编辑块与页面定位

`TypesetPreview.vue` 显示真实 SVG 页面，以 blob URL 载入。Rust 为顶层文档节点生成 metadata 锚点，并从 Typst introspector 取得页号及坐标。

- 编辑选区变化：取顶层块索引，定位对应页上的锚点。
- 页边定位按钮：返回该块附近的合法 ProseMirror 选区并滚入视口。
- 更新防抖 550ms；一次只保留一个运行请求和最新待编译快照。旧结果不会覆盖新文档。
- 旧预览明确标为过期，更新期间禁用定位；组件销毁后撤销 blob URL。窄窗口上下排列。

定位粒度是顶层块。跨页长表格/列表只定位其起始位置，尚未实现任意字符或表格内部单元格的反向点击映射。

## 验证结果

| 检查 | 结果 |
|---|---|
| `npm run test:unit` | 54 个测试文件、257 项通过 |
| 保存边界追加回归 | 22 项通过，含两项新增的自动保存/组合输入边界测试；覆盖自动保存不关闭源码面板、显式保存提交及组合输入阻止切换 |
| `npm run build` | 类型检查与生产构建通过；仍有大 chunk 提示，Mermaid 使用动态导入 |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib docsy_engine:: --offline` | 20 项通过，含中文搜索、分页、重复表头、90 个块的实际所属页、公式、重复/未引用脚注、图片、图表、缓存与拒绝非法输入 |
| `cargo build --manifest-path src-tauri/Cargo.toml --example knowledge_local_bridge --offline` | 通过 |
| 完整 Rust lib 测试 | 初次 248 通过、8 失败、1 忽略；其中 7 项 MCP 测试因沙箱无法监听端口，允许本机端口后这 7 项全部通过 |
| 既有日期测试 | `test_all_natural_language_dates_recognition` 在 2026-09-17 对“九月十五日截止”期待 2026-09-15，现有解析逻辑返回下一年的 2027-09-15；相关测试/解析逻辑未被本轮修改 |
| 本地浏览器 E2E | 实际 Vue 文书工坊 + 原生 Rust 命令；三页 SVG、公式应用及保存、双向定位、页边距修改、PDF 导出、1600/800 宽度截图通过，无 pageerror |
| 第三方 notices | `node scripts/prepare-notices.mjs` 成功整理 1345 个依赖；核对新增依赖及 `typst-assets/NOTICE` 中字体声明 |
| 工作区差异 | `git diff --check` 通过 |

Rust 新依赖引入额外类型比较实现后暴露了几处 `Option<&str>` 与 `Some(&String)` 的推断歧义，已在原位置显式使用 `.as_str()`，不改变业务判断。

E2E 用临时资料目录，文书列表/保存由隔离内存夹具提供，排版及 PDF 导出调用真实 Rust 命令。它验证了编辑到原生排版的链路，不是完整安装包或真实资料库迁移测试。原生保存对话框以明确测试输出路径替代。

证据：

- [自动化脚本](../../tests/e2e/editor-typesetting-local.mjs)
- [E2E 结果与耗时](editor-typesetting-validation-2026-09-17/results.json)
- [桌面截图](editor-typesetting-validation-2026-09-17/a4-desktop.png)
- [窄窗口截图](editor-typesetting-validation-2026-09-17/a4-narrow.png)
- [PDF 第一页栅格化](editor-typesetting-validation-2026-09-17/pdf-page-1.png)
- [实际 PDF](editor-typesetting-validation-2026-09-17/typeset-sample.pdf)
- [实际第一页 SVG](editor-typesetting-validation-2026-09-17/page-1.svg)

记录中的三页样例，Rust 首次调用约 1913ms，后两次约 461/464ms；包含字体、转换、排版、PDF/SVG 输出。测试桥每次另起进程，首次墙钟时间还含隔离数据库初始化，且当时并行跑构建/测试。这不是 release 性能基准，也没有测出参考项目宣称的 0.5–5ms。

## 当前限制

- Word 暂不支持新增公式/脚注节点，会明确失败；建议此类文档使用 PDF 或 Markdown，不会默默省略。
- 脚注正文作为保留源码的文本排版，尚无嵌套富文本脚注编辑。修改引用标识不自动重命名定义；缺定义时 PDF 拒绝生成。
- 内部证据/知识链接在 PDF 保留显示文字，尚未生成可携带 Casy 内部跳转或证据附件的归档包。
- 前端 Mermaid 与 Rust Mermaid 是两个实现，视觉风格及语法覆盖可能不同；严格解析失败会保留源码并报错，不能宣称全部 Mermaid 兼容。
- 本轮版式设置在当前文书工坊会话内使用，未增加每份草稿的持久化版式字段；页眉宏、横向纸张和司法模板库仍可在此基础上扩展。
- 系统中文输入法候选框、macOS Tauri WebKit、Windows、可访问性和超长文档延迟尚未完成真实平台验收。合成 composition 事件测试不能替代这些检查。
- 本轮没有构建/签名/发布桌面安装包。下次分发需运行正常 runtime 准备流程，以包含新增静态字体与更新的许可资源。
