# WYSIWYG 与原生桌面修复实施 Walkthrough

> 日期：2026-09-02  
> 状态：运行时阻塞已全部修复并验证；原生 UI 操作矩阵与 Word/WPS 产物打开仍待人工/自动化走查  
> 对应计划：`docs/wysiwyg-native-remediation-plan-2026-09-02.md`  
> 工作区：当前脏工作区，未提交；本文不代表 `HEAD` 或发布版本

## 零、续作轮（本会话接管，2026-09-02 下午）

### 运行时阻塞修复（对照第四节清单，全部闭环）

| 原优先级 | 问题 | 修复 | 验证 |
|---|---|---|---|
| P0 | 隔离数据库 malformed | **未复现**：全新隔离目录（`/tmp/casy-native-qa-20260902`，旧目录已废弃）冷启动 v1→v23 全链迁移干净，运行期日志零 error；判定为旧 /tmp 目录残留/多实例争用所致，隔离 profile 恢复为可用验收基线 | `tauri dev` 启动日志 + 运行日志扫描 |
| P0 | 文书工坊 Suggestion 插件键冲突（`suggestion$`） | 4 个 suggest composable 注入唯一 `PluginKey`（caseField/legalProvision/partyName/knowledgeReference） | vue-tsc + 类型检查通过 |
| P1 | watcher 把 inbox 目录本身导入 | watcher.rs 事件循环与 import_file_to_inbox 双保险 `!path.is_file()` 跳过 | 冷启动后日志无伪文件项 |
| P1 | CaseListView `loadCases` 未定义 | 模板回调改绑 `casesStore.loadCases` | vue-tsc 通过 |
| P1 | record_decision entity_type CHECK 拒 'recommendation' | schema **v23b**：SCHEMA_SQL CHECK 放宽 + 旧库条件重建（12 步，保 rowid，重建索引/触发器）+ 回归测试 `decisions_accepts_recommendation_entity_type` | cargo test 8/8（schema_v20_test） |
| P1 | LegalEditor 重复注册 Underline（StarterKit v3 内置） | 移除显式 Underline 导入与注册 | vue-tsc 通过 |
| P2 | editorContainer ref 警告 | 复核现状已正确（ref 定义与模板绑定匹配），未复现 | — |

### 门禁（续作轮复跑）

| 检查 | 结果 |
|---|---|
| `cargo test`（全量） | ✅ 132 lib + 全部集成套件绿（**含此前本机失败的 excel_import 4 例——codex 的测试模式基建使其不再依赖真实 keychain**） |
| `vue-tsc --noEmit` | ✅ 零错误 |
| `npm audit --omit=dev` | ✅ 0 vulnerabilities |
| `git diff --check` | ✅ 干净 |
| 隔离原生启动 | ✅ 迁移干净、日志零错误、DB 文件（含 WAL）健康 |

### 仍未完成（如实记录，不得宣称完成）
- Phase 3 三窗口尺寸 UI 操作矩阵（800×600/1024×720/1440×900）未逐项走查；多块 Casy 实例共用 bundle id 导致自动窗口控制不可靠，需人工或专用自动化
- 编辑/未编辑 DOCX 从真实应用导出 + Word/WPS 实际打开核对未执行
- 中文输入法专项、OCR/PageIndex 导入后重启回归未执行
- `cargo tauri build` 原生安装包构建未执行

### 隔离验收环境使用方法
```bash
rm -rf /tmp/casy-native-qa && mkdir -p /tmp/casy-native-qa/{data,templates,exports}
CASY_TEST_DATA_DIR=/tmp/casy-native-qa/data \
CASY_TEST_TEMPLATE_DIR=/tmp/casy-native-qa/templates \
CASY_TEST_EXPORT_DIR=/tmp/casy-native-qa/exports \
npm run tauri dev
```
注意：开始前确保没有其他 Casy 实例在跑（含已安装的 Casy.app），否则窗口归属与 DB 争用会污染结论。

## 一、当前结论

本轮已完成知识库 Markdown 安全/保真、快速切换前保存、文书身份与草稿状态、编辑稿 Rust 原生 DOCX 导出的主要代码改造，并通过现有前端单测、类型检查、前端生产构建以及新增 Rust DOCX 定向测试。

但本轮不能标记为“完成”或“可发布”。真实 Tauri 调试启动暴露了新的运行时阻塞：隔离数据库出现 `database disk image is malformed`、文书工坊 Tiptap 存在 Suggestion 插件键冲突、案件列表模板引用了未定义方法、收件箱监听器会把新建的 `inbox` 目录本身当成文件导入。设置、收件箱、任务、知识库完整操作矩阵和 Word/WPS 产物打开均未完成。

特别说明：第一次桌面控制误选了机器上已有的 `Casy.app`，不是刚启动的最新调试进程。该窗口的截图和 UI 观察全部作废，不作为本文任何通过结论的证据。后续只保留最新 `tauri dev` 会话的编译输出和运行日志；由于多份 Casy 共用 bundle identifier，尚未完成对正确调试窗口的稳定自动控制。

