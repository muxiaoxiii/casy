# 审计 04 · 文档管线（转换 / OCR / 导出 / 卷宗文件域）

审计日期：2026-09-30。范围：`document_pipeline.rs`、`commands/{document_intelligence,conversion,processing,files,markdown_export}.rs`、`workspace_sync.rs`、`background_jobs.rs`、`processing.rs`、`parse/text_document.rs`、`docsy_engine/**`、`tools/casy-doc-engine/src/**`、`src/modules/files/**`、`src/shared/components/FileConversionDialog.vue`。

方法：逐行读码 + 全链路追踪（源文件 → 引擎 stdout → 磁盘产物 → SQLite → 前端）。所有结论标注 `CONFIRMED`（代码路径可完整走通并给出触发条件）/ `SUSPECTED`（依赖运行时条件，未实机复现）。

---

## 概述

转换链路本身写得比一般项目严谨：`validate_result` 做了源哈希二次校验、产物目录包含性校验、页 IR 逐页落盘比对、Markdown 哈希重建比对、来源映射全等比对，PDF 头魔数校验；多产物发布用了 prepare-then-commit + `Drop` 回滚；`progress.json` 引擎侧用 `write pending → rename` 原子替换。这些是真的，不是文档吹的。

**但真正的问题不在校验层，而在三个地方：**

1. **停滞保护的计时窗口和最重的工作阶段完全重叠。** 引擎进入 `finalizing` 后不再写任何进度，而 `finalizing` 恰好包含「用 lopdf 重新栅格化整份 PDF 并重写」——耗时最长的单步。父进程的 15 分钟保护在这一段是硬上限，与文档规模无关。**这是大文件"跑着跑着就死了"的头号原因。**

2. **产物目录只写不清，且每次尝试都写一份完整可搜索 PDF。** `document-artifacts/<job_id>/<sha>/` 在整个代码库里**没有任何一处 `remove_dir_all`**。失败、取消、重试、校订留下的目录永久驻留。

3. **Markdown 里内嵌 base64 裁图，而这条链路只修了独立转换，没修卷宗 OCR。** 引擎 `visual.rs` 把每张图/表/公式/印章按原始分辨率 PNG base64 塞进 `page.markdown`；卷宗 OCR 路径（`background_jobs::persist_success`）把这份带 base64 的 markdown 原样写进 SQLite 和 `case_files.ocr_markdown_path`；而 `get_workspace_document` 对该文件有 16 MiB 硬上限。**结果是：正文只有几百 KB 的文档，只要图多，正文视图就永久打不开。**

另外，Codex 那句"no problems"漏掉的最要紧一条：**一次失败的重试会摧毁原本可用的正文视图**（最新 job 非 completed 时不回退到上一个 completed job）。

---

## 文档声称 vs 代码实测对照表

