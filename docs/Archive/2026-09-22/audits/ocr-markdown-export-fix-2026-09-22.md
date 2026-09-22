# OCR Markdown 图片导出修复（2026-09-22）

## 原因和范围

用户已删除原先安装的应用，无法核对其版本。当前代码仍由 `tools/casy-doc-engine/src/visual.rs` 将版面检测出的图片、图表、公式、印章等裁图嵌入为 `data:image/png;base64,...`；文件转换此前直接复制引擎 Markdown，因此当前源码同样存在问题，不能仅归因为安装旧版。

0.1.1 在用户导出边界将内嵌图片解码保存为文件，Markdown 改用相对路径。三个入口共用 `commands/markdown_export.rs`：独立文件转换、知识笔记 Markdown 导出、统一编辑器 Markdown 导出。

- 导出图片存放在 Markdown 同目录的独立 `casy-images-…` 文件夹。
- 图片以 SHA-256 命名，相同内容在一次导出中只保存一份；不同任务使用独立文件夹，避免冲突。
- 保留图注、OCR 文字和结构；图片字节不重新压缩。
- Markdown 成功落盘后才保留图片目录；失败时临时图片自动清理。
- 已存在的 Markdown 不会被文件转换覆盖；可重新转换旧版 base64 Markdown。
- 文件转换界面提示分享/移动 Markdown 时携带图片目录。
- OCR 内部缓存和校订数据继续使用自包含表示，避免破坏来源映射与应用内对照阅读。此修复针对用户导出的 Markdown 文件，不改写历史缓存。

## 版本

主应用版本从 0.1.0 升至 0.1.1，同步 package.json、package-lock.json、Cargo.toml、Cargo.lock 与 tauri.conf.json。使用当前工作区构建，保留已有修改。

## 验证

- 前端类型检查和生产构建通过。
- 转换进度/处理中心前端测试：5 项通过。
- 后端 commands 测试：101 项通过，1 项真实引擎测试单独运行。
- 覆盖 HTML/Markdown 图片、中文和特殊字符目录、整体移动后相对引用、重复转换不覆盖、图片字节一致、异常清理、知识笔记及编辑器入口。
- 打包前运行 runtime:prepare：OCR/韩文/版面/嵌入模型哈希校验通过，运行时 2155 个文件。
- 合成柱状图扫描图真实执行 OCR，检测到 1 个 chart 区域，可复现原始内嵌 base64 输出。
- 真实引擎 + 完整文件转换验收通过：1 页 / 1 个图表；Markdown 从 34,783 字节降至 1,396 字节；导出不含 base64，PNG 与引擎裁图逐字节一致，原扫描图哈希保持不变。验收记录：`outputs/ocr-export-2026-09-22/verification.json`。
- 主程序 release 编译通过。Tauri 默认 DMG 脚本失败，改用 macOS hdiutil 将同一份签名应用与 Applications 快捷方式制作成只读压缩 DMG。
- 应用与只读挂载 DMG 中的应用均通过 verify-bundle：arm64、ad-hoc 签名、2155 个运行时文件哈希、OCR/韩文/版面模型、768 维本地嵌入、Zvec 向量检索通过。
- DMG 内 Info.plist 版本 0.1.1，主程序与已构建程序 SHA-256 完全一致，镜像 CRC 校验通过。

## 交付

`release/Casy-0.1.1-2026-09-22-OCR-fix-macOS-arm64.dmg`

- Apple Silicon / macOS 26.0 及以上；完整本地运行时。
- ad-hoc 签名，沿用现有分发配置，未作 Apple 公证。
- SHA-256：`f891fded35ec21aafbda54036ebd5cb2f862c0d959a966384a49f7ff31920ff3`
- 打开 DMG，将 Casy.app 拖入 Applications。
- 分享或移动导出的 Markdown 时，一并携带旁边的 casy-images-… 文件夹。
