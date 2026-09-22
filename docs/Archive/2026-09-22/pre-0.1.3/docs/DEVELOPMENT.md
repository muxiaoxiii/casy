# 开发、验证与交付

核对日期：2026-09-22。当前源码版本 0.1.2，最近完成验证的安装包是 0.1.1；按用户要求暂不继续修复或打包。

## 环境

- Node.js 24 与 npm；CI 使用 `npm ci`。仓库也有 pnpm 锁文件，本机 node_modules 来自 pnpm；可复现性需后续统一，避免随意交替安装。
- Rust manifest 要求至少 1.95。本机本次使用 rustc 1.96.0。
- macOS 需要 Xcode command line tools；完整运行时准备需要 Homebrew Poppler。
- Zvec 原生库由固定版本脚本准备；`.cargo/config.toml` 指向 `src-tauri/runtime/zvec`。
- Linux/Windows 有平台依赖，但当前没有已验证的完整 OCR 分发包。

```bash
npm ci
node scripts/prepare-zvec.mjs
npm run tauri -- dev
```

`npm run dev` 只是浏览器前端，部分操作走 mock，不会证明桌面命令、数据库或 OCR 可用。

## 隔离资料

debug 构建支持绝对路径 `CASY_TEST_DATA_DIR`；应用还支持显式 `--profile-dir <绝对路径>`。测试不要使用真实资料库。需要保留验收产物时给独立目录并记录路径，结束后再按需清理。

```bash
CASY_TEST_DATA_DIR=/private/tmp/casy-review-profile \
  cargo test --manifest-path src-tauri/Cargo.toml --locked
```

运行时可用 `CASY_DOC_ENGINE`、`CASY_PPOCR_MODEL_DIR`、`CASY_KOREAN_MODEL_DIR`、`CASY_PDFTOPPM`、`CASY_OCR_FONT` 覆盖具体工具/资源。完整包应在清除这些覆盖后仍可通过验证。

## 验证分层

```bash
npm run typecheck
npm run test:unit
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models
node --test scripts/binary-architecture.test.mjs scripts/license-policy.test.mjs scripts/runtime-components.test.mjs
```

`tests/e2e/` 包含 17 个本地脚本，通常需要桥接 example、前端服务、浏览器和隔离资料目录；它们没有被普通 `npm run test:unit` 自动运行。真实模型用例中带 `#[ignore]` 的项目也必须按各文件说明单独运行。

本次全量前端为 **269 通过、1 失败**，失败在 `quoteSources.test.ts`。不能把历史“全部通过”记录继续写为当前门禁状态。本次未重新执行全套 Rust、桌面 GUI、外部服务或跨平台验收。

## 完整包

```bash
npm run build:desktop -- --bundles dmg
node scripts/verify-bundle.mjs src-tauri/target/release/bundle/macos/Casy.app
```

`build:desktop` 使用 `tauri.full.conf.json`，先准备 runtime 再构建前端。`tauri.dmg-only.conf.json` 跳过 beforeBuild，只能在资源和 dist 已明确更新时使用；普通 `tauri build` 不自动包含完整 runtime 资源配置。

0.1.1 中默认 DMG 脚本失败，最后用 hdiutil 将同一份已签名应用及 Applications 链接制作为只读 DMG，再只读挂载验证。该流程尚未自动化；不要仅改文件名或复用旧 bundle 认定新版已生成。

发布完成标准：

1. 冻结源码和锁文件状态，保留版本与构建证据。
2. 完成相关测试与真实样本验收，记录失败/跳过，不以局部测试代替全套。
3. 准备模型/动态库/字体/许可，生成 manifest 并校验。
4. 构建主应用；检查 Info.plist、架构、签名和实际引擎能力。
5. 生成 DMG，校验镜像与 SHA-256，再挂载核验包内应用。
6. 更新项目状态与发布说明；源码版本和安装包版本一致后才宣布交付。

## CI 实际情况

[ci.yml](../.github/workflows/ci.yml) 的 frontend 跑类型、单测、构建；Rust 矩阵为 Ubuntu/macOS，Windows 已暂停。tag 打包仅 macOS。CI 还声明 Clippy/audit 和 bindings 漂移检查，但本次没有远端运行证据。

目前 tag 打包后未调用 `verify-bundle.mjs`，也没有对应 DMG 挂载验收与真实 OCR gate；应按审阅 R-08 补齐。公证与发行证书仍未完成。
