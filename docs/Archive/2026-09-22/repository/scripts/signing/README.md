# 签名与公证就绪包（R-2）

> 前置：Apple Developer Program（$99/年）+ Windows Authenticode 证书（OV 级即可）。
> 采购完成前，本目录脚本已就绪但不会在 CI 中启用。

## macOS

```bash
# 1. 导入证书到登录钥匙串后，设置环境变量
export APPLE_CERT_ID="Developer ID Application: <你的名字> (TEAMID)"
export APPLE_ID="your@apple.id"
export APPLE_PASSWORD="@keychain:AC_PASSWORD"   # App 专用密码存钥匙串
export APPLE_TEAM_ID="TEAMID"

# 2. tauri.conf.json 已预留 updater；公证由 tauri build 自动走 notarytool
npm run tauri build
```

## Windows

```bash
# signtool 路径加入 PATH 后
scripts/signing/sign-win.ps1 -PfxPath <cert.pfx> -TimestampUrl http://timestamp.digicert.com
```

## 更新签名

私钥已生成于 ~/.tauri/casy.updater.key（**勿入库**），构建时通过
`TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 注入。