| 文档声称 | 代码实测 | 判定 |
| --- | --- | --- |
| 「转换重启后明确中断」 | `background_jobs.rs:214-218` 启动时把 `document_processing_jobs.status='running'` 置为 `failed/INTERRUPTED`，`processing.rs:20` 把 `processing_activities` 置 failed。**但两条 UPDATE 都是 `let _ =`，无日志无重试。** | 基本成立，但失败静默（见 M1） |
| 「尚无断点续跑」 | 确认无 resume 路径。产物目录完整保留（未被清理），但没有任何代码读取半成品目录。 | 成立 |
| 「后端保留 15 分钟无页数或阶段变化的停滞保护；不是整个文件只能处理 15 分钟」 | `document_pipeline.rs:368` 硬编码 900s。计时只在 `current_page` **严格递增**（:351）或 phase 字符串**变化**（:358）时刷新。引擎 `main.rs:952` 写完 `finalizing` 后，直到 `:1000` 才写 `completed`——**中间 48 行包含 `add_search_layer` 整份 PDF 重建，全程零进度**。 | **不成立（见 S1）** |
| 「文字解析需等当前阻塞解析返回后确认取消」 | `document_pipeline.rs:273-281`：文字路径**完全不进 `execute_engine`**，因此既无停滞保护、无 `check_conversion_cancelled`、无任何 `emit_progress_snapshot`。 | 成立但文档未说明文字路径无停滞保护 |
| 「多产物预备后提交，失败回滚新文件」 | `conversion.rs:135-144` + `PublishedOutputs::drop`（:161-167）。对 Rust 错误路径成立。**SIGKILL/断电时 `NamedTempFile`/`TempDir` 的 Drop 不执行** → 输出目录留下 `tmpXXXXXX` 和无主的 `casy-images-XXXX/`。 | 部分成立（见 M2） |
| 「已有同名输出不会覆盖」 | 转换路径成立（`persist_noclobber` + 序号后缀）。**DOCX 导出不成立**：`docsy_engine/export.rs:187-188` 用秒级时间戳命名 + `:48 fs::write` 直接覆盖。 | **不成立（见 M3）** |
| 「Markdown、页 IR、来源映射和数据库记录必须关联同一源文件哈希」 | 源文件哈希链完整。**但 `source_map.markdown_sha256` 算的是引擎原始 markdown（含 base64）；独立转换发布的 markdown 已被 `externalize_images_from` 改写。**目前不冲突纯属巧合——来源映射不随独立转换落库。 | 脆弱（见 M4） |
| 「Markdown 图片按内容哈希去重」 | `markdown_export.rs:39/95` 用图片字节 SHA-256 命名，正确。**但目录名是随机 TempDir 名（`casy-images-XXXXXX`），且系统里没有任何 DB 记录把该目录绑定到源文件。**删了 .md 目录即永久失去关联。 | 成立但无溯源（见 M5） |
| 「工作区全文读取 16 MiB，超限建议分页对照」 | `workspace_sync.rs:85` 硬上限。但这个上限实际限制的是 **base64 裁图体积，不是正文长度**；且 `:87-91` 同一次调用把**全部页的 `plain_text` + `regions_json` 无上限载入内存**。 | **上限语义与文档不符（见 S2）** |
| 「来源校订沿用绑定关系，不能用旧结果冒充当前原件」 | `load_document_page:357`、`get_workspace_document:84`、`queue_file:80/161` 都重新哈希原件。这条链是真的。 | 成立 |
| 「`get_workspace_document` 只返回 completed job」 | `:83` 确实带 `j.status='completed'`，**但不回退到上一个 completed job**。 | 过滤成立，回退缺失（见 S3） |
| 「非全链路有界内存」 | 承认。但实测峰值比文档描述更高：引擎 `Vec<Page>`（含全量 base64）+ 父进程 `serde_json::from_reader` 再物化一份 + `validate_result:561` 重建来源映射时再分配一份全文 `text` + `conversion.rs:116` 再整读一遍 markdown。**同一份数据同时存在 3-4 份。** | 成立但低估（S4） |
| 「100/500 页资源压力未验证」 | 确认无覆盖。`src-tauri/tests/` 无任何停滞保护 / finalizing / 崩溃恢复 / 产物清理用例。 | 成立（测试缺口） |

---

## 严重问题

### S1. `finalizing` 阶段被 15 分钟停滞保护硬杀 —— 大文件必然失败且原因难懂 【CONFIRMED】

- **位置**：`tools/casy-doc-engine/src/main.rs:952` → `:1000`；`src-tauri/src/document_pipeline.rs:351,358,368`
- **缺陷**：引擎在 `:952` 写一次 `finalizing`（`currentPage = total`），此后直到 `:1000` 写 `completed` 之间**不再调用 `write_progress`**。父进程刷新停滞计时的两个条件是「`current_page` 严格递增」和「phase 字符串变化」，两者在这段都恒为 false，因此 `last_progress` 从进入 finalizing 起 900 秒后到点，返回 `DOC_ENGINE_TIMEOUT` 并 `kill_on_drop` 杀掉引擎。
- **这段在做什么**（`main.rs:957-963` → `add_search_layer`，`:873-911`）：
  - `Document::from_file(source)` —— **整份原始 PDF 载入内存**；
  - **对每一页调用一次 `render_page`（外部渲染子进程）**，`std::fs::read(raster)` 把整页 PNG 读进内存；
  - 逐页 `add_image` + `add_invisible_text_runs`；
  - `doc.save(output)` 重写整份可搜索 PDF。
  随后 `:964-966` 写全量页 IR 和来源映射。
