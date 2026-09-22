# 桌面平台兼容审计（2026-09-08）

用户要求 macOS ARM/Intel 与 Windows 32 位 x86/64 位 x64。以下区分交付事实与目标，不能把单个 ARM 安装包称为通用包。

| 目标 | 当前结论 | 阻塞与必要验收 |
|---|---|---|
| macOS ARM64 | 当前完整包的本机验证目标 | App、OCR、PDF renderer、Zvec、嵌入模型需一起通过；当前构建最低系统版本为 macOS 26 |
| macOS Intel x86_64 | 尚不支持交付 | Zvec 0.7.0 官方 SDK 无 macOS Intel 资产；当前 ort-sys rc.13 预编译清单无 x86_64-apple-darwin；需要构建并固定原生依赖且在 Intel 真机验收 |
| Windows x64 | 尚未完成完整包验证 | Zvec 与 ORT 有 x64 路径，但现有 CI Windows 测试因 DLL 入口错误被移出；PDF renderer 可再分发目录未配置；不能跳过这些错误发布 |
| Windows x86 32 位 | 现有完整推理依赖链不支持直接打包 | 官方 Zvec SDK 与当前 ORT 分发清单均无 i686；不能把 64 位 DLL 放进 32 位程序。需原生依赖移植或替代后端，并验证 32 位地址空间内的推理内存 |

证据：

- [Zvec v0.7.0 官方 Release](https://github.com/alibaba/zvec/releases/tag/v0.7.0)：本日 API 返回 osx-arm64、windows-amd64、Linux 多架构、Android/iOS；无 osx-amd64 或 Windows i686。
- 当前 `ort-sys-2.0.0-rc.13/build/download/dist.tsv` 包含 aarch64-apple-darwin、x86_64-pc-windows-msvc、aarch64-pc-windows-msvc 等，未包含 Intel macOS / Windows 32 位。缺少预编译资产不是数学上的不可移植，但当前工程未完成这些移植。
- `scripts/prepare-zvec.mjs` 的固定哈希映射、`scripts/prepare-runtime.mjs` 的平台 renderer 分支、`.github/workflows/ci.yml` 的 Windows 暂停说明，以及 `src-tauri/tauri.full.conf.json` 的 minimumSystemVersion。

## 实施准备与发布门禁

已新增 `scripts/binary-architecture.mjs`：从 Mach-O／Universal／Windows PE 文件头读取真实架构，通用测试覆盖 ARM64、Intel x64 和 Windows ia32。macOS 完整包校验已接入主程序、OCR sidecar、renderer 及全部 dylib 的同架构验证，避免打包时混入其他架构。这个检查是跨平台准备，不代表缺失平台已经构建成功。

1. 四个目标各自拥有独立 runtime manifest：主程序、OCR sidecar、renderer、向量库及依赖库必须同架构；模型 ONNX 可共享，原生二进制不可共享。
2. macOS 两个独立 DMG 是优先可验证方案。Universal 主程序不自动解决 runtime dylib 与 sidecar 的双架构问题；需要所有 Mach-O 都同时含两种架构后才可改成 Universal。
3. Windows x64 先恢复干净 runner 的 DLL 加载与 SQLCipher 测试，再固定 renderer 发行包／源码与许可证、配置 DLL 搜索路径，最后制作 NSIS 安装包并在无开发环境的 Windows 主机验证。
4. Intel macOS / Windows 32 位需要先解决上述推理与向量库依赖，不能用禁用 OCR 或静默跳过知识库来假装实现完整支持。
5. 每个平台必须执行：全新安装、旧库升级、中文路径、凭据读写、GDS/OES 类图文表格、可搜索 PDF、向量检索、卸载／重装后数据保留、单实例和架构检查。

本轮没有 Windows 或 Intel Mac 的可执行环境与相应完整依赖，也没有发布未经验证的其他平台安装包。四平台支持尚未完成；本次不能对此作出已支持的承诺。
