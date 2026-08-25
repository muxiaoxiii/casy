# 第三方软件许可清单（生成配置）

> 本目录随发版流程生成 `THIRD-PARTY-NOTICES.md`。
> R-7 合规包要求：分发产物必须附带完整三方许可声明。

## 生成方式

### Rust 侧（cargo-about）

```bash
cargo install cargo-about
cd src-tauri && cargo about generate about.hbs > ../docs/compliance/THIRD-PARTY-RUST.md
```

模板要点（`about.hbs`）：遍历 `overrides`，逐包输出 name/version/authors/license/license 文本。

### 前端侧（license-checker）

```bash
npx license-checker --production --json --out docs/compliance/npm-licenses.json
```

## 已知重点许可（人工核对项）

| 组件 | 许可 | 注意 |
|---|---|---|
| Tauri v2 全家桶 | MIT/Apache-2.0 | 商用友好 |
| SQLCipher | BSD-style + 加密豁免条款 | **需保留其特定声明** |
| Element Plus | MIT | — |
| vue-draggable-plus / vue / vite 系 | MIT | — |

## 发版前检查清单

- [ ] `cargo about generate` 无失败条目（含 copyleft 检查：当前无 GPL 依赖，若引入须评审）
- [ ] THIRD-PARTY 文件已打包进 DMG/MSI 的资源目录并在「设置 → 关于」可打开
- [ ] EULA.md / PRIVACY.md 版本号与发布说明一致