- **触发条件**：任何总耗时超过 15 分钟的文档转换，与页数无关。典型：300+ 页扫描卷宗，或 100 页以内但含大量复杂矢量/巨幅图片的 PDF（单页栅格化几分钟）。前端此时进度条卡在 99%（`FileConversionDialog.vue:115` 的 `Math.min(99, ...)`），文案停在「正在整理结果」，只显示一个不再跳动的「已用 X 分」。
- **可观察错误结果**：任务报 `DOC_ENGINE_TIMEOUT: 当前阶段超过 15 分钟没有页数或阶段进展，已终止任务`。用户看到的是"卡住 15 分钟然后失败"，而实际上 99% 的工作已经做完，只差最后一次写文件。**这是"生产实验跑不动"最可能的直接原因。**
- **附带**：即使放宽超时，`add_search_layer` 逐页 fork 渲染子进程 + 全 PDF 常驻内存，在大文件上是 O(总页数 × 单页渲染耗时) 的串行累加，本身就是最慢的一段。

### S2. OCR 裁图 base64 直接进 SQLite，16 MiB 上限变成"配图数量上限" 【CONFIRMED】

- **位置**：`tools/casy-doc-engine/src/visual.rs:57-60`；`main.rs:544,552-556,965`；`src-tauri/src/background_jobs.rs:60,62`；`src-tauri/src/workspace_sync.rs:85,87-91`
- **缺陷**：`Visual::html` 把每张裁图按**原始分辨率**（无降采样）编码为 base64 PNG 内嵌进 `page.markdown`。这份 markdown：
  1. 累积进引擎的 `Vec<Page>`（`main.rs:375`），全文档常驻；
  2. 随 `ProcessResult` 序列化到 stdout，父进程 `document_pipeline.rs:382` **再物化一整份**；
  3. `background_jobs.rs:60` 把 `page.markdown`（含 base64）**逐页 INSERT 进 `document_pages`**；
  4. `background_jobs.rs:62,63` 把它作为 `case_files.ocr_markdown_path` 指向的 `source.md`。
  而 `workspace_sync.rs:85` 对该文件做 16 MiB 硬上限判断。
- **触发条件**：60 页卷宗、平均每页 2 个图/公式/印章裁图、单张 PNG 约 400 KB → `source.md` ≈ 60 × 2 × 400 KB × 1.33 ≈ **64 MB**，其中真正文字不到 200 KB。
- **可观察错误结果**：知识库「卷宗正文」面板永久显示「正文超过 16 MiB，请使用分页对照查看」。**分页对照能看，但全文视图/连续性检测（`continuations`）/检索定位全部不可用。**同时数据库体积被撑大 1.33 倍的图，每一份加密备份都要带上它。
- **对比**：独立转换路径**做了**外置（`conversion.rs:117` 调 `externalize_images_from`），所以"独立转换能用、卷宗 OCR 不能用"——这个不一致本身就是用户报告"有的功能能跑有的不能"的来源。
- **附带内存问题**：同一函数 `:87-91` 在 16 MiB 检查**通过之后**，把该 job 全部页的 `plain_text` + `regions_json` 无上限载入内存。`regions_json` 不计入 16 MiB，所以一份"正文 1 MB 但 3000 个区域"的文档可以绕过上限，然后在内存里炸。

### S3. 一次失败的重试/校订会摧毁原本可用的正文视图，且无法回退 【CONFIRMED】

- **位置**：`src-tauri/src/workspace_sync.rs:83`；`src/modules/knowledge/components/WorkspaceDocumentPreview.vue:19-23,58`；`src-tauri/src/commands/document_intelligence.rs:248`
- **缺陷**：`get_workspace_document` 的 join 是
  ```sql
  JOIN document_processing_jobs j ON j.id = (SELECT id FROM document_processing_jobs
                                            WHERE file_id=f.id ORDER BY rowid DESC LIMIT 1)
  WHERE ... AND j.status='completed'
  ```
  取**最新** job 且要求 completed，**不回退到上一个 completed job**。`list_workspace_sources:67` 同样取最新。前端 `:23` `if (status !== 'completed') return` → 直接显示「正文提取失败」+「提取正文」按钮。
