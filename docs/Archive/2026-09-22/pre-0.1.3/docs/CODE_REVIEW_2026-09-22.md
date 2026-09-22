# 代码审阅 · 2026-09-22

## 基线与范围

审阅当前 main 工作区（HEAD `f92564b`，含大量未提交改动），不是该提交本身或已交付 0.1.1 的完整审计。源码版本为 0.1.2，尚未打包。按用户要求，本次只更新文档与计划，没有继续修复功能。

重点追踪文档转换/图片导出、长 IPC、程序期限写入、同步状态和构建发布；其他模块作架构与能力核对，不宣称逐行覆盖全仓库。未进行真实资料库操作、外部服务测试、网络抓包、完整 GUI 或跨平台验收。

P1 表示下一次发布前应解决或明确验证处置；P2 表示可触发的功能缺陷或重要可靠性缺口。编号用于连接[更新计划](UPDATE_PLAN.md)，不是实现顺序。

| 编号 | 级别 | 问题 | 证据 |
| --- | --- | --- | --- |
| R-01 | P1 | 引用层级编辑回归失败 | 本次全量前端测试复现 |
| R-02 | P1 | 放开大文件限制后仍有多处整份内存分配 | 静态路径确认；未复现 OOM |
| R-03 | P2 | Markdown 相对图片跨目录转换未迁移 | 静态路径确认 |
| R-04 | P2 | 文字输入仍允许选择无法生成的双层 PDF | 静态路径确认 |
| R-05 | P1 | 自定义期限规则与审计写入缺少统一事务 | 静态路径确认；未做故障注入 |
| R-06 | P2 | 其他长写操作仍可能前端超时而后台继续 | 调用机制确认；备份等未实测超时 |
| R-07 | P2 | WebDAV 状态接口固定返回未连接 | 静态实现确认 |
| R-08 | P2 | 发布 CI 缺少安装包及运行时验收 | workflow 与本地交付记录确认 |

## R-01 P1 引用层级编辑回归失败

**触发：** 执行 `npm run test:unit`，`tests/unit/quoteSources.test.ts` 中 `preserves all four source depths through rich/source conversion and undo` 失败。270 项中 269 通过、1 失败。

**定位：** [quoteSources.ts](../src/shared/markdown/quoteSources.ts) 第 28 行，`Fragment.from(state.schema.nodes.blockquote.create(...))` 报错：
`Can not convert <blockquote(paragraph("正文"))> to a Fragment (looks like multiple versions of prosemirror-model were loaded)`。

**影响：** 引用深度与源码/富文本往返的发布回归门禁未通过。错误提示指向 ProseMirror 模块身份不兼容，但尚未证实是重复依赖、ESM/CJS 加载差异还是其他原因，也未证实桌面运行时出现同样故障。

**建议与验收：** 固定包管理器和锁文件，核对 Tiptap/ProseMirror 的解析路径及运行时身份；修复根因后保持此用例有效，完成全量前端测试和真实编辑器四层引用、切换、撤销/重做验收。不得用跳过用例代替修复。

## R-02 P1 大文件路径仍依赖整份内存分配

**触发：** 大 Markdown、嵌入大量图片的文档、长扫描 PDF。当前工作区移除了文本 64 MiB 上限，并把引擎 stdout 改为临时文件接收，但下游仍物化完整文档。

**定位：**

- [text_document.rs](../src-tauri/src/parse/text_document.rs) 第 25、32、105 行：整文件读取，然后生成完整 Markdown 和分段集合。
- [引擎 main.rs](../tools/casy-doc-engine/src/main.rs) 第 948、964 行及 [source_map.rs](../tools/casy-doc-engine/src/source_map.rs)：识别返回全部页，再构建完整 Markdown/来源映射。
- [document_pipeline.rs](../src-tauri/src/document_pipeline.rs) 第 541 行：已有 `result.pages` 时再读出完整 `disk_pages` 比较，期间两份页数据同时存在。
- [conversion.rs](../src-tauri/src/commands/conversion.rs) 第 95、127 行：导出再次读入完整 Markdown/PDF；图片解码和替换还会产生额外分配。

