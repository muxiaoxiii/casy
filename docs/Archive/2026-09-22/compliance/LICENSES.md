# 第三方软件许可

完整桌面构建通过 `scripts/prepare-runtime.mjs` 生成资源内的 `runtime/licenses/`。

- `packages/index.json`：Cargo 解析结果与已安装前端生产依赖的版本、许可、源代码包路径。每个包递归保留许可文件及内部第三方声明。明确列入宽松许可白名单的包，其完整源码归档保存在构建缓存，不重复装入应用；清单记录版本、上游来源、归档 SHA-256 和交付方式。Copyleft（例如 GPL／LGPL／MPL）、自定义及未知许可、未附独立许可声明的依赖仍随包提供完整已安装源码；原生组件对应源码的交付不变。
- `native-components.json`：PDF 渲染器及其动态库的实际安装版本、源码 URL、校验值、Homebrew 构建配方。各组件目录包含匹配二进制版本的源码与声明。
- `Noto-OFL.txt`：随包多语言字体的 SIL Open Font License。
- 编辑排版新增 Typst、typst-pdf、typst-svg、typst-layout、typst-assets、MiTeX（Apache-2.0），以及 Mermaid 的 Rust 渲染器、前端 Mermaid 和 KaTeX（MIT）。实际版本和传递依赖以两个锁文件及生成的 `packages/index.json` 为准。`typst-assets/NOTICE` 内含其嵌入字体的独立授权，随包保留，不能仅以 crate 的 Apache-2.0 概括字体许可。
- 模型安装脚本记录 Paddle OCR 与版面模型的固定版本及校验值，模型采用 Apache-2.0 许可。

PDF 渲染器 Poppler 包含 GPL 许可组件，以独立可执行程序调用，并随包提供对应源码和构建配方。不能将整个运行时描述为“无 GPL 依赖”。

`runtime/manifest.json` 为随包资源提供大小及 SHA-256，可核验交付副本。其他平台必须提供匹配平台的完整 PDF 渲染器分发目录；尚未验收的平台不视为已交付。
