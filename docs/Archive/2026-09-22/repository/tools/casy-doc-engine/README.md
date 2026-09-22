# casy-doc-engine

Casy 的独立本地 Rust 文档引擎。默认使用非 mobile 的 PP-OCRv6 medium 检测、识别 ONNX 模型。
VL 路线暂停：不再依赖 Candle / oar-ocr-vl，不再默认下载 PaddleOCR-VL 或 OvisOCR2。
既有模型目录不会删除。推理没有 Python 服务，也不上传案件资料。

## 构建和安装

```sh
cargo build --manifest-path tools/casy-doc-engine/Cargo.toml --release --features models
node tools/casy-doc-engine/install-models.mjs /absolute/model-directory
# 只查看下载预算，不访问网络：
node tools/casy-doc-engine/install-models.mjs /absolute/model-directory --dry-run
```

模型固定到 Paddle 官方仓库版本，逐文件校验哈希和长度。主识别器使用 PP-OCRv6 medium；
韩文候选识别使用配套的 korean PP-OCRv5 mobile 权重和 11,504 个韩文字字典，不能把韩语字典
直接拼入主模型，否则模型输出类别与字典索引会错位。
配置中的字符字典通过 YAML 解析器读取，避免把语言字典与识别权重混用。
默认模型覆盖中英德法日等语言；本机合成测试已检查混排重音和日语竖排。
单个测试不能代表所有字体、低清扫描件和复杂版面的准确率。

从相同环境启动 Casy：

- `CASY_DOC_ENGINE`：构建出的可执行文件。
- `CASY_PPOCR_MODEL_DIR`：安装目录下的 `ppocrv6-medium`。
- `CASY_KOREAN_MODEL_DIR`：安装目录下的 `korean-ppocrv5-mobile`。
- `CASY_OCR_FONT`：覆盖所需语言的 TTF/OTF/TTC。测试使用 macOS Arial Unicode；
  该系统字体不随软件分发，产品打包应配套有授权的多语言字体。
- PDF 渲染依赖 Poppler 的 `pdftoppm`。模型安装脚本需要 Node.js，实际推理不需要 Node.js。

其他兼容 ONNX 模型可以提供 `det.onnx`、`rec.onnx` 和 `dict.txt`，或官方 `rec.yml`。
新模型必须单独核对预处理、字典、精度、语言及许可，文件格式兼容不等于质量验证通过。
RapidOCR 的 ONNX 部署和可切换模型方案是选型参考；当前执行器仍使用 Rust oar-ocr，
没有把 RapidOCR Python 环境装入 Casy。PP-DocLayout / PP-Structure 的版面和表格管线尚未接入。

## 文档产物

PDF 及单帧图片都以可见渲染结果为准，即使原 PDF 有文字层也会视觉识别。
派生可搜索 PDF 从页面像素重建，并只加入新识别文字，防止伪造隐藏文字继续残留。
原 PDF 保持不变；派生件是 2400px 上限的栅格副本，原件的矢量精度、表单、附件和签名不迁移到派生件。
当前没有未经验证的“有文字即跳过 OCR”捷径，也不额外运行模型来判断是否属于复杂版面。

每项任务输出：

- `source.searchable.pdf`：派生的可搜索 PDF。
- `source.document.json`：逐页文字、区域、尺寸、置信度。
- `source.md`：同一识别结果的 Markdown 备份，保留页标记。
- `source.map.json`：原件/Markdown/连续文字哈希、UTF-8 字节范围、页码与区域坐标。
  文本不一致时只保留页级映射，不推测精确坐标。

主程序校验上述映射、源文件哈希和落盘结果后入库。卷宗检索可跨相邻页和行末连字符匹配，
保留原文及两页的来源区域。对照阅读通过文件 ID、任务 ID、页码读取原页面，
支持文字/原页联动、翻页、缩放。原文件变更后拒绝旧位置。
Word、Markdown 等文字文档保留段落定位，不虚构 PDF 页码和坐标。

## 本地验证

```sh
cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --features models
# 设置上述模型/字体变量及独立产物目录：
CASY_OCR_QA_DIR=/absolute/private-qa cargo test \
  --manifest-path tools/casy-doc-engine/Cargo.toml --features models \
  real_multilingual_and_forged_text_pdf -- --ignored --nocapture
```

详见 `docs/audits/casy-ocr-source-binding-2026-09-07.md`。
仍需验证密集双栏、复杂表格、跨页表格及重复页眉页脚，并将模型安装与字体选择产品化。
模型约 139 MB；完整安装包是否达到 200 MB 尚未实测。
