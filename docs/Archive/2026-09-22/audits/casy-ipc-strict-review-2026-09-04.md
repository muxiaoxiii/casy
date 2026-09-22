# Casy 严格 IPC 收口审查与改进方向（2026-09-04）

## 结论

严格 IPC 收口已经完成：前端业务代码不再依赖 `tauriCallSafe<T>` / `tauriCall<T>` 显式泛型，不再使用动态命令名，`tauriBridge` 不再保留宽 fallback overload。所有前端字面量 IPC 命令均登记到 `CommandMap`，核心写命令改为精确 DTO；第二批已继续把 service 返回值和插件工具参数从宽类型迁到命令契约/业务 DTO。

当前可核验口径：

| 项 | 当前值 |
|---|---:|
| Schema | v26 |
| Rust generate_handler 注册命令 | 311 |
| 前端字面量 IPC 命令 | 254 |
| CommandMap 契约键 | 276 |
| specta bindings 类型 | 121 |
| 显式泛型 IPC 调用 | 0 |
| 动态命令名 IPC 调用 | 0 |
| 业务代码直接 `invoke` IPC 调用 | 0 |

## 已实施修复

1. `tauriBridge` 删除宽 fallback overload，只接受 `K extends keyof CommandMap & string`。
2. `tauriBridge` 参数签名改为按命令契约判断：空参数命令可以省略 args，有参数命令必须传 args。
3. `CommandMap` 补齐前端调用命令，并将 Case / Task / Calendar 写命令改为手写精确 DTO。
4. `src/types/ipc.ts` 新增 IPC 专用 DTO，覆盖动态 PATCH、日历载荷、提醒规则、飞书映射、飞书表结构、节假日条目等。
5. `main.js` 的崩溃日志写入和 `ReasoningSearchPanel.vue` 的推理检索改回 `tauriCallSafe`，不再直接 `invoke`。
6. `tests/contract.commands.test.ts` 增加静态门禁：
   - frontend ⊆ CommandMap
   - frontend ⊆ Rust
   - CommandMap ⊆ Rust
   - 禁止显式泛型 IPC 调用
   - 禁止动态命令名 IPC 调用
   - 禁止业务代码直接 `invoke` 绕过 bridge
   - 禁止 CommandMap 出现 `Record<string, unknown>` / `unknown[]` / `any[]` 宽 fallback 类型
7. 修复节假日条目字段口径：后端返回 `kind`，前端 store 不再按不存在的 `type` 判断工作日。
8. 飞书字段发现按后端真实返回 `type` 入契约，服务层补 `fieldType` 兼容现有 UI。
9. 继续收紧 service 返回值：`calendar` / `sync` / `settings` / `ai` / `knowledge` / `files` / `cases` / `inbox` 等服务优先使用 `CommandMap[K]['result']` 或明确业务 DTO，减少组件层断言。
10. 收紧插件工具参数：case/task/knowledge 写工具改用 `CreateCasePayload`、`CasePatchInput`、`CreateTaskPayload`、`UpdateTaskPayload`、`KnowledgePatchInput` 等 DTO；空参数工具从 `Record<string, unknown>` 改为 `{}`。
11. `case_stats` 在 service 边界完成 wire → business 归一化，把后端 tuple 数组 `{ byTrack: [track,count][] }` 转成 store 使用的 `{ track,count }[]`，`stores/cases` 不再自行断言。
12. 任务表单保存、任务 store 变更载荷、收件箱处理结果继续接入 IPC DTO / CommandMap 结果类型，避免刚收口的任务域重新出现宽 payload。

## 真实案件测试前追加收口

