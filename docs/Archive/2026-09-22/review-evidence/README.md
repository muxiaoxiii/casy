# 本次审阅验证证据

日期：2026-09-22。基线：main 工作区，HEAD f92564b，含既有未提交改动，源码版本 0.1.2。两份日志是文档整理阶段重新执行的结果，未修改测试或功能代码。

| 日志 | 命令 | 结果 |
| --- | --- | --- |
| [frontend-tests.log](frontend-tests.log) | `npm run test:unit` | 269 通过、1 失败；55 文件通过、1 文件失败 |
| [script-tests.log](script-tests.log) | `node --test scripts/binary-architecture.test.mjs scripts/license-policy.test.mjs scripts/runtime-components.test.mjs` | 5 通过 |

前端失败见当前 [审阅 R-01](../../../CODE_REVIEW_2026-09-22.md)。本目录不包含当前最终源码的全量 Rust、GUI、跨平台或外部服务验收，不能据此宣布新版发布通过。