- **触发条件**（两条都很常见）：
  1. 正文已成功提取 → 用户在原文对照里**校订一个区域**（`correct_document_region`）→ 该命令在 `document_intelligence.rs:248` **新建一个 job 行**（`status='running'`, `engine='paddle-onnx-corrected'`）→ 若引擎 OOM / `build_page_index_tree` 失败 / 应用被关 → 该 job 停在 failed。**原本完好的正文从此消失。**
  2. 误点「重新识别」→ 新 job 失败（源文件被同步工具碰过、磁盘满、引擎超时）→ 同上。已有的正确结果仍在 `document_pages` 里躺着，但 UI 永远看不到。
- **可观察错误结果**：用户必须**重新 OCR 整份文件**才能拿回自己已经有的正文。而且错误信息是裸的 `Query returned no rows`（rusqlite 未处理），不是中文业务提示。
- **附带**：这条路径也解释了"为什么重新处理之后文件反而打不开了"这一类难以复现的投诉。

### S4. `document-artifacts/` 只写不清，每次尝试都留一份完整栅格化 PDF 【CONFIRMED】

- **位置**：`document_pipeline.rs:243-261`（`artifact_dir`）；`background_jobs.rs:104`；`main.rs:957-963,489,533,562`
- **缺陷**：`artifact_dir` 建 `document-artifacts/<job_id>/<sha256>/`。全库 grep `remove_dir_all` 只命中 `vector_index.rs` 和 `portable_backup.rs`，**产物目录没有任何回收路径**。而每次尝试都会往里写：
  - `source.searchable.pdf` —— 一份**整份重新栅格化的 PDF**，体积约等于原件扫描件；
  - `source.document.json` + `source.md` + `source.map.json`（都含全量 base64 裁图）；
  - `page-N.ocr-lines.json`、`page-N.layout.json` —— 每页两个调试文件，500 页就是 1000 个；
  - `timings.json` —— **每页整体重写一次**（`main.rs:562`），O(n²) 写入。
  `retry_job`（`document_intelligence.rs:171`）每次都生成**新 job id** → 新目录，旧目录永久孤立。
- **触发条件**：正常生产使用。100 份卷宗 × 平均 2.5 次尝试 × 每份 80 MB 可搜索 PDF ≈ **20 GB 永久占用，且无任何 UI 提示**。
- **可观察错误结果**：磁盘缓慢涨满 → 一旦写满，引擎 `main.rs:266` 的 `write_progress` 失败直接让整个 job 中止，父进程只看到 `DOC_ENGINE_FAILED` + 一条 IO 错误；或者 `spool_engine_output` 报「无法保存 OCR 结果，请检查磁盘空间」——但**已经写完的 79 个产物目录不会回收，问题自我放大**。

### S5. 停滞保护的刷新条件过窄 + 文字路径完全没有保护 【CONFIRMED】

- **位置**：`document_pipeline.rs:349-370`；`document_pipeline.rs:273-281`
- **缺陷 A（窄）**：`if let Ok(bytes) = std::fs::read(progress.json)` 与 `if let Ok(progress) = serde_json::from_slice(...)` **两处都用 `if let Ok` 静默丢弃错误**。引擎侧 `write_progress` 是原子的（`main.rs:266-274` 写 pending 再 rename），所以撕裂读概率低；但只要发生一次（引擎在 rename 前被 kill、磁盘错误、`progress.pending` 与 `progress.json` 跨卷 rename 失败），这一 tick 就**不刷新计时**。连续 900 秒内每次读都失败 → 误杀健康任务，且**没有任何日志**（这段完全没有 tracing，只有 `:369` 成功路径有）。
- **缺陷 B（缺）**：`run_processing:273-281` 对文字文档（md/docx/doc/rtf/odt/txt）走 `spawn_blocking`，**根本不进 `execute_engine`**，因此：无停滞保护、无 `check_conversion_cancelled`、无 `emit_progress_snapshot`。一个卡死的 anydoc 解析会永久占住一个 blocking 线程，UI 停在「正在准备转换 / 0 页」，用户点「取消」也无响应（要等解析自己返回）。**文档只说"文字解析需等当前阻塞解析返回后确认取消"，没说文字路径连超时保护都没有。**

---

## 中等问题

### M1. 状态写入全线静默吞错，永久 spinner 无人收拾 【CONFIRMED】

