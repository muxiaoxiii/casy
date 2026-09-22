# 文档处理与转换

核对日期：2026-09-22。以下区分工作区 0.1.2 与已交付 0.1.1；最新状态见 [项目状态](../Casy-STATUS.md)。

## 输入与处理

| 输入 | 当前处理方式 | 独立转换输出 |
| --- | --- | --- |
| PDF | 逐页渲染、OCR、韩文候选识别、版面/表格分析，利用原生文字辅助校核 | Markdown、可搜索 PDF、两者 |
| PNG/JPEG/WebP/BMP/TIFF 等单帧图片 | 图像识别；仅 Markdown 时可跳过生成 PDF | Markdown、可搜索 PDF、两者 |
| MD/Markdown/TXT | UTF-8 文本读取与分段 | Markdown |
| DOC/DOCX/DOCM/RTF/ODT | anydoc 转 Markdown，再分段 | Markdown；旧 DOC 解析失败需另存 DOCX |

界面目前对文字文档也允许选“双层 PDF/两者”，但文字解析器不生成 searchable PDF，会导致失败或部分产物，详见审阅 R-04。文书编辑器的 Typst PDF 导出是另一条能力，不能与独立转换混淆。多帧图片需先转换 PDF，避免仅取一帧。

## OCR 管线与产物

入口：[conversion.rs](../src-tauri/src/commands/conversion.rs)、[document_pipeline.rs](../src-tauri/src/document_pipeline.rs)、[引擎 main.rs](../tools/casy-doc-engine/src/main.rs)。

1. 校验路径、计算源文件 SHA-256。
2. 每页渲染/识别，模型默认为 PP-OCRv6 medium，韩文候选 PP-OCRv5 mobile，版面 PP-DocLayout Plus-L。
3. 表格尝试结构恢复；不能可靠结构化的表格、图表、公式、印章保留原图及辅助识别文字。
4. 输出页 IR、Markdown、来源映射；按需从页面栅格生成可搜索 PDF。
5. 主程序校验源哈希、产物目录、页内容、来源映射及 PDF 文件头，再持久化或导出。

| 产物 | 内容 |
| --- | --- |
| `source.document.json` | 页序、尺寸、文本、区域、置信度、版面/耗时 |
| `source.md` | 引擎 Markdown，内部裁图仍可包含 base64 |
| `source.map.json` | 原件与正文/Markdown 哈希、UTF-8 字节范围和来源坐标 |
| `source.searchable.pdf` | 可选派生件，栅格页面加新识别文字层 |
| `progress.json` | 阶段、页数、耗时及估计剩余时间 |

派生 PDF 不迁移原 PDF 的签名、附件、表单或完整矢量精度。原件保留不改写。来源校订沿用绑定关系，不能用原始文件已经变化的旧结果冒充当前原件。

## Markdown 图片

0.1.1 起，独立转换、知识笔记导出和编辑器 Markdown 导出共用 [markdown_export.rs](../src-tauri/src/commands/markdown_export.rs)：将 base64 图片解码为独立文件，并写入相对引用。每次导出使用独立 `casy-images-…` 目录，内部按内容哈希去重。Markdown 成功落盘才保留该目录，失败会清理临时图片。

移动/分享 Markdown 必须带上图片文件夹。内部 OCR 缓存仍保留自包含表示，历史缓存不会被这个导出修复自动重写。

已有相对图片引用的 Markdown 再转换到其他目录，目前没有复制对应图片，存在断图问题（R-03）；不要把它称为通用附件迁移方案。

## 大文件与资源边界

| 边界 | 0.1.1 包 | 当前 0.1.2 工作区 |
| --- | --- | --- |
| 文字类源文件/提取 Markdown | 各 64 MiB 上限 | 已取消这两项固定限制 |
| OCR 子进程 stdout | 一次收集，128 MiB 上限 | 分块写临时文件，再从文件解析 |
| JSON 序列化 | 部分整块构建字符串/字节 | 引擎结果与部分产物改用缓冲 writer |
| PDF 源文件 | 无显式 64 MiB 限制 | 同前 |
| OCR 停滞 | 无页数增长 15 分钟终止 | 保留 |
| 图片解码 | 宽/高各最多 16000，分配上限 256 MiB | 保留 |
| OCR 校订输入 | 128 MiB | 保留 |
| 工作区全文读取 | 16 MiB，超限建议分页对照 | 保留 |
| 统一编辑器导出 | JSON 和 Markdown 各 40 MiB | 保留 |

**目前不是全链路有界内存处理。** 引擎仍累积 `Vec<Page>`，来源映射构建完整 Markdown，主程序仍物化页集合，文字解析器整文件读取，导出 PDF 也整文件读取。取消大小上限不意味着任意大小都能稳定完成；下一步必须测实际峰值内存、磁盘、恢复与失败行为，而不能只扩大常量（R-02）。

## 超时、失败与恢复

0.1.1 的 `IPC_TIMEOUT` 是前端等待 3 分钟后拒绝 promise，**不会取消后台转换**；因此可能界面失败、文件随后生成，甚至开始下一项。用户已实际观察到文件生成。0.1.2 已针对转换取消这个计时器，但尚未重新交付安装包。

`DOC_ENGINE_TIMEOUT` 是后端无页数增长超过 15 分钟，不等于整个文件只能运行 15 分钟。当前阶段变化和输出字节增加不会刷新该计时，超长单页/收尾需进一步区分慢处理与卡死。

独立转换可停止后续文件，当前活动任务没有完整取消/断点续跑契约。应用重启时不可恢复的 processing activity 会被标记中断。计划统一 job ID、状态查询、取消、恢复和产物清单（计划阶段一）。

## 验收口径

短图表样本只证明图片外置链路；129 MiB 接收测试只证明传输不截断。它们都不替代真实长 PDF、超大 Markdown、内存和中断恢复验收。已运行与未运行测试以 [项目状态](../Casy-STATUS.md) 为准。