1. 案件路由补齐 `民事诉讼+行政诉讼`，覆盖前端类型、案件向导、列表创建、看板筛选、Excel/飞书导入与 Schema v25 迁移。
2. PDF 页文本从 `LIKE` 扫描升级为 `document_pages_fts` FTS5 trigram 索引；Schema v26 新建/迁移均会 rebuild，文档处理完成后的页写入由触发器同步。
3. 全局搜索现在覆盖知识库、文件摘要和最新 completed OCR job 的页级文本，页级结果带 `[pN]` 摘要；用户查询转为安全 FTS phrase，避免标点/引号导致搜索失败。
4. 普通案卷上传改为先复制进案件归档目录再登记，满足后端路径归属校验。
5. 收件箱归档分类会归一为稳定 `case_files.category` key，避免中文目录名或历史别名进入枚举字段。
6. 编辑器知识块引用、WikiLink 建议浮层、文书导出返回字段、导出路径和默认打开路径已按安全审查排除高风险隐患。

## 审查发现

### P1：业务组件仍有 `any` / 宽对象适配点

IPC 边界和 service 层已经明显收紧，但若干 Vue 组件仍因为第三方 UI 事件、TipTap 编辑器、导入预览表格或历史组件状态使用 `any` / `Record<string, unknown>`。这些不是 CommandMap fallback，但会削弱 UI 层的重构反馈。

改进方向：按页面风险拆小批处理，优先处理任务视图、案件导入、文件面板；TipTap/ProseMirror attrs 保留专用 JSON 类型，不追求完全消灭宽类型。

### P1：插件容器和 AI 工具执行仍是动态 JSON 边界

单个插件的高风险写工具已经改为精确 DTO，但 `defineTool` / `CasyContext.executeTool` / `tool-caller` 仍需要接收模型产出的运行时 JSON。这是必要动态边界，但当前类型校验主要靠 JSON Schema 和服务端网关。

改进方向：为 typed tool helper 增加 schema/type 同源能力，或给关键写工具补运行时 validator，让 AI 工具参数在进入 service 前先被明确校验并返回结构化错误。

### P1：错误仍主要是字符串

Rust 命令多以 `Result<T, String>` 透传，前端只能显示文本，无法稳定区分验证失败、权限失败、冲突、外部服务失败。

改进方向：引入最小 `AppError { code, message, detail? }`，先在新增/高风险命令使用，前端 `tauriBridge` 支持结构化错误解析并保留字符串兼容。

### P2：`bindings.ts` 生成顺序会随 export 注册变化产生噪声 diff

本轮 `cargo test` 生成 bindings 时出现类型声明顺序调整。内容正确，但 review 成本偏高。

改进方向：检查 `src-tauri/src/export_bindings.rs` 的导出注册是否可稳定排序；若 Specta 本身顺序不可控，增加生成后稳定化脚本或接受为生成物噪声。

### P2：浏览器 mock 仍在 bridge 层

严格 IPC 后，生产调用链已经收紧；但设计文档里的长期方向是把 mock 上移到 service fake。当前 bridge mock 仍是历史折中。

改进方向：后续做 FakeService 注册表，把浏览器预览数据从 `mockData` 移出 bridge，生产 bridge 只负责 IPC。

## 下一步实施顺序

1. 页级 PDF 结果前端模型化：把页码、坐标、OCR 置信度、文件 id 暴露给知识库搜索和编辑器证据引用。
2. 组件层去 `any`：先做任务视图/任务表单、案件导入、文件面板这三个有明确业务 DTO 的页面。
3. AI 工具参数运行时校验：让 JSON Schema 与 TypeScript DTO 同源，关键写工具进入 service 前先 validate。
4. 结构化错误：先定义 `AppError { code, message, detail? }` 和 bridge 解析，不急着全量改 Rust 命令。
5. FakeService 下沉：把浏览器预览 mock 从 `tauriBridge` 移到 service fake 注册表。
6. bindings 生成稳定化：检查 export 注册顺序，必要时增加生成后排序脚本，降低 review 噪声。

## 验证

本轮已通过：

```bash
npm run typecheck
npm run test:unit
cd src-tauri && cargo test
npm run build
```
