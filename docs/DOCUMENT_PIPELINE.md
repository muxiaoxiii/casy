# 文档处理与转换

核对日期：2026-10-08。适用于 0.1.3 生产验证版；最新状态见 [项目状态](../Casy-STATUS.md)。

## 输入与处理

| 输入 | 当前处理方式 | 独立转换输出 |
| --- | --- | --- |
| PDF | 逐页渲染、OCR、韩文候选识别、版面/表格分析，利用原生文字辅助校核 | Markdown、可搜索 PDF、两者 |
| PNG/JPEG/WebP/BMP/TIFF 等单帧图片 | 图像识别；仅 Markdown 时可跳过生成 PDF | Markdown、可搜索 PDF、两者 |
| MD/Markdown/TXT | UTF-8 文本读取与分段 | Markdown |
| DOC | **anydoc 原生解析 `.doc`（主路径）**；仅当 anydoc 拒绝该文件时，再用侧车 `bin/doc2x`（b2xtranslator 派生）转成 DOCX 后重试。两者都失败才提示另存 DOCX | Markdown |
| DOCX/DOCM/RTF/ODT | anydoc 转 Markdown，再分段 | Markdown |

说明：`.doc` 不是“不支持”。anydoc（已锁定 **0.2.4**）自带 MS-DOC 解析；“请另存为 DOCX”只表示**该文件**解析失败。侧车是兜底，不是主路径。方案与钉版本校验见 [DOC_IMPORT_PLAN](DOC_IMPORT_PLAN.md)。`scripts/prepare-doc2x.mjs` 写入 `runtime/bin/doc2x` 与 BSD-3 声明；`CASY_DOC2X_PATH` / `CASY_DOC2X_TIMEOUT_MS` 可覆盖路径与超时（默认 120s）。浮动图/艺术字/复杂矢量仍可能丢失，失败不发布半成品正文。

文字或混合批次只允许 Markdown，后端也在产出文件前验证格式能力。文字排版 PDF 请用文书编辑器导出。多帧图片需先转换 PDF，避免仅取一帧。

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
| `source.md` | 引擎 Markdown；新任务裁图已外置为 `assets/<sha>.png` 引用 + manifest，存量文档可能仍含 base64（可用存储升级命令转换） |
| `source.map.json` | 原件与正文/Markdown 哈希、UTF-8 字节范围和来源坐标 |
| `source.searchable.pdf` | 可选派生件，栅格页面加新识别文字层 |
| `progress.json` | 阶段、页数、耗时及估计剩余时间 |

派生 PDF 不迁移原 PDF 的签名、附件、表单或完整矢量精度。原件保留不改写。来源校订沿用绑定关系，不能用原始文件已经变化的旧结果冒充当前原件。

## Markdown 图片

0.1.1 起，独立转换、知识笔记导出和编辑器 Markdown 导出共用 [markdown_export.rs](../src-tauri/src/commands/markdown_export.rs)：将 base64 图片解码为独立文件，并写入相对引用。每次导出使用独立 `casy-images-…` 目录，内部按内容哈希去重。Markdown 成功落盘才保留该目录，失败会清理临时图片。

移动/分享 Markdown 必须带上图片文件夹。内部 OCR 缓存仍保留自包含表示，历史缓存不会被这个导出修复自动重写。2026-10-04 起引擎把裁图外置为 `assets/<sha>.png` + manifest（v2），卷宗 OCR、独立转换与知识快照三条路径一致；取图经 `read_document_asset` 校验存在性、大小与 SHA-256；存量 base64 文档可用存储升级/回退命令转换，`document-artifacts/` 尚无回收策略（见[审计](CODE_AUDIT_2026-10-08.md)）。

0.1.3 跨目录转换还会复制原文档目录内的相对图片（Markdown 内联、引用式和带引号的 HTML src），按字节哈希去重。缺图、越界路径或符号链接逃逸会拒绝导出，远程 URL 不下载。Markdown 与 PDF 同批输出全部准备好后才发布，失败回滚本次新建文件，已有同名输出不会覆盖。HTML 无引号 src 尚不迁移。