**影响：** 峰值内存随文档正文、页数和图片量增加；资源耗尽可能导致进程退出。逐页渲染不等于全部处理阶段都是有界内存，单页也可能含超大图像或复杂对象。本次没有 OOM 实测，不能给出已经验证的最大文件容量。

**建议与验收：** 引入逐页落盘与增量校验，减少完整页集合重复读取；PDF 导出流式复制，图片尽早外置；对无法增量处理的解析器采用明确资源预算和隔离策略。不要用恢复统一 64 MiB 限制代替处理链路改进。按计划记录真实样本峰值 RSS、临时磁盘、吞吐、取消与失败结果。

## R-03 P2 Markdown 相对图片跨目录转换会失去引用目标

**触发：** `A/note.md` 包含 `![x](images/a.png)`，转换到 B 目录。现有 0.1.1 导出的 Markdown 若带 `casy-images-…` 相对引用，再单独转换到另一目录，也存在同一问题。

**定位：** [conversion.rs](../src-tauri/src/commands/conversion.rs) 第 94–101 行只向图片导出助手提供 Markdown 和目标目录；[markdown_export.rs](../src-tauri/src/commands/markdown_export.rs) 第 12 行只匹配 data:image/base64，第 50 行保留其余文字。没有传递原 Markdown 基准目录或迁移已有相对资源。

**影响：** 命令可报告成功，但输出 Markdown 引用 B 中不存在的图片。现有图片外置测试覆盖 base64，不能证明已有附件可迁移。

**建议与验收：** 增加源目录感知的附件解析、复制和引用重写；明确缺失图片处理，保护源文件并检查路径。覆盖跨目录、中文/空格、同名冲突、重复图片及缺失附件；远程图片不默认下载。此发现限定于独立转换路径，其他导出入口需另作覆盖核对。

## R-04 P2 文字文档允许选择不支持的双层 PDF 输出

**触发：** 在独立文件转换中导入 TXT/MD/DOCX 等，选择“双层 PDF”或“两者都要”。

**定位：** [FileConversionDialog.vue](../src/shared/components/FileConversionDialog.vue) 第 24、92 行接受文字格式并显示全部输出选项；[text_document.rs](../src-tauri/src/parse/text_document.rs) 第 124 行固定 `searchable_pdf_path: None`；[conversion.rs](../src-tauri/src/commands/conversion.rs) 第 126 行要求该路径存在。

**影响：** PDF 输出必然失败；选择“两者”时第 94 行起已先保存 Markdown，随后失败，形成“界面失败但已有部分文件”。编辑器另有 PDF 导出能力，并不会自动被此路径调用。

**建议与验收：** 前后端共享输入/输出能力矩阵，在写文件之前拒绝不支持的组合；若扩展文字 PDF，显式接入相应渲染器。定义多个输出的提交与失败语义，验证不会在无说明的失败后留下被误认作完整结果的文件。

## R-05 P1 自定义期限规则与审计写入缺少统一事务

**触发：** 新建、修改或启停自定义期限规则，规则 SQL 成功后，审计或后续求值遇到数据库错误。

**定位：** [deadline_rules.rs](../src-tauri/src/commands/deadline_rules.rs) 第 135–194、205–213 行使用普通连接依次写规则、写审计、重新求值。删除路径第 222–232 行已将删除与审计放进事务，但提交后的求值错误仍会作为整个命令失败返回。

**影响：** 规则可能已经生效，调用方却收到失败；审计写入失败时缺失相应变更记录，重试新建还可能重复生成规则。对于法律期限配置，这会削弱变更可追溯性。

**边界：** [recalc.rs](../src-tauri/src/deadline/recalc.rs) 的重算是即时求值与统计，不持久化新期限缓存，因此本发现不是“物化期限未更新”。新程序事件使用独立事务机制，不应与旧自定义规则混为一谈。

**建议与验收：** 规则变更与审计统一事务；区分持久化成功与提交后求值/刷新失败。用审计插入失败、求值失败和重试用例验证：失败不留下半条变更，成功变更不被误报为未提交，审计完整对应。

