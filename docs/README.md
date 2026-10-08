# 现行文档索引

核对日期：2026-10-08（审计复核）。文档描述当前工作区；源码、安装包和验证状态分开记录。

| 文档 | 用途 |
| --- | --- |
| [项目入口](../README.md) / [项目状态](../Casy-STATUS.md) | 产品范围、版本及验证 |
| [视觉系统](VISUAL_SYSTEM.md) | Judicial Docket 图标、色彩、原型及生成方法 |
| [0.1.3 验收](RELEASE_0.1.3.md) | 包、回归、真实转换与限制 |
| [架构与模块](ARCHITECTURE.md) | 代码入口和数据流 |
| [文档处理](DOCUMENT_PIPELINE.md) | OCR、转换、超时、大文件边界 |
| [开发与验证](DEVELOPMENT.md) | 环境、隔离测试、构建和发布 |
| [代码审阅](CODE_REVIEW_2026-09-22.md) | 当前问题、证据及影响 |
| [全面代码审计 2026-10-08](CODE_AUDIT_2026-10-08.md) | 接手前全量审计：门禁实测、bunny 缺陷处置核对、现存缺陷分级、文档漂移 |
| [更新计划](UPDATE_PLAN.md) | 优先级、依赖和验收条件 |
| [程序期限](procedure-deadline-rules.md) | 规则语义与人工核实边界 |
| [运行时分发](runtime-distribution-plan.md) | 模型、运行库、许可证、平台 |
| [数据与安全](DATA_AND_SECURITY.md) | 存储、密钥、网络、备份 |

## 2026-09-23 证据归档

| 归档 | 内容 |
| --- | --- |
| [日历布局回归](Archive/2026-09-23/calendar-layout-regression/) | 时间线/周历/窄幅布局修复截图与说明 |
| [横向甘特排期](Archive/2026-09-23/gantt-planning/) | `task_plans` 实现、浏览器与 Rust 回归证据 |
| [任务链路与休息时段](Archive/2026-09-23/task-chain-leave/) | 计划投影、截止别名、半天休息及接手补审 |
| [0.1.3 修订包](Archive/2026-09-23/validation-0.1.3-revision/) | 主题/WebDAV/日历修订包核验产物 |

## 归档

原 audits、anti-gravity、devlog、ui 和其余旧根文档已进入 [Archive](Archive/README.md)。原 README、状态、工具与签名 README 保留快照。保留并更新的旧 docs 根文件为 `procedure-deadline-rules.md` 和 `runtime-distribution-plan.md`，其原副本也已归档。

`docs/compliance` 是指向归档原目录的兼容符号链接，保留打包脚本和版权注释使用的路径。第三方许可原文不改写。旧 EULA/隐私文本的技术描述并非当前能力承诺，见数据与安全文档。

维护时：当前问题在审阅中登记，通过编号链接计划；测试必须写日期、命令、范围及失败/忽略项；阶段过程文档归档，不再并存多套“当前总纲”。
