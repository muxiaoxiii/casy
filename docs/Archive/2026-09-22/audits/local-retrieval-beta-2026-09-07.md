# 本地检索与转换 beta

日期：2026-09-07。沿用 0.1.0 版本，本轮范围以用户最新决定为准：真实案卷由用户后续自行使用验收，Apple 开发者账号及发行手续暂缓；不重建 OCR 跨页表格，重点保证索引与人工编辑表格的 Word 导出。

## 检索方案

| 方案 | 存储思路 | 本轮结论 |
|---|---|---|
| Zvec | 嵌入式 C++ 向量数据库，Rust/C API；含 HNSW、DiskANN、IVF-RaBitQ、PQ-INT8 等 | 能量化和使用磁盘索引；在核对的文档功能中未发现 LEANN 式大部分向量丢弃后按需重算。本轮没有引入此依赖。 |
| LEANN | 保存裁剪图结构，搜索路径按需重算向量 | 以额外计算换磁盘；上游最高 97% 节省是其特定基准结果，不能直接套用到 Casy。当前 Python/原生图索引依赖和重算成本需要另行评测，本轮没有引入。 |
| Casy beta | 现有加密 SQLite，FTS + 逐段 INT8 向量 + 本地 Markdown 章节路径，RRF 融合 | 无需新数据库服务，保留文档与案件关联和索引版本管理；语义侧仍是线性扫描，未实现 ANN。 |

来源：

- https://github.com/alibaba/zvec/tree/44d8bc6f7c3d656efde2641c9410432ea14c1657
- https://github.com/StarTrail-org/LEANN
- https://huggingface.co/intfloat/multilingual-e5-base
- https://huggingface.co/Xenova/multilingual-e5-base/tree/1ec9243030a27d1a115d5c340572074c125b58b2

向量模型和向量数据库是两层，Zvec / LEANN 都不替代 embedding 模型。Casy 使用 E5-base INT8 ONNX，经 Rust 文档引擎和既有 ONNX Runtime 在本地 CPU 运行；无需 Ollama、API key、云端 token 或生成式摘要。PageIndex 部分沿用本地标题/页码树，并将实际章节路径加入向量输入，没有嵌入 PageIndex 上游 LLM 检索代理。

选择 base 而非 small 的原因：小规模合成法律资料比较中，small 对法语查询中文专利正文发生误排，base 在相同查询下通过。通用文件名会干扰短正文，因此文件标题保留在全文索引，向量使用正文和实际章节路径。不能据此宣称 base 在所有法律资料上最优。

模型和 tokenizer 合计 295,266,822 字节，随完整包分发。固定下载版本和 SHA-256 见 `tools/casy-doc-engine/install-models.mjs`；MIT 许可见 `docs/compliance/E5-MIT-LICENSE.txt`。

768 维向量从 3,072 字节 FP32 改为 772 字节（4 字节格式头 + 768 字节 INT8），向量载荷减少约 75%。10 万段约 77.2 MB，仅指向量，正文、元数据、SQLite 页和 WAL 另计。仍可读旧 FP32 数据；更换模型/分段配置后索引标记过期，需更新索引，文件系统上的数据库不会保证立即自动缩小。

长输入按 512 token 窗口、32 token 重叠处理，不静默截掉尾部；输入仍受每段 64 KB、每批 8 段限制。进程复用、超时及取消后重启已验证。检索等待超时会返回全文结果。

启用：设置 > AI > 知识库语义检索 > 内置本地 E5-base（多语言）> 保存并测试向量模型。默认不启用语义检索；知识库检索的索引任务页可手动更新全部索引，工作区的自动向量索引是单独开关。

## 文件转换与 Word 表格

- 顶部快速捕获旁的“文件转换”替代“添加任务”，窄屏保留图标入口。支持批量选择/拖入、输出目录、失败重试、移出队列和定位 Markdown；停止操作在当前文件完成后停止后续队列。
- 可选 PDF、PNG/JPG/JPEG/WEBP/BMP/TIF/TIFF、MD/MARKDOWN/TXT、DOC/DOCX/DOCM/RTF/ODT，沿用已有文本提取与本地 OCR。不是任意文件格式转换器，未新增 Excel/PPT 转换。
- 原件转换前后校验哈希，结果原子写入；已有同名文件自动追加序号。独立转换不登记案件或自动写知识库，输出仅 MD；原有案卷 OCR 的 PDF/MD 页码坐标绑定流程继续使用。
- 富文本/Markdown 编辑导出 DOCX 时，连续表头可跨页重复，正文行与超长单元格允许续页，支持横向和纵向合并。非法或不一致的合并结构明确报错，输出先暂存后替换，避免半成品。
- 嵌套引用内容可导出；Word 引用样式目前不保留编辑器四个来源的独立色彩与逐层缩进，不将这一点算作本轮表格分页能力。

