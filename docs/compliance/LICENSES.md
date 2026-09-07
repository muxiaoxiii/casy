# 第三方软件许可

完整桌面构建通过 `scripts/prepare-runtime.mjs` 生成资源内的 `runtime/licenses/`。

- `packages/index.json`：Cargo 解析结果与已安装前端生产依赖的版本、许可、源代码包路径。每个包保留许可文件与完整已安装源码，包括内部的第三方声明。
- `native-components.json`：PDF 渲染器及其动态库的实际安装版本、源码 URL、校验值、Homebrew 构建配方。各组件目录包含匹配二进制版本的源码与声明。
- `Noto-OFL.txt`：随包多语言字体的 SIL Open Font License。
- 模型安装脚本记录 Paddle OCR 与版面模型的固定版本及校验值，模型采用 Apache-2.0 许可。

PDF 渲染器 Poppler 包含 GPL 许可组件，以独立可执行程序调用，并随包提供对应源码和构建配方。不能将整个运行时描述为“无 GPL 依赖”。

`runtime/manifest.json` 为随包资源提供大小及 SHA-256，可核验交付副本。其他平台必须提供匹配平台的完整 PDF 渲染器分发目录；尚未验收的平台不视为已交付。
