# Casy × Doco 对标研究与优化计划

> 日期：2026-09-03  
> 对标项目：[songofhawk/doco](https://github.com/songofhawk/doco)  
> 取证版本：`4cedec74825312105f56b96d01187ca6c4c65ca6`（2026-08-22）  
> 结论：**把 Doco 级编辑器与导出体验作为产品主线，同时吸收其内容内核和 Agent 安全写入协议；不直接移植 React/Yjs 整套架构。**

## 1. 一句话结论

Doco 对 Casy 有两层同等重要的价值。第一层是已经接近成品的编辑体验：纸张式正文、浮动工具栏、块操作、斜杠命令、目录、折叠、丰富节点以及集中式导入导出。第二层是支撑这些体验的内容协议：每个块有稳定地址，读取带版本，写入必须带前置版本，批量修改原子提交，重试具有幂等性，原生包负责无损迁移，Markdown/PDF/DOCX 是有明确降级说明的投影。

Casy 目前已经具备 WYSIWYG/Markdown、Wiki 双链、版本快照、OCR Markdown、可搜索 PDF、PageIndex 树和 AI 授权网关的分别实现，但编辑器仍属于“基础控件集合”，知识库也没有完整导出入口；底层则缺少贯穿这些能力的统一 Content IR、稳定块 ID 和版本化操作协议。因此需要同时推进可见的编辑器/导出完成度与不可见的内容可靠性，不能把其中任何一项长期推迟。

## 2. 本次调研边界

本次不是只读 README，而是核对了 Doco 的下列真实实现：

- `src/editor/components/BlockIdExtension.ts`：块 ID 分配、重复 ID 修复与撤销历史处理；
- `backend/document-schema.js`：服务端 schema、块 ID 校验、重复 ID 拒绝；
- `backend/ydoc-service.js`：Yjs 状态哈希、串行写入、`If-Match`、409 冲突；
- `backend/open-api/router.js`、`backend/open-api/idempotency.js`：块操作、批处理、幂等键；
- `backend/markdown.js`：自定义节点序列化与导出降级告警；
- `backend/native-transfer.js`：文档/目录/知识库原生包、附件校验、哈希和安全解包；
- `src/editor/DocoTextEditor.tsx`：独立编辑器边界与 ProseMirror 增量 steps；
- `backend/tests/open-api.test.js`、`backend/tests/native-transfer.test.js`：并发、重复块 ID、附件和迁移包约束。

这是源码级设计评估；没有执行 Doco 自身代码或部署服务，因此不把 README 中的全部功能宣传当作已做过运行验收的事实。

## 3. 值得吸收、暂缓和不应照搬的内容

| Doco 能力 | 对 Casy 的价值 | 决策 |
|---|---|---|
| `block_<ULID>` 稳定块地址 | OCR 页块、PageIndex 节点、证据引用、双链、AI Diff 都可指向同一个不会因移动而失效的地址 | **立即吸收** |
| 读取返回内容版本，写入要求前置版本 | 防止自动保存、AI 修改、版本恢复互相静默覆盖 | **立即吸收** |
| 原子 batch + Idempotency-Key | AI/批量整理失败可安全重试，不生成半棵知识树或重复笔记 | **立即吸收** |
| 前后端双重 schema 校验 | 不能只相信编辑器输出；Rust 落库前必须校验块类型、ID、附件、引用 | **吸收原则，改为单一契约生成** |
| 原生无损包 + 投影格式降级告警 | 解决附件、关系、OCR 坐标、PageIndex、历史无法由纯 Markdown 完整承载的问题 | **立即吸收** |
| 编辑器独立组件、宿主决定存储 | 降低知识库/文书工坊之间的编辑器重复和页面耦合 | **吸收边界设计** |
| `/` 命令、块拖拽、折叠、快捷键、目录与文档设置 | 构成日常写作是否顺手的核心，不是锦上添花 | **首批吸收，与稳定块模型同批实现** |
| 单文档/目录/整库导入导出入口 | 让内容真正可带走、可交付、可恢复 | **首批建设单文档导出，随后补齐整库导出** |
| Yjs + Hocuspocus + 浏览器 IndexedDB | Doco 面向 Web 多用户；Casy 当前是本地单用户 Tauri/SQLCipher，直接引入会增加双存储和恢复复杂度 | **近期不引入** |
| 29 个 MCP 工具和开放 REST 服务 | Casy 需要的是受授权的本地能力，不是先扩大写入口数量 | **按用例最小化建设** |
| React 版 `doco-text-editor` 直接嵌入 | Casy 是 Vue，跨框架运行时、样式和生命周期成本高；也会绕开现有 Markdown 保真工作 | **不直接依赖，移植模式与交互** |
| Mermaid/PlantUML、Callout、增强表格 | 对研究笔记、证据关系和复杂文书有直接价值 | **纳入完整编辑器目标** |
| 内嵌电子表格、微信专用导出、完整协同权限 | 电子表格可作为后续高级块；微信专用适配和协同权限不是 Casy 主线 | **核心编辑器稳定后按需实现** |

补充：Doco 仓库声明 MIT 许可，法律上允许复用和修改，但复制实质代码时仍需保留版权与许可声明。本计划优先移植架构模式；如后续直接移植具体实现，应建立第三方代码清单。

## 4. 当前 Casy 的真实基础与缺口

### 4.1 已有基础，不应推倒重来

1. **Markdown/WYSIWYG 已有可工作的保真基线**
   - `MarkdownWysiwygEditor.vue` 已有 TipTap、WikiLink、表格、任务列表、图片和 Raw HTML 占位；
   - `mdBridge.ts` 已负责 Markdown/HTML 双向转换与安全预览；
   - 本次执行两个定向前端测试文件，31 项全部通过。

2. **双链已由 Rust 统一维护**
   - `sync_wiki_links` 以 `[[标题]]` 为事实源，并与手动关系、PageIndex 结构关系分域管理；
   - 恢复版本时会重新同步 Wiki 链接。

3. **版本历史具备最低可用闭环**
   - 正文真正变化时按编辑会话快照；
   - 恢复前另存当前正文，可反向找回；
   - 本次执行 `cargo test commands::knowledge --lib`，7 项知识库定向测试全部通过。

4. **OCR/PageIndex 已经能够沉淀到知识库**
   - OCR Markdown 作为根笔记；
   - PageIndex 节点生成层级子笔记；
   - 重复导入复用根笔记，孤儿树会清理；
   - 原始文件通过 `links` 与知识笔记关联。

5. **AI 授权网关已具备关键机制**
   - 案件/任务写入已在同一事务内校验并消费一次性 token；
   - 已校验工具、实体、目标 ID、前态哈希和真实 payload。

### 4.2 结构性缺口

| 缺口 | 当前表现 | 后果 |
|---|---|---|
| 无统一内容 IR | `knowledge_items.content` 是整篇 Markdown；PageIndex 节点又被建成独立 knowledge item | 编辑器块、OCR 区域、索引节点、证据引用没有共同地址 |
| “知识层级”与“正文块”混用 | `parent_id/block_type` 同时承担笔记树和块级化意图 | 未来拖动正文块、引用段落、移动笔记容易混淆语义 |
| 无写入前置版本 | `update_knowledge(id, serde_json::Value)` 未要求 revision/hash | 自动保存、历史恢复、AI 写入可发生最后写入者覆盖 |
| 版本只保存正文 | `knowledge_versions` 只有 `content` | 标题、层级、标签、关联、附件与来源状态不能一致恢复 |
| 编辑器块 ID 不持久 | TipTap 内容最终序列化为 Markdown，未见稳定块 anchor 契约 | AI Diff 与证据引用只能依赖文本位置/标题，移动后易失效 |
| OCR 来源粒度不足 | 根笔记保留文件和 PageIndex 摘要，但正文段落没有统一页码/bbox/模型版本引用 | 搜索命中后无法稳定回到原 PDF 的精确证据位置 |
| AI 知识写入未统一收口 | `update_knowledge` 本身没有 origin/proposal token/pre-state 契约 | 将来新增 AI 知识编辑入口时可能绕过案件/任务已有的安全模型 |
| 导出没有能力清单 | Markdown/DOCX/PDF 输出没有统一“保留/降级/丢失”报告 | 用户会把投影格式误认为完整备份 |
| 契约仍偏动态 | `update_knowledge` 使用 `serde_json::Value` 和字符串字段映射 | PATCH 空值语义、字段漂移和错误码难以稳定治理 |

当前源码中的 `list_knowledge_document_sources` 只保留了一次案件表关联；后续仍需用真实数据库命令测试验证来源列表、导入与重启重读，不能只凭 SQL 静态检查判定该链路完成。

## 5. 编辑器与导出的目标状态

### 5.1 编辑器不是“Markdown 文本框加工具栏”

Casy 的目标应明确为一套完整的文档工作台，而不是只保证 Markdown 能输入和保存。对标 Doco 后，首个可用版本至少应覆盖：

| 区域 | 目标体验 |
|---|---|
| 页面骨架 | 居中纸张式正文、可编辑标题、稳定宽度与留白、长文档不卡住；笔记树可折叠，正文保持专注 |
| 文本格式 | 标题 1–4、正文、粗体、斜体、下划线、删除线、高亮、行内代码、链接、对齐 |
| 块类型 | 引用、有序/无序/任务列表、代码块、分隔线、Callout、图片、表格、Mermaid、PlantUML |
| 块操作 | 悬浮块柄、拖动、上下移动、复制、删除、插入下方、类型转换、折叠；多块选择时行为可解释 |
| 快速输入 | `/` 命令、中文及拼音搜索；粘贴 Markdown 时询问按富文本解析还是纯文本粘贴 |
| 长文导航 | 自动目录、当前标题高亮、标题多级编号、折叠状态持久化、块链接复制 |
| 上下文工具 | 选区浮动工具栏、链接编辑器、图片工具栏、表格行列/合并/对齐工具 |
| Casy 专属节点 | WikiLink、案件引用、文件/证据引用、法条引用、OCR 原文块、PageIndex 区段、AI 建议块 |
| 状态反馈 | 正在保存/已保存/保存失败/存在冲突必须清楚显示；字数、来源状态和当前版本可见 |
| 编辑模式 | 所见即所得为默认；Markdown 源码和分栏模式仍保留，三种模式共享同一份当前内容 |

布局可以吸收 Doco 的“左侧知识树 + 中央文档画布 + 按需浮出的目录/设置/历史”，但 Casy 还需要一个可收起的右侧上下文面板，承载双链、案件、证据来源、历史和 AI Diff，避免把法律工作流塞进通用格式工具栏。

### 5.2 导出必须是编辑器的一等能力

编辑器右上角应提供统一“导入/导出”入口，并先刷新当前编辑事务，保证导出的就是屏幕上的最新版本：

| 格式 | 范围 | Casy 目标 |
|---|---|---|
| Markdown | 单篇、文件夹、整库 ZIP | 保留正文、WikiLink、块 anchor 和本地附件相对路径；返回降级告警 |
| DOCX | 单篇、选定笔记集合 | 使用现有 Rust `docx-rs` 结构化导出，不使用 HTML 冒充 Word；支持标题、列表、表格、图片、证据引用与中文字体 |
| PDF | 单篇、选定笔记集合 | 有打印预览、页面尺寸、页边距、页眉页脚、页码和字体设置；不只截取编辑器 DOM |
| HTML | 单篇 | 自包含或资源目录两种模式，输出经过清洗，可选择 Casy 主题 |
| Casy 原生包 | 单篇、文件夹、整库 | 无损保存 Content IR、附件、双链、案件关系、历史、OCR/PageIndex 来源与哈希 |
| 可搜索证据 PDF | OCR 来源笔记 | 直接打开/导出已生成的 searchable PDF，不把笔记渲染 PDF 与证据原文件混为一类 |

每次导出都应先显示能力报告：哪些内容完全保留、哪些转换、哪些会丢失；完成后显示目标路径，并提供“打开文件”和“在文件夹中显示”。

### 5.3 对标 Doco 的体验，但不照抄其导出简化

Doco 当前源码中，Markdown 导出是直接生成 `.md`；PDF 使用浏览器端 `html2pdf`；所谓 Word 导出实际把编辑器 HTML 包成 `application/msword` 并下载为 `.doc`。这些方式让入口显得完整，但不等同于高保真办公文档输出。

Casy 应把 Doco 作为 UI/工作流标杆，同时在输出质量上超过它：

- DOCX 统一走 Rust 结构化序列化，并做 Word/WPS 实际打开验证；
- PDF 使用确定性的排版模型，不能依赖当前窗口宽度或隐藏 UI 状态；
- Mermaid/PlantUML、Callout、复杂表格等节点定义逐格式适配策略；
- 原始证据 PDF 与“由笔记生成的 PDF”在命名、入口和来源信息上严格区分；
- 单篇导出在第一阶段可用，整库无损包随后补齐，不再把所有导出压到路线图末端。

## 6. 目标内容模型

### 6.1 两层对象必须分开

- **Notebook/Page 层**：一个可命名、可分类、可移动、可关联案件的知识条目；沿用 `knowledge_items`。
- **Content Block 层**：页面内部的标题、段落、列表、表格、图片、引用、OCR 区域；新增稳定块模型，不再用子 knowledge item 模拟每个正文块。

建议新增：

```text
knowledge_items
  id / title / category / parent_id / linked_case_id / ...
  current_revision / content_hash / schema_version
  content                  # 兼容期物化 Markdown，用于现有 UI/FTS
  document_json            # 新 Content IR，迁移完成后为权威内容

knowledge_blocks           # 从当前 revision 派生的可查询索引
  block_id                 # block_<ULID>，全库稳定唯一
  item_id / parent_block_id / order_key / node_type
  plain_text / attrs_json / source_ref_json

knowledge_versions
  id / item_id / revision / parent_revision
  snapshot_json / markdown_projection / content_hash
  change_reason / actor_type / actor_id / proposal_id / created_at

knowledge_operations
  operation_id / item_id / base_revision / result_revision
  idempotency_key / actor_type / proposal_id / operations_json / created_at
```

`knowledge_items.parent_id` 表示笔记树；`knowledge_blocks.parent_block_id` 表示正文结构。两者不得再混用。

### 6.2 Content IR 最小范围

Content IR 首版覆盖 Casy 已有节点以及第一批完整编辑能力：

- paragraph、heading、blockquote、bullet/ordered/task list；
- code block、table、image、horizontal rule、callout；
- Mermaid、PlantUML（存源代码和渲染配置，渲染图为衍生件）；
- wiki link、case/file/evidence reference；
- raw HTML placeholder；
- OCR region、PageIndex section。

每个块至少包含：

```json
{
  "id": "block_<ULID>",
  "type": "paragraph",
  "attrs": {},
  "content": [],
  "sourceRef": {
    "kind": "pdf",
    "fileId": "...",
    "sourceSha256": "...",
    "jobId": "...",
    "page": 12,
    "bbox": [0.1, 0.2, 0.8, 0.3],
    "ocrEngine": "ovisocr2+ppocrv5",
    "ocrModelVersion": "...",
    "pageIndexNodeId": "..."
  }
}
```

人工笔记的 `sourceRef` 可以为空；OCR 生成块必须保留来源文件哈希、处理任务、页码、坐标和模型版本。这样全文搜索、引用、AI 回答、证据预览和重新 OCR 才能共享同一条可追溯链路。

### 6.3 Markdown 的定位

不再同时宣称 Markdown 与富文本 JSON 都是事实源：

- Content IR 是结构、块地址和关系的权威事实源；
- Markdown 是用户可读、可编辑、可导出的主要投影；
- Markdown round-trip 子集通过 `<!-- casy:block=block_xxx -->` 保留块地址；
- 无法无损表达的属性在导出时返回 warnings，不静默丢失；
- 兼容迁移期继续物化 `knowledge_items.content`，供现有 FTS 与旧命令读取，禁止两边独立修改。

## 7. 统一写入协议

### 7.1 读取

读取页面必须返回：

```text
item + document + revision + content_hash + capabilities
```

`content_hash` 对规范化后的 Content IR 计算，而不是对易受空白影响的 HTML 计算。

### 7.2 语义操作

不要把 ProseMirror steps 直接作为永久业务协议；它们可用于编辑器内部增量同步，但跨版本、AI 和审计应使用稳定语义操作：

- `insert_block_after`
- `insert_block_before`
- `replace_block`
- `replace_text`
- `delete_block`
- `move_block`
- `set_block_attrs`
- `set_page_metadata`
- `link_reference` / `unlink_reference`

每次写入包含 `base_revision` 或 `base_content_hash`、`idempotency_key` 和 1–100 个 operations；Rust 在单个数据库事务中完成校验、应用、重建派生索引、保存历史、同步链接、记录审计并更新 revision。

冲突必须返回结构化错误和当前 revision。UI 或 Agent 重新读取并展示待重放差异，不能自动盲写覆盖。

### 7.3 AI Diff

AI Diff 应改为稳定块上的 Proposal，而不是一段“看起来像 diff”的文本：

```text
读取 snapshot(revision/hash)
  → AI 生成 semantic operations
  → 用户逐块预览/接受/拒绝
  → Rust 校验 proposal + target + payload + pre-state
  → 同一事务消费 token 并应用 operations
  → 生成新 revision 和审计记录
```

原则：

- AI 只能提出操作，不直接拿任意 JSON 更新知识表；
- 被批准的操作、真实执行操作和最终目标必须完全一致；
- 版本漂移返回冲突，原提案失效或进入人工 rebase；
- 一个 idempotency key 只能对应一个请求体；
- 执行失败时 token 与知识内容一起回滚；
- OCR 来源块的 `sourceRef` 默认不可被 AI 改写，只能显式派生新的解释块。

## 8. 分阶段实施计划

### Phase 0：修复基线并冻结契约（2–3 天）

- 补一个真实数据库命令测试，覆盖来源列表、导入、重启后重读；
- 为知识写入定义 typed DTO 和结构化错误码，停止继续扩展 `serde_json::Value`；
- 记录当前 Markdown round-trip 支持矩阵和已知降级；
- 以当前源码构建的 Tauri 应用完成一次原生 smoke test，明确不是旧安装包。

验收：来源选择能真实打开；现有 31 个编辑器测试和 7 个 Rust 知识测试继续通过；运行证据包含当前 commit/dirty diff 与应用版本标识。

### Phase 1：完整编辑器骨架、单篇导出与稳定块（7–10 天）

- 将知识编辑器重组为“文档画布 + 标题 + 状态栏 + 编辑器扩展工厂 + 宿主面板”结构；
- 加入选区浮动工具栏、块柄、块移动/复制/删除/类型转换、`/` 命令、目录和标题编号；
- 提供 Markdown、DOCX、PDF、HTML 四个单篇导出入口；导出前必须 flush 当前编辑内容；
- DOCX 接入现有 Rust rich export，PDF 建立独立打印视图，不能直接照搬 Doco 的 HTML `.doc` 和 DOM 截图方案；
- 定义 Content IR v1 和 Rust 权威 validator；
- 新增 `knowledge_blocks`、revision/hash 字段和迁移；
- TipTap 为支持节点分配 `block_<ULID>`，移动保持、复制/粘贴生成新 ID；
- 后端拒绝非法或重复块 ID；
- Markdown 加入可选 anchor round-trip；
- `update_knowledge` 拆为 typed metadata patch 和 versioned content mutation。

验收：用户不进入源码模式即可完成一篇包含标题、列表、表格、图片、引用和 WikiLink 的笔记；块拖动、折叠、模式切换、保存、重启后 ID 不变；四种单篇导出都使用最新编辑态，DOCX 可由 Word/WPS 打开，PDF 分页稳定；过期 revision 写入必定失败且不会覆盖新内容。

### Phase 2：丰富节点、原子操作与完整历史（7–10 天）

- 补齐 Callout、代码高亮/复制、图片尺寸和对齐、完整表格工具、Mermaid/PlantUML；
- 完成 Markdown 智能粘贴、中文/拼音 `/` 命令和全套键盘快捷键；
- 实现 semantic operations 与 batch apply；
- 引入 idempotency key/request hash；
- 历史版本升级为整页快照，包含元数据、IR、关系摘要和来源；
- 恢复通过同一 operation service 执行，不再直接 UPDATE 正文；
- 历史面板按块显示变更，可点击跳转具体块。

验收：Doco 核心编辑器功能矩阵逐项核对；复杂节点保存、重启、三模式切换后不丢失；批处理中任一操作失败则全部回滚；同键同请求重试不重复写；同键不同请求返回冲突；恢复后标题/正文/链接/来源一致。

### Phase 3：OCR/PageIndex 与稳定内容统一（7–10 天）

- OvisOCR2 Markdown 与坐标 OCR 统一生成 Content IR；
- 每页、标题、段落、表格与 OCR region 获得稳定块 ID 和 `sourceRef`；
- PageIndex 节点引用块集合，不再复制一份容易漂移的摘要正文作为事实源；
- 搜索结果返回 `item_id + block_id + page + bbox`；
- PDF 预览支持从命中块跳到原页/区域；
- 重新 OCR 生成新 revision，并输出块对齐/失配报告，禁止静默替换人工修订。

验收：任意 OCR 搜索命中可回到原 PDF 精确页；重新 OCR 后旧引用可解释地保持、迁移或标为 orphan；无真实模型时不写 completed。

### Phase 4：AI Diff 收口（5–8 天）

- AI 读取接口返回块、版本、来源能力，不返回无约束整表写权限；
- Proposal 保存 semantic operations 与 pre-state；
- 知识写入接入现有 Rust 授权网关；
- 实现逐块接受/拒绝、冲突重读、人工 rebase；
- MCP 首批只开放 `read/search/propose`，执行仍走应用内授权。

验收：直接 IPC 模拟 AI origin 无 token 必须拒绝；篡改 operation/payload/target 必须拒绝；旧 revision 必须拒绝；授权成功只执行一次并留下历史与审计。

### Phase 5：整库无损包与批量投影（4–6 天）

- 定义 `casy-knowledge-package` v1：manifest、Content IR、附件、关系、OCR/PageIndex 元数据、来源哈希；
- ZIP 解包做路径穿越、数量、体积、重复 ID、哈希和 schema 校验；
- 导入时重新分配本地资源 ID，并维护 source→target 映射；
- 把 Phase 1 的单篇 Markdown/DOCX/PDF/HTML 导出扩展到文件夹、选定集合和整库；
- 所有投影导出统一返回 capability/warnings；
- 明确“原生包是备份，Markdown/PDF/DOCX 是投影”。

验收：整本知识库导出后可在空库恢复，附件/双链/案件关联/OCR 来源数量一致；篡改或缺附件的包被拒绝；投影降级在 UI 可见。

### Phase 6：高级块与编辑体验打磨（3–5 天，可与后期测试并行）

- 评估并实现内嵌电子表格或轻量数据表块；
- 打磨多块选择、跨块复制、复杂表格、图片拖放和无障碍键盘路径；
- 完善块链接复制、引用原 PDF、主题和沉浸写作模式；
- 将编辑内核抽成 Vue 组件，知识库和文书工坊共享扩展工厂与 host contract。

验收：键盘和鼠标路径都可完成常用操作；中文输入法不触发误提交；两处宿主不再各自注册冲突插件。

### Phase 7：协同能力仅做条件评估（不进入当前承诺）

只有出现“同一知识页需要多设备/多人同时编辑”的真实需求后，才评估 Yjs。届时要求：

- Yjs 只能作为同步适配器，不能另建与 SQLCipher 竞争的事实源；
- 加密、备份、离线恢复、版本审计先形成设计；
- 先做单文档原型和故障注入，再决定是否引入 Hocuspocus。

## 9. 预计投入与优先级

以一名熟悉当前 Casy 的开发者全职估算，不含 OCR 模型精度调优：

| 里程碑 | 范围 | 粗估 |
|---|---|---:|
| M0 编辑与单篇导出形成可用闭环 | Phase 0–1 | 2–2.5 周 |
| M1 丰富编辑和历史真正可靠 | Phase 2 | 1.5–2 周 |
| M2 OCR/PageIndex 真正可引用 | Phase 3 | 1.5–2 周 |
| M3 AI 可安全共编 | Phase 4 | 1–1.5 周 |
| M4 可备份迁移 | Phase 5 | 1 周 |

推荐按两条紧耦合主线推进：**编辑器/单篇导出的可见完成度**与**稳定块/版本协议的内容可靠性**在 Phase 1 同时落地；随后是丰富节点与历史 → OCR/PageIndex 统一 → AI Diff → 整库无损包。UI 不能再被视为最后的“体验增强”，但也不能只抄工具栏而不解决保存、冲突和导出保真。

## 10. 测试与发布门槛

### 必须自动化

- Content IR schema/normalize/重复 ID/循环/附件引用测试；
- Markdown ↔ IR 往返 corpus：中文、混合语言、表格、任务、脚注、WikiLink、Raw HTML；
- DOCX/PDF/HTML golden fixtures：标题编号、分页、中文字体、图片、表格、Callout、Mermaid/PlantUML；
- 版本冲突、原子 batch、幂等重试、失败回滚测试；
- OCR sourceRef、重新 OCR 对齐、PageIndex 引用完整性测试；
- AI 无 token、错 token、错 payload、错 target、过期 revision、重复执行测试；
- 原生包路径穿越、压缩炸弹上限、缺文件、哈希不符、版本不兼容测试。

### 必须原生验证

- 用当前工作区源码构建并启动 Tauri，而不是浏览器 preview 或旧安装包；
- 中文输入法连续编辑、模式切换、切换笔记、自动保存、重启恢复；
- 大型 OCR 笔记编辑和检索时的卡顿、内存和数据库锁；
- 搜索结果跳转 PDF 页码/bbox；
- AI Proposal 从预览、批准到执行后的真实数据与历史记录。
- Word 与 WPS 打开 DOCX，系统 PDF 阅读器检查分页、字体和可复制文字；
- 导出前未失焦的最新输入、模式刚切换后的内容和恢复版本后的内容都不能导出旧状态。

浏览器测试可以覆盖组件与协议，但不能再作为设置、文件系统、OCR sidecar、SQLCipher、原生菜单和窗口卡顿的最终验收。

## 11. 明确不做

- 不直接把 Doco React 编辑器塞进 Vue/Tauri；
- 不为“对标功能数量”一次性开放大量 MCP 写工具；
- 不在单用户写入仍可能覆盖时引入多人协同；
- 不把 Markdown、Content IR、Yjs 三套状态同时当事实源；
- 不把可搜索 PDF、OCR Markdown、PageIndex 摘要互相复制后各自独立更新；
- 不把投影导出称为无损备份；
- 不因静态测试通过就宣称原生应用已好用。
- 不以“已有基础按钮”为由降低完整编辑器目标，也不把导出长期留到最后补齐。

## 12. 最终判断

Doco 可以显著帮助 Casy，它既是文档内核协议的参考，也是编辑器完成度和导入导出工作流的直接产品标杆。Casy 现有工作没有白做：编辑器、双链、历史、OCR/PageIndex、原生 DOCX 导出能力和授权网关都可以保留；需要重构的是共同内容语言，同时需要把当前偏基础的编辑界面提升到真正可长期写作的状态。

完成 Phase 0–1 后，知识库首先要从“能编辑的笔记页面”升级为“用户愿意每天写、并能马上导出交付”的文档工作台；完成 Phase 2 后，它同时具备不会轻易覆盖、可追溯、可恢复的内容基础；完成 Phase 3–4 后，OCR 证据、PageIndex 检索和 AI Diff 再连成 Casy 独有的法律工作流。多人协同可以延期，但完整编辑器和导出能力不能延期。
