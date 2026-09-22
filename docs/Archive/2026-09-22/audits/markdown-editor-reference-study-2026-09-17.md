# Markdown 编辑器参考研究：Casy、Rust 原生路线与 ProseMirror 路线

日期：2026-09-17。范围：技术筛选、源码与测试审查、Casy 实际编辑组件的隔离验证。没有修改产品实现。

## 结论

1. 有值得借鉴的 Rust 原生编辑器，但本轮证据不足以认定任何候选已完整达到 Typora 的交互质量。Velotype 值得研究源码与可视内容的位置映射；Ferrite 值得研究焦点、编辑会话和字素边界。
2. 对现有 Casy，最容易转化成改进的是 Kuku 的预览生命周期、选区与输入法测试方法，以及 Milkdown/Crepe 的公式、图表、图片交互。它们的编辑主体是 Web/ProseMirror，不是 Rust。
3. 当前 Casy 确有内容保真缺口。相关现有 50 项测试通过，但新增样例发现表格对齐丢失，以及五、六级标题、脚注、数学公式缺少可靠的导入/保留策略。实际组件复核说明：只修改文末，也会影响前面未改的内容。
4. 第一优先级应是明确内容模型和保真契约，第二是完善连续编辑体验，第三才是接入 Typst 做精确排版。三者可以形成同一产品方向，但不是更换一个渲染引擎就能同时解决。
5. 暂不建议整体迁移 GPUI/egui，也不建议现在直接替换 TipTap。保留这一判断的前提是用同一组场景验证现有内核；如果原型证据显示其他内核明显更好，应重新选型。

## 证据口径

- **本机验证**：Casy 现有定向测试；11 个内容样例的真实 TipTap 创建、追加编辑、序列化及重开；其中 4 个通过实际 Vue DocumentEditor 组件复核。
- **隔离验证**：Kuku 两个小模块及其原始测试复制到临时目录，使用 Casy 已有 Vitest/jsdom 执行。没有修改模块与测试逻辑；仅复用测试运行环境。3 项输入法事件保护测试、7 项预览调度测试通过。
- **源码审查**：Ferrite、Velotype、Kuku、Typora Lite 的指定模块。没有编译或运行 Ferrite/Velotype 的完整应用，也没有跑 Kuku 全套应用测试。
- **文档筛查**：Aster、vdmark、MD Preview。它们不是本轮深度源码结论的依据。
- **未验证**：真实 macOS/Windows 输入法、原生候选框位置、真实鼠标跨块拖选、屏幕阅读器、原生长文输入延迟、安装包内存。jsdom 和模拟事件不能证明这些能力。
- 没有把已关闭的历史 issue 当作当前版本仍存在的问题；也没有把 README 的性能数字当作本机测量结果。

固定源码版本：