- `processing.rs:26-34` `fn write` —— 所有 activity 写入失败只 `log::error!`，`Activity::start`（:79）连返回值都不看。若 insert 失败，后续 `finish`/`progress` 的 `WHERE id=?1` 命中 0 行，整个 activity 在 DB 里根本不存在，UI 永远等不到终态。
- `background_jobs.rs:130-141` —— OCR 成功后写 `index_status` 的四条语句**全部 `let _ =`**。DB 一旦忙/锁/损坏，`index_status` 永远停在 `'running'`。而 `commands/processing.rs:42-44` 把 `status='completed' AND index_status='running'` 映射成 UI 的 `running` —— **处理中心里这条任务永远显示"正在索引"，本会话内没有任何代码路径能把它翻成失败**（`processing::recover` 只在下次启动时跑）。
- `background_jobs.rs:214-218` —— 启动时的中断恢复同样是两条 `let _ =`，**失败无日志**。用户重启一次不够，得反复重启。
- `background_jobs.rs:84,86` —— `persist_failure` 里 `case_files` 状态更新和 `tx.commit()` 都吞错。可能出现 job 标 failed 但 `case_files.ocr_status` 还停在 `processing`。

### M2. 预备-提交在 SIGKILL 下留下无主文件 【CONFIRMED】

- `conversion.rs:122`（Markdown）、`:130`（PDF）用 `NamedTempFile::new_in(&destination)`，`markdown_export.rs:36/81` 用 `tempdir_in(destination)`。`PublishedOutputs::drop`（:161-167）和 `TempDir::drop` 只在 Rust 展开/返回错误时执行。
- **触发**：转换途中强杀应用 / 断电。
- **结果**：用户选定的输出目录里留下 `tmp3Xk9a`（可能是半个 200 MB PDF）和一个没有任何 .md 引用的 `casy-images-XXXXXX/` 目录。后续运行不会把它们当作有效产物（`publish_output` 找 `stem.md` / `stem (1).md`），但也不会清理——**用户的输出目录会随着每次崩溃而污染**。
- 文档"失败回滚本次新建文件"对 Rust 错误路径是真的，对 kill 路径不是。

### M3. DOCX 导出：秒级时间戳命名 + 非原子覆盖，与"同名不覆盖"承诺冲突 【CONFIRMED】

- `docsy_engine/export.rs:187-188`：`format!("{template_name}_{YYYYMMDD_HHMMSS}.docx")`。
- `docsy_engine/export.rs:48`：`fs::write(&output, &output_bytes)?` —— 直接写最终路径，**无 `persist_noclobber`、无临时文件、无 `sync_all`**。
- **触发**：同一模板在同一秒内导出两次（双击、脚本、自动化）。第二次**静默覆盖**第一次。
- **可观察错误结果**：用户丢一份文书。对比转换路径（`conversion.rs:168-179` `persist_noclobber` + 序号），这里是明确的回归/遗漏。
- **附带**：写到一半被 kill → 最终路径上一个**截断的 .docx**，Word 打不开，且因为文件名确定，用户在下一秒重导会覆盖它，但更晚重导就会留下两个文件，坏的那个没人提示。

### M4. 模板字段解析失败被降级为"0 字段"，导出成功但全是占位符 【CONFIRMED】

- `docsy_engine/template.rs:127`：`let (fields, description) = extract_template_info(path).unwrap_or_default();`
- `extract_template_info:151`：`if let Ok(mut doc) = archive.by_name("word/document.xml")` —— 取不到就当没有字段。
- `export.rs:124-125`：未提供的字段**保留原始占位符** `caps[0].to_string()`。
- **触发**：模板是加密 docx、Word 2003 XML 改名成 .docx、zip 结构异常、或 `word/document.xml` 不是 UTF-8。
- **可观察错误结果**：模板浏览器里该模板显示为"0 个字段"，看起来正常；填表导出后弹「导出成功」，得到的 .docx 里**原样保留 `{{原告姓名}}`、`{{案号}}`**。典型的"占位符静默 no-op"。

### M5. `casy-images-…` 目录没有任何溯源记录 【CONFIRMED】