## R-06 P2 其他长写操作仍可能前端超时而后台继续

**触发：** 大备份、恢复、导入/导出或同步超过前端等待期限。

**定位：** [tauriBridge.ts](../src/core/tauriBridge.ts) 第 21 行只豁免独立转换的默认超时；第 22 行其他命令仍按名称分配 180 秒或 60 秒，第 25 行 `Promise.race` 超时不会取消后端 invoke。例如 [portable_backup.rs](../src-tauri/src/commands/portable_backup.rs) 第 574 行起的完整备份/恢复仍可能长时间执行。

**影响：** 前端无法直接获知最终结果，用户重复操作可能和仍在执行的任务重叠。错误文案已提示刷新核对和不要重复提交，降低了误导，但没有提供完整任务状态契约。本次未对备份实测超时；用户观察到的是旧版转换超时后文件生成。

**建议与验收：** 长任务统一使用 job ID、持久状态、最终结果和输出清单；前端等待结束只表示连接等待状态，不能代替业务终态。补齐取消、重连、幂等重试以及重启后的可恢复/不可恢复区分。

## R-07 P2 WebDAV 状态接口固定返回未连接

**触发：** 配置或完成 WebDAV 同步后查询统一同步状态。

**定位：** [sync/mod.rs](../src-tauri/src/sync/mod.rs) 第 32–41 行固定返回连接 false、空 URL、无上次同步时间、版本 0、无待同步变化；[commands/sync.rs](../src-tauri/src/commands/sync.rs) 暴露此结果。

**影响：** 状态消费者无法反映真实配置和最近同步活动；不能将此接口的“未连接”视为服务未配置或不可达的证据。实际同步代码存在，这不是整个 WebDAV 功能都未实现。

**建议与验收：** 从配置及活动记录读取状态，区分已配置、连通性检测、正在同步、上次结果及待上传变更。响应不携带密码。覆盖无配置、成功、认证失败、离线和同步中状态。

## R-08 P2 发布 CI 缺少包内运行时与安装镜像验收

**触发：** tag 触发发布任务，构建返回成功即上传草稿 DMG。

**定位：** [ci.yml](../.github/workflows/ci.yml) 的 tauri-build 只构建并上传，没有调用 [verify-bundle.mjs](../scripts/verify-bundle.mjs)，也没有 DMG 挂载、引擎真实 OCR、独立 engine crate 测试步骤。顶部“三平台”注释与实际 Rust 两平台、打包仅 macOS 的矩阵不一致。

**影响：** CI 不能仅凭构建成功保证包内模型、动态库、许可、最低系统版本和实际识别能力都正确。0.1.1 的 Tauri DMG 脚本曾失败，最终本地用 hdiutil 完成镜像与核验；这不是自动发布流程已经稳定的证据。

**建议与验收：** 把资源验证、实际包内引擎样本、镜像挂载、版本/架构/签名检查和校验和清单纳入发布门禁。保存源码与锁文件标识，清除开发环境覆盖后测试。签名、公证、自动更新需独立验收；现有 signing README 提及的 Windows 脚本实际不存在，本次已纠正文档。

## 验证证据与未覆盖项

本次重新执行：

```bash
npm run test:unit
node --test scripts/binary-architecture.test.mjs scripts/license-policy.test.mjs scripts/runtime-components.test.mjs
```

- [前端日志](Archive/2026-09-22/review-evidence/frontend-tests.log)：56 文件中 55 通过、1 失败；270 项中 269 通过、1 失败。
- [脚本日志](Archive/2026-09-22/review-evidence/script-tests.log)：5 项全部通过。
- 先前定向 Rust/引擎测试及 0.1.1 交付证据见 [项目状态](../Casy-STATUS.md)，不能算当前最终源码全量回归。

其他待验证事项：工作区全文 16 MiB、编辑器导出 40 MiB、OCR 校订 128 MiB 的能力边界；截图、自动监听、转写和部分批处理的占位接口；正式隐私/许可文本与实现一致性。它们已进入计划，不以占位注释直接推断所有相关功能都不可用。
