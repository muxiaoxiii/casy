# Casy 当前状态

核对日期：2026-09-22。基线为 `main` 当前工作区，HEAD `f92564b`，含大量既有未提交及未跟踪文件，不是该提交的纯净快照。用户要求暂停修复、打包，转为文档整理、代码审阅和计划。

## 源码与交付包

| 项目 | 当前工作区 0.1.2 | 已交付包 0.1.1 |
| --- | --- | --- |
| Markdown 图片外置 | 文件转换/知识笔记/编辑器三个出口已接入 | 已包含并验证 |
| 文本及提取正文 64 MiB 限制 | 已移除；大文件全链路验收未完成 | 仍存在 |
| OCR stdout 128 MiB 限制 | 改临时文件流式接收；仍解析完整页集合 | 仍存在 |
| 转换固定 3 分钟前端超时 | 已豁免转换命令，等待后台结果 | 仍存在；已观察到超时后文件最终生成 |
| OCR 无页数进展 15 分钟保护 | 保留 | 保留 |
| 安装包 | **未完成 0.1.2 DMG** | 包内验证完成 |

0.1.1 文件：`release/Casy-0.1.1-2026-09-22-OCR-fix-macOS-arm64.dmg`；910,964,894 字节；Apple Silicon / macOS 26+；ad-hoc 签名，未公证。

SHA-256：`f891fded35ec21aafbda54036ebd5cb2f862c0d959a966384a49f7ff31920ff3`。

0.1.2 版本字段、前端构建和运行时准备已更新，但旧 `target/release/bundle/macos/Casy.app` 不能代表新的应用交付。

## 验证记录

| 验证 | 结果与范围 |
| --- | --- |
| 0.1.1 commands 后端回归 | 101 通过，1 个真实引擎用例另跑；[原记录](docs/Archive/2026-09-22/audits/ocr-markdown-export-fix-2026-09-22.md) |
| 0.1.1 真实 OCR 导出 | 1 页/1 图表，34,783 → 1,396 字节，图片字节一致；[验证数据](outputs/ocr-export-2026-09-22/verification.json) |
| 0.1.1 包内核验 | 版本、签名、2155 个资源文件、模型与向量检索通过，不适用于未交付的 0.1.2 |
| 超时/转换/处理中心前端定向测试 | 8 通过，模拟长等待不会提前失败 |
| 文档相关 Rust 定向测试 | 20 通过；`cargo test --lib document_`；启动时编译版本仍为 0.1.1，后续版本更新及缓冲优化没有被这次运行覆盖 |
| OCR 引擎模型特性测试 | 19 通过，5 忽略；不是实际大 PDF 验收 |
| 0.1.2 前端生产构建 | 通过，有大 chunk 提示 |
| 本次全量前端测试 | **269 通过、1 失败，共 56 个文件**；[日志](docs/Archive/2026-09-22/review-evidence/frontend-tests.log) |
| 本次运行时脚本测试 | 5 通过；[日志](docs/Archive/2026-09-22/review-evidence/script-tests.log) |

失败项：`quoteSources.test.ts` 报 ProseMirror Fragment 不兼容。具体依赖/模块加载原因未定位，不能把产品运行时也判定为相同故障。

尚未完成：超过 64 MiB 且含真实 PNG 的 Markdown 完整转换；长 PDF 耗时/内存/取消恢复验收；当前最终工作区全套 Rust 回归；0.1.2 主程序打包与包内核验。大样本已生成在 `outputs/large-conversion-2026-09-22/`，仅是输入样本。新增 `accepts_text_source_and_extracted_markdown_larger_than_64_mib` 用例未被此前 `document_` 过滤器选中，不能算已经通过。

## 当前基线

- schema 41；程序事件、任务/日历、来源映射、白板事实修订均有实现。
- PP-OCRv6 medium + 韩文 PP-OCRv5 mobile + Paddle 版面，E5-base INT8 / Zvec 检索。
- 统一编辑器已接入；引用层级回归未全绿。
- 截图、自动剪贴板监听、语音转写、部分批量/重试以及 WebDAV 状态仍有占位。
- CI Rust 矩阵为 Linux/macOS，发行仅 macOS，不能沿用“三平台已验证”说法。

下一步见[代码审阅](docs/CODE_REVIEW_2026-09-22.md)与[更新计划](docs/UPDATE_PLAN.md)。
