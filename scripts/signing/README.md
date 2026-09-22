# 签名、公证与更新交付状态

核对日期：2026-09-22。本目录目前只有本说明，没有旧文档提及的 `sign-win.ps1`。不能将历史准备计划描述为已就绪的签名流水线。

## 当前状态

- 0.1.3 生产验证包为 macOS arm64、最低 macOS 26，ad-hoc 签名，未公证。
- [tauri.full.conf.json](../../src-tauri/tauri.full.conf.json) 使用 `signingIdentity: "-"`，用于当前本地 beta。
- [tauri.conf.json](../../src-tauri/tauri.conf.json) 有 updater 公钥与 endpoint，但 `createUpdaterArtifacts=false`。本轮未核验私钥存在或更新服务器可用，不能声明自动更新已交付。
- CI 目前只打包 macOS；Windows/Linux 完整分发未验收。

## 正式发行前

macOS 正式发行需配置有效 Developer ID 身份，签名嵌套二进制/库，提交公证并装订票据；随后在全新环境验证 Gatekeeper、启动、离线模型及升级。当前配置需要相应修改，单独设置环境变量不构成完成证明。

Windows 需先完成运行时分发和本地构建验证，再接入 Authenticode 与可信时间戳。实际脚本、证书存储和 CI 注入方式尚待实现。

更新签名与系统代码签名是两个契约：前者验证更新产物，后者验证可执行应用。需核验公私钥匹配、更新清单、下载校验、安装失败恢复及版本路径。私钥、证书口令和服务凭据不得进入仓库、构建日志或普通文档。

详细门禁见[开发与验证](../../docs/DEVELOPMENT.md)及[更新计划](../../docs/UPDATE_PLAN.md)。本说明不执行证书采购、密钥操作或发布。