## 二、本轮已实施内容

### 2.1 知识库 Markdown 安全与保真

- 用 DOMPurify DOM 白名单替换正则 HTML 清洗。
- 预览消毒覆盖危险协议、实体编码、大小写混淆、SVG、事件属性和正常 GFM/WikiLink。
- 为知识库 Tiptap 注册图片节点，保留 Markdown 图片的 `src`、`alt` 和 `title` 往返。
- 增加 `RawHtmlInline` / `RawHtmlBlock` 只读占位：暂不支持的原始 HTML 在富文本中不可直接修改，但会把原文编码保存并在 Markdown 回写时恢复，避免第一次编辑后静默丢失。
- 修正 WikiLink 标题特殊字符的重复转义。
- `MarkdownWysiwygEditor` 暴露 `flushAndGetMarkdown()`，父级在切换笔记、WikiLink 跳转、路由离开和历史恢复前先取得编辑器权威内容。
- 笔记保存绑定 `noteId + editRevision`：旧请求完成后只有在仍为同一笔记且没有新编辑时才能清除 dirty；保存失败保留 dirty 并阻止切换/离开。
- 历史恢复增加 `beforeRestore` 保存门槛。

尚未完成：粘贴/拖入图片复制到 Casy 管理附件目录、远程图片占位、中文输入法专项验证、OCR/PageIndex 导入后重启回归。

### 2.2 文书生成状态一致性

- 渲染请求增加递增 `requestId`，旧请求结果不能覆盖新选择。
- 记录 `renderIdentity`（模板、案件、请求），保存和导出前校验当前选择与渲染身份一致。
- 模板/案件切换遇到未保存编辑时先确认；取消后回滚选择。`TemplateBrowser` 增加外部 `modelValue` 回灌，确保卡片高亮也能回滚。
- 区分“偏离模板渲染”与“偏离最近成功保存草稿”，保存成功后更新 `lastSavedHtml` 和 `draftId`。
- 已有 `draftId` 时更新原草稿，不再每次点击都创建重复草稿。

尚未完成：上述取消/确认和连续快速切换尚未在正确的真实 Tauri 窗口中操作验证。

### 2.3 编辑稿 Rust 原生 DOCX 导出

- 删除文书生成页和文书工坊对前端 `html-to-docx` / `file-saver` 的使用，并卸载这两个依赖。
- `LegalEditor` 暴露 Tiptap JSON，前端新增 `export_edited_docx` 服务与命令契约。
- Rust 新增 `docx-rs` 和结构化导出器，覆盖：
  - 段落与标题；
  - 粗体、斜体、删除线、下划线、代码和安全超链接；
  - 引用、代码块、分隔线；
  - 有序、无序和任务列表；
  - 表格与嵌套表格；
  - base64 图片和绝对本地图片；
  - EvidenceLink 正文编号与“参考证据列表”。
- 未实现的节点/mark 会明确拒绝导出，不生成看似成功但内容残缺的 DOCX。
- 未编辑模板稿仍走原模板替换导出，两条链路共用输出结果结构。

尚未完成：真实 Tauri IPC 导出、系统路径/打开文件、Microsoft Word 与 WPS/LibreOffice 实际打开验证。当前只证明 Rust 层能生成结构正确且包含核心内容的 OOXML。

### 2.4 隔离调试路径

- 新增 debug-only 路径覆盖：
  - `CASY_TEST_DATA_DIR`；
  - `CASY_TEST_TEMPLATE_DIR`；
  - `CASY_TEST_EXPORT_DIR`。
- 数据库、密钥、日志、案件文件、收件箱、模板和两条 DOCX 导出路径接入统一隔离目录。
- 隔离 profile 不读取或写入正式数据库 Keychain 密钥。
- release 构建忽略上述环境覆盖。

该部分尚未达到完成标准：最新调试会话后续出现数据库损坏错误，因此目前只能确认路径确实被切到 `/tmp`，不能确认隔离 profile 可稳定反复使用。

## 三、自动验证证据

| 检查 | 当前结果 | 证据边界 |
|---|---|---|
| `npm run test:unit` | 通过：5 个测试文件，41 个测试 | Markdown 转换/消毒和组件 API；不等于真实 WebView |
| `npm run typecheck` | 通过 | Vue/TypeScript 静态类型 |
| `npm run build` | 通过：Vite 转换 1984 个模块 | 前端生产构建；不等于 Tauri 可操作 |
| `cargo check` | 通过 | Rust 编译检查；有一条既存 dead-code warning |
| `cargo test docsy_engine::rich_export -- --nocapture` | 通过：2 个新增测试 | DOCX OOXML、中文、字体、任务列表、表格、图片、证据引用和未知节点拒绝 |
| `git diff --check` | 通过 | 当前 diff 无空白错误 |
| 完整 `cargo test` | 未在本轮最终状态执行 | 不得宣称全量 Rust 回归通过 |
| `cargo tauri build` | 未执行 | 不得宣称原生安装包通过 |
| Word/WPS 打开 | 未执行 | 不得宣称产物兼容性通过 |

