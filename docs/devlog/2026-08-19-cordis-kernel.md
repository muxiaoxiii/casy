# Casy 架构内核升级：cordis 风格数据通路（对标 DeepSeek Harness）

> **日期**: 2026-08-19
> **背景**: 用户定位 Casy 为"对标工程"——架构与 AI 部分学习 DeepSeek Harness 的 cordis 内核，把数据通路做好。
> 本轮把前端插件层从"仿壳的注册表"升级为 cordis 风格内核。

---

## 一、cordis 内核研究结论

阅读 `@deepseek-ai/cordis` 源码（DSH 的依赖）后确认，cordis 的核心不是插件注册，而是：

| 机制 | 作用 |
|------|------|
| **Context 服务解析器（Proxy）** | `ctx.cases` 属性读取自动解析到已注册 Service——模块间不直接 import，全部经 ctx 取服务 |
| **Service + inject 依赖声明** | 插件/服务声明依赖，启动前校验 |
| **Fiber 生命周期** | 每个插件一个 Fiber，dispose 自动清理 effects（事件/定时器） |
| **作用域事件 + isolate** | 子上下文事件自动回收 |
| **Logger / 配置驱动加载** | `ctx.logger(name)`；插件树由配置文件加载 |

## 二、Casy 落地（前端 `src/core/`）

### 数据通路（本轮核心）

```
视图 / AI 工具（插件）
   ↓ ctx.<service>.<method>()
业务服务（core/services/*.ts，9 个）
   ↓ tauriBridge（tauriCallSafe）
Rust 命令（写入口唯一）
   ↓
SQLite
```

### 新增/改造文件

| 文件 | 内容 |
|------|------|
| `src/core/plugin/types.ts` | 增强：`Service` 基类、`Fiber`、`InjectKey`、`CasyLogger`；CasyContext 接口新增 provide/unprovide/getService/plugin/effect/fork/logger |
| `src/core/plugin/context.ts` | 重写为 cordis 风格：Proxy 服务解析、FiberImpl（逆序清理）、作用域事件（on 登记到当前 Fiber）、`ctx.logger`/getLogger |
| `src/core/services/*.ts` | **9 个业务 Service**：cases/tasks/knowledge/calendar/inbox/reminder/files/sync/settings，每个封装 tauriBridge 命令 |
| `src/core/services/index.ts` | 注册全部服务 + module augmentation（`ctx.cases` 等获得类型提示） |
| `src/core/plugin/initializer.ts` | 先注册服务、再安装 9 个插件 |
| `src/core/plugins/*.ts` | 8 个插件 execute 从直接 `tauriCallSafe` 改为 `ctx.<service>.<method>()`——插件零直接 tauri 调用 |

### 服务清单（ctx.xxx）

| 服务 | 方法 |
|------|------|
| ctx.cases | list / get / create / update / remove / search / stats |
| ctx.tasks | list / create / toggle / update / remove |
| ctx.knowledge | list / search / create / update / remove |
| ctx.calendar | events / deadlineWarnings / dashboardStats |
| ctx.inbox | list / add / process / file / dismiss |
| ctx.reminder | rules / createRule / log / startEngine |
| ctx.files | list / add / remove |
| ctx.sync | status / testWebdav / push / pull |
| ctx.settings | get / save / configureAi / webdavCredentials |

## 三、设计哲学对齐

- **§原则六 双路径铁律**：确定性执行永远在 Rust 命令；服务层是前端唯一数据通路
- **§11.11 智伴层组件化**：没有特权核心——业务模块 = Service + 插件（工具），AI 工具与视图共用 ctx 服务
- **写入口唯一**：所有写操作经服务 → tauriBridge → Rust 命令；确认机制（§11.4）在插件层保留

## 四、遗留（后续增量）

- **视图/store 迁移**：44 个 Vue 组件仍直接 tauriCallSafe。服务通路已建立（插件已验证），视图迁移作为后续里程碑逐模块推进（tasks → cases → ...）
- **isolate/intercept**：cordis 的服务隔离/拦截未移植（当前无多实现需求，克制）
- **配置驱动加载**：插件安装仍在 initializer 写死（Casy 是固定 9 模块，暂不需要 loader）