- `markdown_export.rs:36` 用 `tempfile::Builder::prefix("casy-images-")` 生成**随机**目录名；文件内以图片字节 SHA-256 命名（`:39`/`:95`，这部分是对的）。
- **缺陷**：**系统里没有任何数据库表或元数据把这个目录绑定到源文件或 markdown 文件**。`conversion.rs:147-154` 返回的 JSON 只有 `outputPath`/`markdownPath`/`pdfPath`。
- **可观察错误结果**：
  - 用户删了 .md 保留目录 → 目录永久占空间，无人回收（叠加 S4）。
  - 用户只拷了 .md → 所有图片变裂图，且**没有任何校验会告诉他**（`reveal_path`/`open_file_with_default` 只看扩展名）。
  - 同一目录被反复导出后无法判断归属。
- 文档已提示"移动/分享必须带上图片文件夹"，但那是把一致性问题转移给了用户；系统侧本可以记录 `image_dir -> (source_sha256, md_sha256, created_at)`。

### M6. 重命名/移动只挡 `running`，不挡 `queued` 【CONFIRMED】

- `files.rs:879`：`... AND j.status='running'`。
- **触发**：给一个已排队（`status='queued'`，worker 还没 claim）的文件改名或移动 → `case_files.file_path` 被更新（`:927-930`），但 `document_processing_jobs.source_path` 是**快照**（`background_jobs.rs:19` claim 时从 `case_files` 读，进程内已持有的 `ClaimedJob.source_path` 更是早就固定了）。
- **结果**：worker 随后 `process_one:93` 对**旧路径**做 `sha256_file` → 文件已不在 → `persist_failure("无法读取…")`。**用户只是改了个文件名，就让一个排队中的 OCR 任务凭空失败。** 若旧路径已被别的文件占用，则是 `SOURCE_CHANGED` 失败。
- 附带：`relocate_files` 不清除 `source_sha256` / `ocr_markdown_path` / `document_ir_path`。改名不改内容所以哈希仍对，**目前侥幸正确**——但这是靠"重命名不改变字节"这个隐含前提成立的，任何未来引入内容改写的重命名都会静默产生指向旧内容的引用。

### M7. `scan_unregistered_files` / `relocate_knowledge_reference_batch` 无上限载入 【CONFIRMED】

- `files.rs:584-617`：递归 walk 整个卷宗根，**没有条数上限**（`walk` 里 `out` 无 guard），结果 `Vec<UnregisteredFile>` 全量返回前端。对比 `list_case_dirs:368` 有 `out.len() >= 2000` 保护、`workspace_sync::scan:115` 有 100,000 保护——**这里漏了**。一个 50 万文件的卷宗会直接把 IPC 返回体撑爆。
- `files.rs:965-976`：**每次重命名/移动都全表 `SELECT id,content FROM knowledge_items` 并在内存里对每条做字符串替换**，而且是在 `relocate_files:939` 的**写事务内、`tx.commit()` 之前**。知识库一大，重命名就会长时间持有 SQLite 写锁，**阻塞全应用所有写操作**（包括 OCR 进度写入 `document_pipeline.rs:362`）。
- `workspace_sync.rs:375` 在同步 tick 的事务里也调它，每 10 秒可能触发一次。

---

## 大文件与内存边界

文档说"仍不是端到端有界内存"，实测下来峰值比文档描述高一倍，因为**同一份页数据同时存在 3-4 份**：

| 阶段 | 位置 | 驻留内容 |
| --- | --- | --- |
| 1. 引擎识别 | `main.rs:375` `Vec<Page>` | 全文档 `markdown`（**含全部 base64 裁图**）+ `plain_text` + `regions` + `layout` |
| 2. 引擎输出 | `main.rs:1041-1045` `write_result` | 整个 `ProcessResult` 序列化 |
| 3. 父进程物化 | `document_pipeline.rs:382` `from_reader` | **第 2 份**（base64 再解一遍进 RAM） |
| 4. 父进程校验 | `document_pipeline.rs:561` `source_map::write_to(..., sink)` | **第 3 份**：函数内 `text: String`（全文拼接）+ `text_spans` + `markdown_spans` 两个 Vec |
| 5. 独立转换发布 | `conversion.rs:116` `read_to_string` + `markdown_export.rs:127` `String::with_capacity(len)` | **第 4 份** × 2 |
| 6. finalizing | `main.rs:874` `Document::from_file` + 每页 `std::fs::read(raster)` | 整份原 PDF + 整页 PNG |

具体读-everything-进-RAM 的点：

