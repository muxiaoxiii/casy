# 本地 OCR 与来源定位验收

日期：2026-09-07。本报告记录这一批的实现及已验证范围，不判定整个 Casy 已完成交付。

## 选型事实

- [RapidOCR](https://github.com/RapidAI/RapidOCR) 使用可替换的 Paddle ONNX 模型，代码 Apache-2.0；主仓库是 Python 实现，其他语言组件独立维护。
- RapidOCR 的默认配置当日为 PP-OCRv6 small；切换到该项目本身不代表采用了非 mobile 高精度模型。
- 本批使用官方 PP-OCRv6 medium ONNX，沿用现有 Rust oar-ocr 运行库，暂停 oar-ocr-vl / Candle 依赖。
- [检测权重](https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/tree/61323801669c338b7891481ec7bac61ce31b576a)：62,032,837 字节。
- [识别权重](https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/tree/50c7eacafc52fa7bcf4194e8cd08e46f8558504b)：76,554,979 字节。
- 权重和配置合计 138,739,282 字节，下载后校验固定哈希。模型字典使用官方 YAML 字段，不借用旧 v5 字典。
- [Paddle 官方模型说明](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv6/PP-OCRv6.en.md) 将 medium 定位为 server，支持中英日和拉丁语系。本批实际验证中英德法日。
- [Paddle 版面方案](https://www.paddleocr.ai/main/en/version3.x/module_usage/layout_detection.html) 与 PP-Structure 提供独立版面、表格等模块。PP-DocLayout-M 的现成 ONNX 约 23.5 MB，尚未接入本批，不额外跑模型判断页面是否复杂。
- 完整安装包还要计入主程序、渲染器、字体及各平台依赖，不能据权重体积宣称安装包已达到 200 MB。

## 实现

- 每页视觉 OCR，不再凭文本层存在就跳过。派生 PDF 从可见像素重建，不保留旧的伪造文本；原件哈希不变。
- Markdown 和区域文字使用同一识别结果，来源映射保存原件、Markdown、连续文字的哈希和 UTF-8 字节范围。
- 主程序验证映射与页面、备份一致性，拒绝坐标、页码、哈希和产物路径不一致的结果。
- 页级 FTS 之外，检索选定文件最新完成版本的连续区域文字与相邻页连接。中日换行、拉丁文行末连字符可以跨页匹配，显示原始文本而非修改后的文本。
- 跨页结果返回多个页码和区域。仅在有实际 OCR 区域时提供坐标，文字型文档仍按段定位。
- 软件内提供原 PDF / Markdown 对照、文字区域联动、翻页、缩放和跨页命中切换。原件变更时拒绝旧坐标。
- 旧 VL / 原生文本引擎的已完成结果，在用户再次提交处理时不再阻止新视觉识别任务。

## 验证证据

- 主程序 Rust 全量测试通过：197 项 lib 测试及集成测试；需要私有快照的测试默认忽略。
- 前端 168 项测试通过，生产构建通过。
- 引擎非模型单测覆盖伪造隐藏金额清除、渲染像素误差、原件哈希、页码、图片转换和多语言来源映射。
- 真实模型中文双页扫描件检查第三人、金额、日期、Markdown 与可搜索 PDF，通过。
- 真实模型混排样本检查中文、英语、德语变音字母、法语重音、日语横排和竖排，以及隐藏伪造金额不进入输出，通过。
- 首次混排样本使用 Songti 缺少部分日文字形，原 PDF 本身缺字；查看渲染图后改用覆盖目标字形的 Arial Unicode，重新完整验证。没有通过删减日语断言掩盖失败。
- 优化构建对双页混排样本处理 3.767 秒，最大 RSS 916,160,512 字节，峰值 footprint 945,784,128 字节。此为单一合成样本，不能推算所有材料的吞吐量。
- Playwright 经真实 Rust IPC 和独立加密数据库完成检索、两页定位、原图像素检查、缩放、文字高亮、Markdown、1440/390px 窗口及源文件变化错误状态。最终 19 次原生命令，数据库完整性 ok，外键错误 0，页面错误 0。
- Word/Markdown 回归经真实 Rust 后端完成正文提取、段落对照、检索、知识库沉淀：43 次原生命令，页面错误 0。合成旧 DOC 的解析不兼容仍明确提示另存 DOCX，没有伪报成功。

私有本地产物：

- 模型与多语言测试：`/Users/only/Documents/Casy-Local-Test/ocr-medium-2026-09-07/`
- 进程性能：`/Users/only/Documents/Casy-Local-Test/ocr-benchmark-70W4Fc/`
- PDF 对照 UI：`/Users/only/Documents/Casy-Local-Test/source-view-profile-TVBW2Q/`
- Word/Markdown 回归：`/Users/only/Documents/Casy-Local-Test/text-document-profile-I75M8T/`

## 未完成范围

- PP-DocLayout / PP-Structure、复杂双栏阅读顺序、表格结构、跨页表格拼接、公式和印章精度没有作为已完成能力。
- 跨页检索当前连接相邻页；超过两页的单次匹配、重复页眉页脚消除、长文档预计算分块索引尚未实现。
- Markdown 是与原件绑定的识别备份；还没有 OCR 校订、修订版本映射及写作编辑器证据引用回跳的完整产品流程。
- 视觉识别牺牲原生 PDF 的快捷抽取速度；派生件栅格化，原件的签名、附件、表单和矢量精度留在原件中。
- 模型安装、平台渲染器与可分发多语言字体尚未打成最终安装包；本轮软件内 UI 采用浏览器加真实 Rust IPC 验收，未完成打包 WebView 的逐平台验收。

本批没有访问或修改飞书表格，没有向外部 AI 发送文档，测试数据库和生成产物均位于隔离目录。