## 验证

全部测试使用隔离资料库和合成文件；未访问或修改飞书，未操作正式资料库或 `/Applications/Casy.app`。

- `cargo test`：209 项库测试和全部默认集成测试通过。私有真实快照及真实向量测试默认 ignored；真实向量测试本轮另行显式运行通过。
- `npm run test:unit`：173 项、29 个测试文件通过；完整桌面构建包含 TypeScript 检查及生产前端构建。
- `local_retrieval_test`：五段不同主题的中文正文，中英德法日查询专利赔偿均命中目标首位；FP32/INT8 排序一致；长输入、取消后复用、772 字节存储验证通过。报告：`/Users/only/Documents/Casy-Local-Test/beta-retrieval-0907/retrieval.json`。
- 上述五段建索引 1,089 ms；首次查询 906 ms（含首次全文分词初始化），后续查询 11 至 20 ms。50,004 段线性扫描 341 ms，debug 构建且主要为重复合成向量，不能当作 5 万真实文档或百万级知识库性能保证。
- 转换端到端：`/Users/only/Documents/Casy-Local-Test/conversion-ui-yLSc95`，11 次真实 Rust 命令，损坏 DOCX 失败后修复重试、批量转换、原件保护、MD 导出 81 行表格和手机入口通过；完整性 ok、外键错误 0、页面错误 0。
- 检索端到端：`/Users/only/Documents/Casy-Local-Test/knowledge-index-profile-8umFzw`，向量配置、索引取消/失败/重试、混合检索、接口失败回退和手机返回后重开通过；完整性 ok、外键错误 0、页面错误 0。此项使用隔离接口验证网络协议，真实本地模型由上项独立验证。
- Word 表格：`/Users/only/Documents/Casy-Local-Test/beta-tables-0907/cross-page-table.docx`。84 行、两行表头、横纵合并及超长末行，用 LibreOffice 实际渲染为 21 页 PDF；21 页均含重复表头，OOXML 中保留 1,790 次正文片段，PDF 各独有汉字及句号也均为 1,790 次，末尾嵌套引用仍在。已检查第 2 页图像。未用 Microsoft Word 原生应用验收分页差异。

## 安装产物

本轮安装产物位于 `/Users/only/Documents/Casy-Delivery/2026-09-07-beta/`。完整包含 OCR、布局模型、PDF 渲染、字体、本地 E5 模型及依赖，不设 200 MB 上限。

- `Casy_0.1.0_aarch64.dmg`：779,343,083 字节；SHA-256：`08d8fd92737c5c5756f77a8dcb863c999805797ca5ccd794bc38ef4d911561cb`。
- 从 DMG 只读挂载提取同目录 `Casy.app`；`scripts/verify-bundle.mjs` 通过 2,412 个运行时文件的大小与 SHA-256 校验，总计 946,182,714 字节；ad-hoc deep/strict 签名及动态库依赖检查通过。
- 仅系统 PATH 下，OCR/坐标模型/PDF 渲染/可搜索 PDF 探测通过，并实际运行内置向量模型，两个输入均返回 768 维有限值。
- `--profile-dir /Users/only/Documents/Casy-Local-Test/beta-installed-2026-09-07` 原生启动通过；界面可打开文件转换，启用内置模型后“保存并测试向量模型”返回 `multilingual-e5-base-int8：768 维，连接成功`。正常退出并重新启动回到首页，隔离日志无 ERROR/panic。
- 本地提交：`2d7ebd2`（内置模型与紧凑检索）、`7be9876`（文件转换、跨页 Word 表格及界面回归）。本记录后续独立提交。生产前端构建时间晚于最后一次移动布局编辑，DMG 包含最终界面。

开发预览继续保留在 `http://127.0.0.1:1424/`，浏览器预览不等同于完整桌面 OCR 运行环境。

当前支持已验收的 macOS Apple Silicon，使用 ad-hoc 签名，无 Developer ID 公证；用户接受以此状态进行 beta 使用。真实办案、跨库质量基准和其他平台没有因为自动化测试通过就视为已验收。