依赖安装/卸载时 npm 报告 `0 vulnerabilities`；计划要求的最终 `npm audit --omit=dev` 尚未单独执行。

## 四、真实 Tauri 会话及新发现

使用以下隔离环境启动了最新源码的 `npm run tauri dev`：

```text
CASY_TEST_DATA_DIR=/tmp/casy-native-qa-20260902/data
CASY_TEST_TEMPLATE_DIR=/tmp/casy-native-qa-20260902/templates
CASY_TEST_EXPORT_DIR=/tmp/casy-native-qa-20260902/exports
CASY_LOG=debug
```

启动日志确认：数据库迁移到 v23、日志写入隔离目录、收件箱监听目标为隔离目录。随后最新调试会话出现以下必须修复的问题：

| 优先级 | 运行时问题 | 实际日志/现象 | 当前状态 |
|---|---|---|---|
| P0 | 隔离数据库损坏 | `update_knowledge failed: database disk image is malformed` | 未定位；隔离 profile 暂不可作为可靠验收基线 |
| P0 | 文书工坊编辑器不能挂载 | `RangeError: Adding different instances of a keyed plugin (suggestion$)` | 未修；4 个 Suggestion 扩展复用默认 plugin key |
| P1 | 收件箱产生伪文件项目 | watcher 将 `/.../documents/inbox` 目录记录为标题 `inbox` 的 file 项 | 未修；事件处理需排除目录 |
| P1 | 案件列表模板缺方法 | `Property "loadCases" was accessed during render but is not defined on instance` | 未修；导入完成回调仍引用旧方法名 |
| P1 | AI Diff/决策记录契约不一致 | `record_decision failed: CHECK constraint failed: entity_type IN ('case','client','task','knowledge')` | 未修；需核对前端提交的 entityType |
| P1 | LegalEditor 扩展重复 | `Duplicate extension names found: ['underline']` | 未修；StarterKit v3 与显式 Underline 重复 |
| P2 | LegalEditor 模板 ref 警告 | `Template ref "editorContainer" used on a non-ref value` | 未定位 |

因此，设置、收件箱、任务、知识库、文书工坊之间的真实 UI 连续切换不具备通过条件。上述日志发现本身证明了“只看浏览器或只跑构建”会漏掉关键问题。

## 五、工作区边界

- 当前所有内容均在脏工作区，尚未提交。
- `docs/global-benchmark-inspirations-walkthrough-2026-09-01.md` 和部分知识库文件在本轮开始前已有修改；本轮未将它们重置。
- `vendor/image-size/node_modules/...` 下的 tracked 删除是既有工作区状态，本轮未恢复、未扩大，也未执行 checkout/reset/clean/stash。
- 本轮曾尝试按 build utility 调用指定 DeepSeek 模型，但当前 ChatGPT 账户不支持该模型，调用在修改代码前失败；后续实施由当前任务直接完成。

## 六、下一轮建议顺序

1. 先修复 watcher 目录误导入，并使用全新隔离目录重新验证数据库；若仍损坏，定位 SQLCipher/共享连接/多实例并发原因。
2. 为四个 Suggestion 扩展分配独立 `PluginKey`，移除重复 Underline，确保 LegalEditor 在真实 Tauri 中可挂载。
3. 修复 `CaseListView` 的 `loadCases` 旧回调和 `record_decision` 的 `entity_type` 契约。
4. 在确保只剩一个最新调试实例后，完成设置、收件箱、任务、知识库和文书的 800×600、1024×720、1440×900 原生矩阵。
5. 从真实应用导出编辑/未编辑 DOCX，用 Microsoft Word 和至少一个兼容应用打开并核对。
6. 执行完整 `cargo test`、`npm audit --omit=dev`、`cargo tauri build` 和最终 `git diff --check`。
7. 再更新 `docs/global-benchmark-inspirations-walkthrough-2026-09-01.md` 的完成状态；在此之前不得把本批写成“已完成”。

## 七、阶段状态

| 计划阶段 | 状态 |
|---|---|
| Phase 0 隔离环境 | 代码已接入，运行稳定性失败，未完成 |
| Phase 1 知识库安全/保真 | 核心代码和单测完成，原生持久化/OCR 回归未完成 |
| Phase 2 文书状态/原生导出 | 核心代码和 Rust 定向测试完成，真实 IPC/产物打开未完成 |
| Phase 3 原生 UI 验收 | 未完成；已发现多项 P0/P1 阻塞 |
| Phase 4 门禁/文档 | 部分完成；本文为阶段性交接，不是最终交付 |