1. **`conversion.rs:116`** — `fs::read_to_string(&result.markdown_path)` 把整份 markdown 读进内存，紧接着 `externalize_images_from` 又构造一份等长 `String`。注意 `:96` 刚刚 `drop(result.pages)` 释放了页集合，**然后立刻又把同样的内容从磁盘读回来**。对 500 MB 的 .md，这是 ~1.5 GB 峰值。
2. **`document_pipeline.rs:561`** — 校验阶段重建来源映射。虽然传给 `io::sink()` 不落盘，但 `SourceMap.text`（全文）+ 两个 span Vec 全在内存。对应文档说的"来源文本/坐标仍驻内存"，但**这是校验代码里的驻留，文档没提**。
3. **`document_intelligence.rs:232-235`** — 校订一个区域要把该 job **全部页**的 `plain_text` + `markdown`（含 base64）+ `regions_json` + `layout_json` + `timing_json` 载入内存，**且在 `Immediate` 事务内**（`:224`）。500 页文档 = 持锁 + 数 GB 内存。
4. **`workspace_sync.rs:87-91`** — 前述，16 MiB 只管 markdown 文件，不管 `document_pages`。
5. **`files.rs:965`** — 知识库全表正文。
6. **`main.rs:562`** — `timings.json` 每页整体重写，O(n²)。
7. **重复哈希源文件**：`queue_file:80` → `validate_result:462` → `conversion.rs:107` = 一次转换至少把原件完整读 3 遍；`get_workspace_document:84` 每次打开正文都重算一遍完整原件哈希（500 MB PDF = 每次点开正文多等数秒）；`get_document_page:329` 在**已经花 60 秒渲染完页面图片之后**又把原件哈希重算一遍（`load_document_page:357` 刚刚已经算过）。
8. **无上限的 IPC 负载**：`document_intelligence.rs:322-326` 把单页预览 PNG（上限 16 MiB）编成 base64（约 21 MiB 字符串）走 IPC 返回。翻页即 21 MiB。

**没有任何测试覆盖 100/500 页**（`src-tauri/tests/` 只有 `text_documents_test.rs` 测 64 MiB markdown 和 `schema_v21_document_test.rs`），也没有任何测试覆盖 S1 的停滞窗口、S3 的回退、S4 的清理。

---

## 修复建议（按性价比排序）

### P0 — 一天内可做，直接解锁生产实验

1. **给 `finalizing` 阶段加进度心跳，并把它移出停滞保护窗口。**
   在 `main.rs:957-1000` 之间每处理一页（或每 5 秒）调一次 `write_progress(request, "finalizing", total, total + processed, ...)` —— 更干净的做法是引入 `current_page` 之外的独立 `stage_progress` 字段，父进程 `document_pipeline.rs:351` 改成「`current_page` 递增 **或** `stage_progress` 变化 **或** 阶段变化」都刷新计时。同时把 `:368` 的 900s 改成：阶段切换后给 finalizing 单独的更长预算（如 60 分钟），或改成"心跳静默超时"（引擎每 N 秒必有心跳，心跳断了才杀）——**后者才是正确的语义**，能同时干掉 S1 和 S5-A。
   *收益：S1 消失。这是最值钱的一条。*

2. **`get_workspace_document` 与 `list_workspace_sources` 回退到最近一个 completed job。**
   把 `workspace_sync.rs:83` 和 `:67` 的 `ORDER BY rowid DESC LIMIT 1` 改成 `... AND j.status='completed' ORDER BY rowid DESC LIMIT 1`（并把 `error` 字段改为取最新 job 的，这样用户仍能看到"最新一次失败了"）。同时把 rusqlite 的 `QueryReturnedNoRows` 翻译成中文业务错误。
   *收益：S3 消失。改动 2 行，收益极大。*

3. **`document-artifacts/` 加保留策略 + 失败即清理。**
   最小可行版：`background_jobs::persist_failure` 和 `cancel_job` 里，对应 job 目录在**没有其他 completed job 引用它**时 `remove_dir_all`。完整版：启动时扫描 `document-artifacts/`，删除「对应 job 行不存在」或「job 非 completed 且早于 N 天」的目录。再加一个设置项让用户看到/清理总占用。
   *收益：S4 消失，磁盘不再单调上涨，间接缓解"写满盘 → 任务失败"的级联。*