| 项目 | 审查提交 | 编辑架构 |
|---|---|---|
| [Ferrite](https://github.com/OlaProeis/Ferrite) | `3ba085c561670342d72c560efbf6b0b92b5c0b46` | Rust + egui，源码编辑器与渲染块编辑 |
| [Velotype](https://github.com/manyougz/velotype) | `ed65977be94f2f2703037fcb8b6cbab2e7579571` | Rust + GPUI，块树与行内显示映射 |
| [Kuku](https://github.com/kuku-mom/kuku) | `ea07b629ead912ada70442e8c87ae90359742d33` | Tauri + SolidJS + ProseKit/ProseMirror |
| [Typora Lite](https://github.com/JustinGastby/typora-lite) | `54ab235805ae426d4b7b522380fca0b2415742e3` | Tauri + React + Milkdown/Crepe |

Casy 按当前工作区文件验证，包含此前已有未提交修改，不能仅用 Git HEAD 代表本次基线。

## 对“Typora 体验”的具体定义

“能显示 Markdown”“能点开一个块修改”“同一表面连续所见即所得编辑”是不同能力。应检查：

- 输入 `**`、链接或公式标记时，光标附近的源码怎样出现/隐藏；样式变化是否改变光标位置。
- 中英文混输、组合字符、emoji、中文拼音候选未确认时，退格和快捷键是否只作用于正确区域。
- 从段落拖选到列表、图表和表格，剪切与粘贴是否完整；跨块操作能否一次撤销并恢复选区。
- 切换源码/可视模式是否保留内容、选择方向和位置；不支持的语法是否明确保留。
- 图片加载、图表异步渲染、主题变化后，当前阅读/输入位置是否稳定。
- 保存与重开是否保留语义；只修改一处是否造成无关部分的格式重写或内容损失。

Rust 有利于构建高效的文本与渲染基础，但不会自动提供这些行为。GPUI/egui 也不是可以直接放进 Vue 的编辑器组件。

## 候选项目的具体借鉴点

### Velotype：最值得研究的原生位置映射路线

[行内投影](https://github.com/manyougz/velotype/blob/ed65977be94f2f2703037fcb8b6cbab2e7579571/src/components/block/runtime/projection.rs) 区分纯文本、带样式文本、开闭标记和链接目标，并保存 `clean_to_display_cursor`、`display_to_clean` 等偏移映射。

[文档级源码映射](https://github.com/manyougz/velotype/blob/ed65977be94f2f2703037fcb8b6cbab2e7579571/src/editor/source_mapping.rs) 处理列表前缀、代码围栏等额外语法产生的位置差异。[输入接口](https://github.com/manyougz/velotype/blob/ed65977be94f2f2703037fcb8b6cbab2e7579571/src/components/block/input.rs) 再把 GPUI 的 UTF-16 范围转换到内部 UTF-8 范围，并区分组合输入与正式提交。

这是接近 Typora 的关键基础：显示字符、源码字符和操作系统输入位置必须能互相对应。对 Casy 可借鉴位置与选区设计；如果继续使用 ProseMirror，应以其 transaction mapping 和文档位置为基础，不能混用 Rust 的字节偏移。

[撤销实现](https://github.com/manyougz/velotype/blob/ed65977be94f2f2703037fcb8b6cbab2e7579571/src/editor/history.rs) 保存源码、选区和时间，对连续输入分组，并在恢复文档后恢复选区。[编辑测试](https://github.com/manyougz/velotype/blob/ed65977be94f2f2703037fcb8b6cbab2e7579571/src/editor/tests.rs) 覆盖输入撤销、清空重做、模式切换后保留表格位置和嵌套图片等。它不只是一个静态预览演示；但本轮没有执行这些 GPUI 测试。

边界：README 仍列“完善 IME 行为”为待办；查询时 [#82](https://github.com/manyougz/velotype/issues/82) 仍开放，用户报告 Windows/Rime 候选输入中退格会删除已有正文。该报告针对 0.6.0，不能直接推断所有平台或当前 main 都有同样问题。

### Ferrite：焦点与编辑会话很有参考价值，交互模型不同

[RenderedEditSession 源码](https://github.com/OlaProeis/Ferrite/blob/3ba085c561670342d72c560efbf6b0b92b5c0b46/src/markdown/rendered_session.rs) 为每个标签页维护活动块和编辑缓冲；块切换时提交旧缓冲，再激活新块。带格式段落可进入 raw Markdown TextEdit。其“渲染编辑”不能直接等同于整篇连续富文本编辑。

[架构说明](https://github.com/OlaProeis/Ferrite/blob/3ba085c561670342d72c560efbf6b0b92b5c0b46/docs/technical/markdown/rendered-edit-session.md) 解释了稳定 widget ID、避免每次输入按内容哈希重建身份，以及外部修改时通过 epoch 失效缓冲的做法。表格、代码块仍有不同的提交路径，双栏也不是任意并发编辑自动合并。

[撤销提交模块](https://github.com/OlaProeis/Ferrite/blob/3ba085c561670342d72c560efbf6b0b92b5c0b46/src/markdown/rendered_commit_undo.rs) 的设计是按提交边界保存前后源码快照，而非假定所有块天然共享一个统一历史。应借鉴“明确提交边界”的思想，Casy 则应继续让结构变更进入同一 ProseMirror 历史。

[字素边界测试](https://github.com/OlaProeis/Ferrite/blob/3ba085c561670342d72c560efbf6b0b92b5c0b46/src/editor/ferrite/grapheme.rs) 涉及 ZWJ emoji、组合重音和韩文字母。说明光标移动不能简单按字节或 Unicode scalar 切分。测试存在不等于本轮已验证其平台输入体验。

边界：README 标注 macOS 为实验性。大文件宣传要分清源码模式和渲染模式；源码编辑的虚拟滚动不能直接证明含大量表格、图片和图表的可视编辑同样快。文档也记录了大文件下关闭部分功能以减少开销的取舍。

### Kuku：对现有 Casy 最直接的工程参考

[编辑引擎](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/components/editor/system/editor_engine.ts) 基于 ProseKit，使用插件注册节点和行为；Casy 的 TipTap 与它共享 ProseMirror 底层，不需要迁移整个 UI 框架才能借鉴。

优先借鉴：

1. [图表预览调度](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/plugins/builtin/core_editor/code_block_preview_scheduler.ts)：接近可见区域才执行、判断任务是否仍有效、离开时清理 observer。
2. [代码块 NodeView](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/plugins/builtin/core_editor/nodes/code_mirror_node_view.ts)：区分源码编辑与非编辑预览，使用渲染 token、预留高度、滚动锚点恢复。比“插入 SVG 后重新布局”完整得多。
3. [输入法事件测试](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/components/editor/system/__tests__/ime_composition_workaround.test.ts)：覆盖异常事件顺序和组合输入中的选区更新。本轮隔离执行 3 项通过，但使用 fake view/合成事件，不是系统输入法验收。

不能照抄的部分：

- [IME workaround](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/components/editor/system/ime_composition_workaround.ts) 修改 ProseMirror 的内部 `docView.setSelection`，针对特定 WebKit/韩文输入异常。Casy 应先复现对应问题、核对版本，再决定是否需要补丁。
- [核心 Markdown 转换器](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/plugins/builtin/core_editor/markdown_handlers.ts) 的图片导出显式设置 `title: undefined`，表格核心映射未传递列对齐和合并跨度。不能把它当作 Casy 富文书模型的完整适配器；这是所读 handler 的边界，不是本轮对整个 Kuku 应用的运行结论。
- [往返测试](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/lib/markdown/__tests__/round_trip.test.ts) 的主要断言是第一次转换结果等于第二次转换结果。它证明幂等性，不能单独排除第一次丢失信息。[fallback 测试](https://github.com/kuku-mom/kuku/blob/ea07b629ead912ada70442e8c87ae90359742d33/apps/desktop/src/lib/markdown/__tests__/fallback.test.ts) 的契约允许结构降级，只保证文本。Casy 的证据锚点、任务 ID、表格跨度需要更强契约。

### Typora Lite / Milkdown：交互原型参考，不是 Rust 编辑引擎

[MilkdownEditor.tsx](https://github.com/JustinGastby/typora-lite/blob/54ab235805ae426d4b7b522380fca0b2415742e3/src/components/MilkdownEditor.tsx) 直接创建 Crepe，启用 Table、Latex、CodeMirror、ImageBlock 等功能；Mermaid 接到 CodeMirror 的预览回调，默认优先显示预览。

可以参考“正文中看到结果、局部进入源码”的交互，特别是图表和公式。若评估整个 Milkdown 替换，则必须证明证据引用、任务关联、合并表格、图片尺寸和现有保存恢复流程能迁移。它与 TipTap 都建立在 ProseMirror 上，换包装层不等于消除全部输入法或内容模型问题。

## Casy 本机内容验证

使用 11 个小样例，经 `mdToHtml → documentExtensions/TipTap → 文末追加段落 → htmlToMd → 重开`，保留初始/重开 JSON、序列化 HTML 和 Markdown。四个重点样例另用实际 DocumentEditor 组件复核。

| 样例 | 实际观察 | 判定 |
|---|---|---|
| 五、六级标题 | 导入为 paragraph；修改后保存去掉 `#####` / `######` | 当前只支持 H1–H4；超范围内容没有保留原结构 |
| 脚注 | `[^law]` 被解析为普通引用链接，脚注正文进入 href，正文中不再显示脚注定义 | 需独立支持或原样保留，不能继续误解析 |
| 数学公式 | `$a*b*c$` 内 `b` 成为 italic；`$x_i + y_i$` 保存时下划线被转义 | 缺少公式节点/语法保护；不是数学语义保真 |
| 表格左/右对齐 | 初始 JSON 有 `align: left/right`，重开后变成 null | 已确认往返属性损失 |
| Mermaid | language=mermaid 和源码保留 | 源码保真，不代表已有图表可视编辑 |
| 引用式链接 | 转为行内链接，目标 URL 和 title 保留 | 语义保持，源码格式发生规范化 |
| 带 title 的图片 | src、alt、title 在该样例中保留 | 当前样例通过；没有测试图片实际解码 |
| 合并表格 | colspan=2 保留 | 当前样例通过；不能推广为所有表格属性都保留 |
| 从 3 开始编号 | 起始编号保留，空白有变化 | 语义保持，格式规范化 |
| 未识别 HTML | details/summary 通过 rawHtmlBlock 保留 | 有效保护；但当前 PDF 导出会拒绝此节点 |
| 生僻字、组合重音、emoji | 字符串保存重开保持 | 仅内容往返；没有测试候选输入、光标字素边界或字体显示 |

表格对齐丢失的路径可以在代码中对应：编辑器序列化输出 `style="text-align: right"`；富 HTML 重开经过 DOMPurify 白名单时没有保留 style；单元格对齐缺少像段落 `data-text-align` 那样的保真通路。修复时应保留明确的对齐属性，不能直接放开全部 style 来规避过滤。

脚注样例：

```markdown
主张依据[^law]。

[^law]: 第六十五条。
```

只在文末追加“追加核查”，保存结果为：

```markdown
主张依据[^law](%E7%AC%AC%E5%85%AD%E5%8D%81%E4%BA%94%E6%9D%A1%E3%80%82)。

追加核查
```

实际组件中，不编辑时 `flushAndGetMarkdown()` 会返回原始输入；因此“打开→保存未丢失”的测试不足以覆盖“编辑一处→整篇回写”。还需同时检查编辑器内部结构，避免只因原文缓存存在就误判导入正常。

证据文件：[完整样例与输出](markdown-editor-reference-study-2026-09-17/observations.json)、[实际组件复核](markdown-editor-reference-study-2026-09-17/component-observations.json)、[Kuku 定向测试摘要](markdown-editor-reference-study-2026-09-17/upstream-test-summary.json)。

## 建议的落地方向与先后关系

### 先定义保真契约，再扩大支持范围

- 对知识笔记，区分 Markdown 语义保真与源文件字节级保真。普通空白规范化可以接受与否，需要显式约定；不能与丢失脚注、表格属性混为一谈。
- 对正式文书，结构化节点/属性要能表达证据锚点、任务引用、图片尺寸、表格跨度及版式。纯 CommonMark/GFM 无法表达全部这些内容，需要自定义节点及明确的可移植编码。
- 不支持的输入应作为带原文的占位节点保留，或在进入可视编辑前明确提示。已有 rawHtmlBlock 提供了部分基础。
- 统一解析/序列化责任，优先评估直接 AST ↔ 编辑文档转换；这不会自动带来无损效果，每个节点和属性仍需对应处理。
- 新测试要同时断言正文、结构、关键属性和二次幂等，覆盖“修改其他段落”。

### 在现有编辑器内完善交互，再决定是否替换

为代码/公式/图表设计局部源码编辑与结果预览，保存源码及稳定节点身份，处理焦点进出和跨节点删除。参照 Kuku 加入渲染失效、预留高度、清理及滚动保护。大图表异步更新不得抢走光标，也不得覆盖较新的源码结果。

跨块操作、AI 应用修改、插入图片/表格应使用统一文档事务与历史，不另造一套与 ProseMirror 脱节的撤销栈。切换视图后恢复语义位置；不能只保存滚动百分比。

性能方面，当前文书 HTML 模式在每次内容更新中调用 `getHTML()`，Markdown 模式采用 400ms 防抖，大纲采用 180ms 防抖扫描。它们是需要测量的整篇处理点；本轮没有证据证明某个具体文档大小已造成卡顿，也没有给出速度排名。

### 精确分页作为同一文档的排版视图

Typst 可承担 A4 预览与 PDF 输出，编辑输入仍由编辑模型处理。首版采用切换/并排预览，并建立块级定位；不能把整个 PDF 每次更新后替换显示就称为“原地分页编辑”。如果未来需要像 Word 一样直接在纸张页上输入，应单独验证连续选区、输入法和跨页表格方案。

## 下一次选型实验的验收矩阵

统一用同一份样例分别运行当前 Casy、改进原型和候选，不能拿源码编辑的性能与富文书编辑比较。下面是待执行的实机验收要求，不是已通过项目。

| 维度 | 最小操作 | 要记录的结果 |
|---|---|---|
| 中文输入 | macOS 拼音/Rime，拼音未确认时退格、Esc、方向键、切换窗口 | 是否误删正文、候选框位置、提交次数、撤销粒度 |
| Unicode | 生僻字、组合重音、ZWJ emoji 两侧移动/退格 | 是否拆坏字素；不能仅比较最终字符串 |
| 跨块选区 | 段落→嵌套列表→表格，拖选、剪切、撤销 | 文本/结构、光标位置、选择方向 |
| 语法边界 | 粗体/链接/公式前后输入、移入移出 | 标记显隐、插入点和样式归属 |
| 模式切换 | 在表格单元格中选词，切源码再返回 | 内容不丢，位置可恢复 |
| 异步预览 | 同一图表连续改两次，延迟第一个结果，期间继续输入和滚动 | 旧结果不覆盖新结果，用户主动滚动不被拉回 |
| 内容保真 | 修改与脚注/表格不相关的段落，再保存重开 | 原内容、属性、引用、公式源码保持 |
| 长文 | 10KB、100KB、1MB 中文混合文本；固定表格/图片/图表数量 | 输入延迟分布、长任务、内存、保存耗时，注明冷热状态 |
| Casy 特有内容 | 证据页码锚点、任务 ID、合并单元格、图片宽度 | 导入、编辑、保存、导出链路全部保持 |

建议先实现最小对照原型：一份 Casy 文档、一个带公式和图表的编辑面，以及上述保真测试。只有在同一场景下出现明确收益，才讨论整体迁移 Milkdown 或原生编辑内核；不以语言、星数、截图或“已支持 Markdown”替代证据。

## 复现命令与验证范围

当前基线：

```sh
npm run test:unit -- tests/unit/MarkdownWysiwygEditor.test.ts tests/unit/mdBridge.test.ts tests/unit/documentExport.test.ts
```

结果：3 个文件，50 项测试通过。

本研究的持久化探针：

```sh
npx --no-install vitest run --config docs/audits/markdown-editor-reference-study-2026-09-17/vitest.config.ts
```

该探针会更新同目录下 `observations.json`、`component-observations.json`。它检查研究样例能运行、追加内容可重开，以及真实组件和转换路径一致；**探针通过不表示 11 种语法全部保真**，具体损失见上表和 JSON。

Kuku 原始 10 项测试在临时目录中执行，依赖为 Casy 已安装的 Vitest 4.1.11/jsdom；结果摘要注明了模拟环境限制。没有安装或升级 Casy 依赖，也没有修改其编辑实现。

其他筛查来源：[Aster](https://github.com/kumarUjjawal/aster)、[vdmark](https://github.com/cedar12/vdmark)、[MD Preview](https://github.com/vorojar/md-preview)。后两者分别是 Tauri/Vditor 和 Rust/wry 系统 WebView 路线，不能因仓库含 Rust 就归为原生 Rust 编辑内核。