## 大文件与资源边界

| 边界 | 0.1.3 实现 |
| --- | --- |
| 文字源文件/提取 Markdown | 移除各 64 MiB 的固定限制；仍需足够内存 |
| OCR 子进程 stdout | 分块写临时文件，缓冲反序列化 |
| Markdown / 来源映射 | Markdown 增量写入与哈希；来源文本/坐标仍驻内存 |
| PDF 输出复制 | 流式复制；不再整份读入 Vec |
| 页 IR 二次校验 | 流式读盘逐页比较；原始响应 pages 仍是完整集合 |
| 图片解码 | 宽/高各最多 16000，分配上限 256 MiB |
| OCR 校订输入 | 128 MiB |
| 工作区全文读取 | 16 MiB，超限建议分页对照 |
| 统一编辑器导出 | JSON 和 Markdown 各 40 MiB |
| 旧 `.doc` 侧车 | 单次转换默认 120s 超时；产物必须 ZIP + `word/document.xml` |

仍不是端到端有界内存：可搜索 PDF 由 lopdf 整份载入后重写（行内性质）；文字解析器与图片外置入口仍会读取完整 Markdown。单页 PDF 也可能因巨幅图片、复杂矢量或大量嵌入对象占用大量资源。移除文件字节硬限制不等于无限容量保证。工作区正文的 `regions_json` 另有 32 MiB 总预算，超预算页只保留纯文本（续篇检测降级）。

识别与 finalize 均已逐页流式：引擎识别一页即把该页写入页 IR（内存中不累积页面集合），可搜索 PDF 重建、来源映射与 Markdown 备份从页 IR 流式读取；主程序侧校验与落库同样是“内存中只有一页”。

引擎结果只回传路径与页数（`pageCount`），页面集合唯一事实源是落盘页 IR；父进程 `stream_disk_pages` 按需多轮流式遍历。

## 超时、失败与恢复

旧版前端 3 分钟 `IPC_TIMEOUT` 不会取消后台任务，造成“显示失败但文件稍后生成”。0.1.3 的转换、备份、导入导出及长同步写操作等待后端终态，普通读取仍保留前端超时。

后端保留 15 分钟无页数或阶段变化的停滞保护，页数增加或阶段切换刷新计时；不是整个文件只能处理 15 分钟。引擎在 `finalizing` 阶段逐页写相位心跳（`finalizing:<页码>`），主路径不再被误杀；单页渲染超过 15 分钟、或收尾段（外置 + 全量页 IR + 来源映射写入）长时间零进度仍会终止。文字文档路径不走引擎调度，没有停滞保护与取消检查，卡死的解析会占住阻塞线程。

转换具有持久化 job ID；处理中心或转换窗口可取消当前任务并停止后续文件。OCR 子进程轮询取消标记，输出提交阶段不再接受取消；文字解析需等当前阻塞解析返回后确认取消。取消或失败不发布新输出。

**R-06 长任务恢复（2026-10-08）**：任务完成时把完整产物清单与结果摘要（`document_job_results`：页 IR/Markdown/可搜索 PDF/来源映射路径、页数、引擎、Markdown 哈希）与任务状态同事务落库；`get_document_job_result` 可在窗口断开或重启后查询。重试失败/已取消任务时，若同一文件同一内容哈希已有完成结果则直接复用（`reused`），不重跑；显式重跑已完成任务仍会重新识别。启动时对“产物已写全但被标记中断”的任务尝试用落盘产物直接完成（校验源文件哈希 + 流式读完页 IR），不重跑 OCR。半成品产物目录支持断点续算：worker claim 时若同一任务已有部分页 IR，则从第 N+1 页继续识别（不重跑已完成页）；重试失败任务时其未完成的部分产物目录会移交给新任务作为续算起点；页数不一致或页 IR 截断会直接报错而非静默重来。

## 验收口径

短图表样本只证明图片外置链路；129 MiB 接收测试只证明传输不截断。它们都不替代真实长 PDF、超大 Markdown、内存和中断恢复验收。已运行与未运行测试以 [项目状态](../Casy-STATUS.md) 为准。
