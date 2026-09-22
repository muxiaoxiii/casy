# 依赖安全审计报告（R-4）

> **日期**: 2026-08-24　**工具**: npm audit / cargo audit（advisory-db 1226 条）
> **结论**: npm 高危清零；Rust 侧修复 1、接受风险 2（附理由与复评时点）

## 一、已修复

| 项 | 修复 | 验证 |
|---|---|---|
| nanoid <3.3.18（GHSA-2v37-7h3g-55p8，high） | `npm audit fix` | npm audit → 0 vulnerabilities；build 绿 |
| h2 0.4.15 无界空 DATA 帧（RUSTSEC-2026-0258，经 tauri-plugin-updater→reqwest 引入） | `cargo update -p h2` → 0.4.19 | cargo audit 该条消除 |

## 二、接受风险（附理由）

| 项 | 版本 | 理由 | 复评时点 |
|---|---|---|---|
| quick-xml 命名空间无界分配 + 二次方属性检查（RUSTSEC-2026-0195/0194） | 0.31.0（calamine 锁定）/ 0.37.5（直接依赖 docsy_engine） | 攻击面为"解析不可信 XML"；本项目仅**本地解析用户自有 Excel/文书模板**，无远程输入路径。升级需跨 minor 适配（calamine 换版 + docsy_engine 解析调用点），属 C 线域变更 | C 线解冻后随 B1 收口一并处理 |
| gtk3 栈 unmaintained 警告 ×15（atk/gdk/glib 等） | Tauri v2 Linux 底层 | Tauri 上游选择，非本项目可控；macOS 为主要目标平台 | Tauri v3 迁移评估 |

## 三、常态化

- CI（R-3）中接入 `npm audit --audit-level=high` 与 `cargo audit`（失败阈值：high+），报告产物归档
- 本报告随每次发版前重跑刷新
