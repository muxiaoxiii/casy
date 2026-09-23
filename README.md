# Casy

本地优先的律师案件工作台，覆盖案件、程序期限、任务、日历、卷宗、知识笔记、文书与事实白板。

文档核对日期：**2026-09-23**，以当前工作区源码为准。历史记录位于 [docs/Archive](docs/Archive/README.md)。

## 版本与交付

| 对象 | 状态 |
| --- | --- |
| 当前源码 | **0.1.3，生产验证版** |
| 已交付安装包 | **0.1.3**，Apple Silicon / macOS 26+ |
| 数据库 | SQLCipher / SQLite，schema **42** |
| 分发 | 本地生产验证包，ad-hoc 签名，未公证 |

0.1.3 修订已恢复独立主题，新增黄宣纸纹理，并在 WebDAV 页面接通全部数据备份与恢复。修订包使用 `themes-webdav` 文件名标记。

0.1.3 集成 Judicial Docket（卷宗墨卷）视觉系统，修复导出图片迁移、转换超时误报、期限规则事务和同步状态。用户导出的 Markdown 配图存入 `casy-images-…` 目录；64 MiB 文本转换硬限制已移除。实际验收范围与剩余限制见 [项目状态](Casy-STATUS.md)，生产验证版不等同于已完成公证、跨平台和全部外部服务验收。

当前源码另已完成日历布局修复及独立甘特排期，尚未重新打包；现有 DMG 不包含这两项更新。甘特计划不改变任务截止或法定期限，验证范围见[甘特记录](docs/Archive/2026-09-23/gantt-planning/README.md)。

## 能力与边界

| 模块 | 当前实现 | 边界 |
| --- | --- | --- |
| 案件与程序期限 | 案件关系、送达事件、责任方、期限、跨案统筹、审计 | 有适用条件，不代表全部法律程序已编码 |
| 任务与日历 | GTD、子任务、重复任务、独立日程、横向甘特排期、庭审联动 | 外部服务需配置并核验送达/同步结果 |
| 卷宗 | 归档、目录协调、按页 OCR、表格/版面、来源定位、区域校订、可搜索 PDF | 复杂图表与公式保留原图；非全链路有界内存 |
| 知识与文书 | Markdown/富文本、版本恢复、全文/向量检索、统一编辑器、Typst PDF、DOCX | 引用层级回归已修复；导出保真有边界 |
| 白板 | Excalidraw 场景、事实来源、修订冲突保护、历史 | 不等同于完整双时间图谱或自动法律推理 |
| 收件箱 | 文字/文件捕获、文本剪贴板、录音保存、归卷、知识入口 | 截图、自动剪贴板监听、转写及部分批量/重试仍为占位 |
| 外部服务 | AI/Ollama、飞书、WebDAV、CalDAV、IMAP/SMTP、MCP | 需显式配置和真实服务验收；WebDAV 支持手动完整加密备份；旧快照同步仍限同密钥 |
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

完整离线打包使用 `npm run release:validation`；普通 `tauri build` 不等同于完整资源交付。隔离测试和构建说明见 [开发与验证](docs/DEVELOPMENT.md)。

## 文档

- [文档索引](docs/README.md)与[当前状态](Casy-STATUS.md)
- [架构与模块](docs/ARCHITECTURE.md)、[文档处理](docs/DOCUMENT_PIPELINE.md)
- [代码审阅](docs/CODE_REVIEW_2026-09-22.md)、[后续更新计划](docs/UPDATE_PLAN.md)
- [程序期限](docs/procedure-deadline-rules.md)、[运行时分发](docs/runtime-distribution-plan.md)、[数据与安全](docs/DATA_AND_SECURITY.md)

代码入口：`src/modules` 与 `src/shared` 为界面；`src/core` 为服务/插件/IPC；`src-tauri/src` 为数据库和领域逻辑；`tools/casy-doc-engine` 为文档引擎；`scripts` 为资源准备与校验；`tests`、`src-tauri/tests` 为测试。`release` / `outputs` 是本地产物，不是自动发布证明。

2026-09-23 修订增加主题恢复与黄宣纸纹理、WebDAV 全数据备份/恢复、节假日日期预览及写入回执、年度任务热力日历。详见 [0.1.3 发布记录](docs/RELEASE_0.1.3.md)。
