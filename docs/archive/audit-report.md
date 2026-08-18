# Casy 代码质量审计报告

> 审计日期：2026-08-03
> 审计范围：全部 Rust 后端 + Vue 前端 + 文档架构

## 一、已修复的问题

### 1. 编译 warnings 清理（23 → 1）

| 类别 | 数量 | 处理方式 |
|------|------|---------|
| 已实现但前端未调用的函数 | 14 | 标记 `#[allow(dead_code)]` |
| 未使用的导入 | 2 | 注释/标记 |
| 未使用的变量 | 4 | 前缀 `_` |
| 不需要 mut 的变量 | 1 | 移除 mut |
| 重复函数定义 | 1 | 删除 `auto_classify_from_name`，统一用 `files::auto_classify` |
| 未使用的类型 variant | 1 | 标记 `#[allow(dead_code)]` |

### 2. 重复代码消除

- `inbox.rs::auto_classify_from_name` 与 `files/mod.rs::auto_classify` 功能重复
- 统一使用 `files::auto_classify`，inbox.rs 改为调用它

## 二、当前架构状态

### 模块结构
```
src-tauri/src/
├── ai/           — AI 分类/提取/写作（32K行 mod.rs，过大）
├── app_log.rs    — 日志系统（tracing + 文件输出）
├── commands/     — Tauri 命令层（15 个文件）
├── db/           — 数据库层（schema/cases/search）
├── deadline/     — 期限引擎（holidays + engine）
├── docsy_engine/ — 文书生成引擎
├── email/        — 邮件监控
├── files/        — 文件管理（模板化目录 + 智能路由）
├── formula/      — 公式引擎（AST/Parser/Eval/Dependency）
├── parse/        — 文档解析
├── sync/         — 飞书同步 + WebDAV
├── tray.rs       — 系统托盘
└── watcher.rs    — 文件夹监听
```

### 统计
- 代码总行数：32,283
- Tauri 命令：110+
- 测试：63/63 通过
- 编译 warnings：1（param_idx 未读取，不影响功能）
- 前端组件：35 个 Vue/JS 文件

## 三、发现的问题（待修复）

### 🔴 Critical

| # | 模块 | 问题 | 影响 |
|---|------|------|------|
| 1 | `ai/mod.rs` | 单文件 32K 行，应拆分为多个子模块 | 可维护性差，编译慢 |
| 2 | `commands/mod.rs` | build_handler 中有 11 个非命令的类型/trait 被误匹配 | 潜在编译问题 |

### 🟡 Warning

| # | 模块 | 问题 | 影响 |
|---|------|------|------|
| 3 | `files/mod.rs` | `auto_classify` 返回 `&'static str` 但 inbox 期望 `String` | 类型不一致 |
| 4 | `formula/` | 多个公共方法标记 dead_code，说明公式引擎未被充分集成 | 功能浪费 |
| 5 | `sync/feishu.rs` | send_feishu_message 等函数未被任何 Tauri 命令调用 | 飞书消息通道未接通 |
| 6 | `commands/reminder.rs` | ReminderEngine 的 check_interval_secs 字段未使用 | 定时检查未实现 |
| 7 | 文档 vs 代码 | architecture.md 定义了 Phase B-E，但部分功能标记为完成实际只是骨架 | 进度虚报 |

### ℹ️ Info

| # | 模块 | 问题 | 影响 |
|---|------|------|------|
| 8 | 前端 | 35 个组件中部分可能未被路由使用 | 需要前端路由审计 |
| 9 | 测试 | 63 个测试全在 Rust 端，前端 0 测试 | 前端质量无保障 |
| 10 | 日志 | app_log 已实现但部分模块仍用旧的 `log::` 宏 | 日志格式不统一 |

## 四、改进方案

### P0：AI 模块拆分（预计 2 天）

当前 `ai/mod.rs` 有 32K 行，应拆分为：
```
ai/
├── mod.rs          — 公共 API + 配置
├── backend.rs      — AI 后端抽象（Ollama/OpenAI/DeepSeek）
├── classify.rs     — 文档分类
├── extract.rs      — 信息提取
├── writing.rs      — 写作辅助
├── prompts/        — Prompt 模板
│   ├── classify.md
│   ├── extract.md
│   └── writing.md
└── config.rs       — AI 配置管理
```

### P1：飞书消息通道接通（预计 1 天）

`send_feishu_message` 和 `create_feishu_task` 已实现但未被调用。需要：
1. 在 ReminderEngine 的 `trigger_reminder` 中，当 channel 包含 `feishu_message` 时调用 `send_feishu_message`
2. 当 channel 包含 `feishu_task` 时调用 `create_feishu_task`
3. 测试：手动触发一条飞书消息验证

### P2：公式引擎集成（预计 1 天）

公式引擎的 `evaluate_formula`、`DependencyGraph` 等核心方法未被使用。需要：
1. 在 `recalculate_case_formulas` 中使用 `evaluate_formula` 而非硬编码计算
2. 在案件更新时自动触发关联公式的重算
3. 前端：在动态字段组件中显示公式计算结果

### P3：前端路由审计（预计 0.5 天）

检查所有 Vue 组件是否被路由引用，清理未使用的组件。

### P4：测试覆盖（预计 2 天）

- 前端：关键组件的单元测试
- Rust：飞书 API 的集成测试、公式引擎的端到端测试
- E2E：收件箱→归档→案件目录的完整流程测试
