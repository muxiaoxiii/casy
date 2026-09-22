# Casy

本地优先的律师案件工作台，覆盖案件、程序期限、任务、日历、卷宗、知识笔记、文书与事实白板。

文档核对日期：**2026-09-22**，以当前工作区源码为准。历史记录位于 [docs/Archive](docs/Archive/README.md)。

## 版本与交付

| 对象 | 状态 |
| --- | --- |
| 当前源码 | **0.1.2，未发布**；大文件与转换超时改动尚待收尾 |
| 已交付安装包 | **0.1.1**，Apple Silicon / macOS 26+ |
| 数据库 | SQLCipher / SQLite，schema **41** |
| 分发 | 本地 beta，ad-hoc 签名，未公证 |

0.1.1 导出的 Markdown 已将 base64 配图另存到 `casy-images-…` 文件夹；该包仍有文本 64 MiB 上限和转换前端 3 分钟等待上限。0.1.2 源码已有对应改动，不能据此认为安装包已更新。用户当前要求暂停修复和打包，先整理文档、审阅及规划。

## 能力与边界

| 模块 | 当前实现 | 边界 |
| --- | --- | --- |
| 案件与程序期限 | 案件关系、送达事件、责任方、期限、跨案统筹、审计 | 有适用条件，不代表全部法律程序已编码 |
| 任务与日历 | GTD、子任务、重复任务、独立日程、排期、庭审联动 | 外部服务需配置并核验送达/同步结果 |
| 卷宗 | 归档、目录协调、按页 OCR、表格/版面、来源定位、区域校订、可搜索 PDF | 复杂图表与公式保留原图；非全链路有界内存 |
| 知识与文书 | Markdown/富文本、版本恢复、全文/向量检索、统一编辑器、Typst PDF、DOCX | 引用层级回归有 1 项失败；导出保真有边界 |
| 白板 | Excalidraw 场景、事实来源、修订冲突保护、历史 | 不等同于完整双时间图谱或自动法律推理 |
| 收件箱 | 文字/文件捕获、文本剪贴板、录音保存、归卷、知识入口 | 截图、自动剪贴板监听、转写及部分批量/重试仍为占位 |
| 外部服务 | AI/Ollama、飞书、WebDAV、CalDAV、IMAP/SMTP、MCP | 需显式配置；WebDAV 总览状态仍是占位 |
| 备份 | 数据库快照、含附件的加密完整备份及恢复 | 数据库备份与完整备份需区分 |

## 技术组成

- Tauri 2；Vue 3 / TypeScript / Element Plus / Pinia / Vue Router；Rust 后端。
- SQLCipher、FTS5；Zvec FP16 HNSW；多语言 E5-base INT8（768 维）。
- TipTap / ProseMirror、CodeMirror；Typst、MiTeX、Mermaid、KaTeX；Excalidraw。
- OCR：Rust `casy-doc-engine`、**PP-OCRv6 medium**、韩文候选 **PP-OCRv5 mobile**、**PP-DocLayout Plus-L**；Poppler 渲染；Noto CJK 字体。
- 当前初始化 10 个业务插件，包括 workspace；最终写操作由 Rust 命令执行。

历史 Ovis/PaddleOCR-VL 计划和探测字段不代表当前默认 OCR 管线。

## 开发入口

推荐 Node.js 24；Rust manifest 最低要求 1.95。需要目标平台依赖和 Zvec 原生库。

```bash
npm ci
node scripts/prepare-zvec.mjs
npm run tauri -- dev
```

完整离线打包使用 `npm run build:desktop -- --bundles dmg`；普通 `tauri build` 不等同于完整资源交付。隔离测试和构建说明见 [开发与验证](docs/DEVELOPMENT.md)。

## 文档

- [文档索引](docs/README.md)与[当前状态](Casy-STATUS.md)
- [架构与模块](docs/ARCHITECTURE.md)、[文档处理](docs/DOCUMENT_PIPELINE.md)
- [代码审阅](docs/CODE_REVIEW_2026-09-22.md)、[后续更新计划](docs/UPDATE_PLAN.md)
- [程序期限](docs/procedure-deadline-rules.md)、[运行时分发](docs/runtime-distribution-plan.md)、[数据与安全](docs/DATA_AND_SECURITY.md)

代码入口：`src/modules` 与 `src/shared` 为界面；`src/core` 为服务/插件/IPC；`src-tauri/src` 为数据库和领域逻辑；`tools/casy-doc-engine` 为文档引擎；`scripts` 为资源准备与校验；`tests`、`src-tauri/tests` 为测试。`release` / `outputs` 是本地产物，不是自动发布证明。
