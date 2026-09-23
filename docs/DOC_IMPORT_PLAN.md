# 旧 Word `.doc` 导入方案（anydoc 主路径 + Rust 侧车兜底）

日期：2026-09-23。对应更新计划 P2「旧 Word `.doc` 导入」。不改变法定期限、OCR 模型或备份语义。

## 背景与澄清

- **anydoc 一直支持 `.doc`**：自 0.1.x 起即含 MS-DOC（OLE2/FIB/piece table/SPRM/列表/表格）解析；`Format::from_extension("doc")` 与 `to_markdown_bytes(..., Format::Doc)` 都是正式能力。
- 本仓库现锁定 **anydoc 0.2.4**（上游 firecrawl/anydoc；0.2.x 含自研 Excel、公式 LaTeX、PDF 需 OCR 页上报等）。
- 文案「无法解析此旧版 Word 文件，请另存为 DOCX」**只表示该文件解析失败**，不表示产品不支持 `.doc`。
- 卷宗/转换入口早已接受 `.doc`；失败路径才要求用户另存 DOCX。

因此侧车定位是 **anydoc 失败时的兜底**，不是替代 anydoc 的 `.doc` 能力。

社区包 [jitOffice/doc2docx](https://github.com/jitOffice/doc2docx) 提供的是另一条 **`.doc → .docx` 格式转换** 能力（非语义解析）：

| 层 | 归属 | 许可 |
| --- | --- | --- |
| Node 调用壳 | JitOffice | MIT（本项目不用） |
| 原生 `doc2x` 二进制 | 派生自 **b2xtranslator**（C#/.NET） | **BSD-3-Clause** |

本方案 **只重写 JitOffice 那层工程壳**（判型、临时目录、超时、失败三重校验、平台分发），**不**从零实现 MS-DOC 解析器；主解析仍是纯 Rust 的 anydoc。

## 目标与非目标

**目标**

1. 常见 `.doc` 由 anydoc 0.2.4 直接出 Markdown。
2. anydoc 失败时，侧车转为合法 `.docx` 再走 anydoc。
3. 侧车缺失、超时、半成品产物时，仍回落到现文案（另存 DOCX），不静默吞失败。
4. 二进制与 BSD-3 声明进入 runtime manifest / licenses，离线可装。
5. 判型与产物校验不信任侧车退出码（转换器异常时仍可能 exit 0）。

**非目标**

- 不实现浮动图/艺术字/EMF 矢量全覆盖（与 doc2docx 已知边界一致，写入文档）。
- 不替换 anydoc 的 `.docx/.rtf/.odt` 路径。
- 不引入 LibreOffice / Node 运行时。
- 不把 `.doc` 直接当 OCR PDF 输入。

## 数据流

```text
用户文件 *.doc
    │
    ▼
anydoc 0.2.4 Format::Doc  ──成功──► Markdown → 分段/索引
    │失败
    ▼
classify（ZIP 透传 / CFB+WordDocument / 加密拒绝）
    │
    ▼
runtime/bin/doc2x  [input] -o [out.docx]   默认超时 120s
    │
    ├─ [E] 行 / 无产物 / 非 ZIP / 缺 word/document.xml → 失败
    ▼
anydoc::to_markdown_bytes(docx, Docx)
    │
    ▼
segment_markdown → 现有 job/产物路径
```

若侧车不可用或转换失败：输出含「请另存为 DOCX」与双路径错误详情的 `DOCUMENT_PARSE`。

## 组件

| 路径 | 职责 |
| --- | --- |
| `src-tauri/Cargo.toml` `anydoc = "=0.2.4"` | 主 `.doc` 解析 |
| `scripts/prepare-doc2x.mjs` | 按平台钉住 npm 子包，校验 SHA-256，解出 `bin/doc2x` + LICENSE |
| `src-tauri/runtime/bin/doc2x` | 开发/打包用可执行文件（git 不入库大二进制，由脚本准备） |
| `src-tauri/runtime/licenses/doc2x-BSD-3-Clause.txt` | 随包声明 |
| `src-tauri/src/parse/doc2docx.rs` | 判型、调用、超时、产物校验 |
| `scripts/prepare-runtime.mjs` | 调用 prepare-doc2x，纳入现有 runtime 清单 |
| `src-tauri/src/parse/text_document.rs` | anydoc 主路径 + 侧车兜底 |

## 判型规则（对齐 doc2docx JS 壳，仅侧车路径）

1. 长度 &lt; 8 → 拒绝。
2. ZIP 魔数 `PK` → **passthrough**（扩展名写错的 `.docx`）。
3. 非 CFB 魔数 `D0 CF 11 E0 A1 B1 1A E1` → 拒绝。
4. **全缓冲区** UTF-16LE 扫描 `EncryptedPackage` → 加密拒绝（目录扇区可落在文件任意位置，禁止只扫文件头）。
5. 全缓冲区扫 `WordDocument` → 否则「不是 Word 二进制文档」。
6. 否则调用侧车。

## 产物校验（不信任退出码）

1. 子进程输出含 `[E]` → 失败。
2. 显式 `-o` 路径（或工作目录中非输入文件的 `.docx`/`.docm`）且 size&gt;0 → 取产物。
3. 产物以 ZIP 开头。
4. 字节中出现 `word/document.xml`（ZIP 成员名为明文，无需解压）。

## 超时与资源

- 默认 `120_000 ms`，可用 `CASY_DOC2X_TIMEOUT_MS` 覆盖。
- 独立临时目录，结束删除；超时 kill 子进程。
- 不常驻、不监听端口。

## 平台与钉版本

`prepare-doc2x.mjs` 钉住 `@jitword/doc2docx-<platform>@0.1.0`，与 Zvec 相同模式：缓存 tarball、SHA-256 校验、解包到 `runtime/bin/doc2x`（Windows 为 `doc2x.exe`）。开发可用 `CASY_DOC2X_PATH` 覆盖可执行文件。

| 平台 | 包 | tarball SHA-256 |
| --- | --- | --- |
| darwin-arm64 | `@jitword/doc2docx-darwin-arm64@0.1.0` | `503cfe45ae93885aece34a0c3fdef799bdc73a81610c1aa123291a60b08d5ce8` |
| darwin-x64 | `@jitword/doc2docx-darwin-x64@0.1.0` | `37d051390b70aab6edfeb60934a456d9a02e42753195e77127bdc18208077ce9` |
| linux-x64 | `@jitword/doc2docx-linux-x64@0.1.0` | `0d0a82def78f68baf8cb53de8d40b91cd22f7c9357634d71ff4c35b9b990ae71` |
| linux-arm64 | `@jitword/doc2docx-linux-arm64@0.1.0` | `ec758e8c81f10df5d67cbd0d0615c64a5ff318eb8d804466eff4875ef65fb081` |
| win32-x64 | `@jitword/doc2docx-win32-x64@0.1.0` | `6b9c7c6e725d2019e86ebdaae6c13b7b51de772110a9ae486ff2dd364ca8bae5` |

体积约 34MB/平台，归入 runtime `document-runtime` 组；licenses 归入 `compliance`。

## 许可

- anydoc：MIT（crate）。
- 转换器二进制：BSD-3-Clause（b2xtranslator 派生），声明文件随 `runtime/licenses/` 分发，`Casy-dependencies.md` 索引。
- 本仓库 Rust 壳：与 Casy 相同许可。
- 不把 Node 包或 Node 运行时打进桌面应用。

## 验收

1. 单元：判型（ZIP/CFB/加密/非 Word）、缺 `[E]`/缺产物/非 ZIP/缺 `document.xml` 的失败路径。
2. 集成：`anydoc_parses_legacy_doc_without_sidecar`（无侧车也能出正文）；`extracts_legacy_doc_when_sidecar_ready`（主路径经 extract_markdown）。
3. `prepare-doc2x` 幂等 + 校验和不匹配失败。
4. 类型检查与既有前端/Rust 套件不回归。

## 已知边界

- anydoc `.doc`：部分复杂版式/图形可能简化；加密文档拒绝。
- 侧车：浮动/锚定图片、艺术字、复杂分栏、EMF/WMF 可能丢失或简化。
- 侧车独立使用场景上游仍标注试验性；生产前用脱敏合同/公文自测。
- 不替代 OCR 对扫描件/PDF 的路径。
- 跟进 anydoc 上游：`cargo update -p anydoc` 时阅读 Release Notes，回归 `.doc` 与 PDF `NeedsOcr`。
