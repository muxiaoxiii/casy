# casy-doc-engine

Casy 的独立本地文档引擎。它把 PP-OCRv5 坐标 OCR、OvisOCR2 页面转 Markdown 和 harumi 不可见文字层组合成一个进程边界，原 PDF 始终只读。

模型版构建：

```bash
cargo build --release --features models
# Apple Silicon: cargo build --release --features metal
```

运行前配置：

- `CASY_DOC_ENGINE`：该二进制路径（由主程序读取）
- `CASY_PPOCR_MODEL_DIR`：包含 `det.onnx`、`rec.onnx`、`dict.txt`
- `CASY_OVISOCR2_MODEL_DIR`：OvisOCR2 Hugging Face 本地模型目录
- `CASY_OCR_FONT`：覆盖目标语言字形的 TTF/OTF，例如 Noto Sans CJK
- `CASY_OCR_DEVICE`：`cpu`、`metal`、`cuda` 或 `cuda:N`

`probe` 只做依赖探测；`process` 从 stdin 接受 JSON 请求、向 stdout 返回 JSON 结果，诊断写 stderr。