4. **把 `let _ =` 换成至少 `log::error!`（P0-1 组）。**
   `background_jobs.rs:130,131,140,141,214,218`、`processing.rs:31`、`document_pipeline.rs:349-350`、`workspace_sync.rs:509`。
   *收益：M1 降级为"至少可见"。零风险，纯观测。*

### P1 — 一到三天，堵住数据链断点

5. **卷宗 OCR 路径也做图片外置。**
   在 `background_jobs::persist_success` 之前（或 `run_engine` 返回后）复用 `markdown_export::externalize_images_from`，把裁图落到 `document-artifacts/<job>/<sha>/casy-images-…/`，`case_files.ocr_markdown_path` 指向外置后的 markdown。这样 S2 的 16 MiB 限制回归"正文长度"语义、SQLite 不再被 base64 撑大、备份体积下降。
   *收益：S2 消失，并且让"独立转换"和"卷宗 OCR"两条路径行为一致——直接消除用户最容易感知的"为什么这个能跑那个不能"。*
   *配套*：`document_pages.markdown` 存外置后的相对引用。

6. **所有落盘改原子写。**
   `text_document.rs:140`、`docsy_engine/export.rs:48`、`main.rs:489/533/562` 全部改成 `NamedTempFile` + `sync_all` + `persist`。`export.rs:187` 的秒级时间戳换成 `persist_noclobber` + 序号后缀（照抄 `conversion.rs:168-179`）。
   *收益：M3 消失；M2 的"半个文件"风险减半（SIGKILL 只会留临时文件，且 `tempfile_in` 目录可定期扫）。*

7. **`document_pipeline.rs:349-350` 换成 `match` 并加 `tracing::warn!`；把 `read` 换成 `tokio::fs`。**
   顺便修掉一个隐藏问题：这是 async 上下文里的**同步阻塞文件读**，每 1 秒一次，读的是可能很大的 `progress.json`（含 `pageTiming`）。改成 `tokio::fs::read` 并限制只读前 64 KiB。
   *收益：S5-A 可观测 + 不阻塞 runtime。*

8. **文字路径补上超时与取消检查。**
   `document_pipeline.rs:273-281` 的 `spawn_blocking` 外面包一层 `tokio::time::timeout`（如 10 分钟），并在 `segment_markdown` 之前调一次 `check_conversion_cancelled`；`text_document::process` 每 N 个分段 emit 一次进度。
   *收益：S5-B 消失，文字转换不再"0 页卡死"。*

### P2 — 一周内，结构性

9. **给 `casy-images-…` 建溯源记录。** 新增表 `export_image_sets(dir, source_sha256, markdown_sha256, created_at, file_count)`，`conversion.rs:143` 处写入，导出面板显示占用并支持一键清理。→ M5。
10. **`files.rs:879` 加上 `j.status IN ('queued','running')`。** 一行。→ M6。
11. **`files.rs:584-617` 加条数上限（对齐 `scan` 的 100,000）；`files.rs:965-976` 改成游标逐条 UPDATE，或至少把 `relocate_knowledge_reference_batch` 移到 `tx.commit()` 之后。** → M7。
12. **把 `source_map.markdown_sha256` 的语义显式化。** 在 `ProcessResult` 里区分 `markdown_sha256`（引擎原始）和 `published_markdown_sha256`（外置后），或者在 `externalize_images_from` 返回一个 `rewritten: bool` 让调用方知道来源映射已失效。→ M4，防止未来有人把外置后的 markdown 拿去和来源映射比对。
13. **加测试**（当前为零）：① finalizing 阶段 >15 分钟不误杀；② 最新 job failed 时正文回退；③ SIGKILL 后 `document-artifacts/` 与输出目录的残留清单；④ 50 万文件卷宗的 `scan_unregistered_files` 上限；⑤ 模板 zip 异常时不导出"全占位符"docx。

---

## 一句话总结

校验层（哈希链、产物包含性、页 IR 比对）是真的扎实的；**真正让生产实验跑不动的是三件事**：`finalizing` 阶段被 15 分钟停滞保护硬杀（引擎在这段做整份 PDF 重建且不写任何进度）、卷宗 OCR 的 base64 裁图把正文撑过 16 MiB 上限、以及失败的重试不回退到上一个可用 job。**先做 P0 的 1、2、3 三条**——都是小改动、能立刻验证——再动 P1 的图片外置。
