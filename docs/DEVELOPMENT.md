# 开发、验证与交付

核对日期：2026-09-23。当前版本 0.1.3，生产验证版；验收结果见 RELEASE_0.1.3.md。

## 环境

- Node.js 24 与 npm；CI 使用 `npm ci`。以 package-lock.json 为唯一提交的锁文件；不要混用 pnpm node_modules。
- Rust manifest 要求至少 1.95。本机本次使用 rustc 1.96.0。
- macOS 需要 Xcode command line tools；完整运行时准备需要 Homebrew Poppler。
- Zvec 原生库由固定版本脚本准备；`.cargo/config.toml` 指向 `src-tauri/runtime/zvec`。
- Linux/Windows 有平台依赖，但当前没有已验证的完整 OCR 分发包。

```bash
npm ci
node scripts/prepare-zvec.mjs
node scripts/prepare-doc2x.mjs
npm run tauri -- dev
```

`npm run dev` 只是浏览器前端，部分操作走 mock，不会证明桌面命令、数据库或 OCR 可用。

## 隔离资料

debug 构建支持绝对路径 `CASY_TEST_DATA_DIR`；应用还支持显式 `--profile-dir <绝对路径>`。测试不要使用真实资料库。需要保留验收产物时给独立目录并记录路径，结束后再按需清理。

```bash
CASY_TEST_DATA_DIR="$(mktemp -d /private/tmp/casy-review.XXXXXX)" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked
```

运行时可用 `CASY_DOC_ENGINE`、`CASY_PPOCR_MODEL_DIR`、`CASY_KOREAN_MODEL_DIR`、`CASY_PDFTOPPM`、`CASY_OCR_FONT` 覆盖具体工具/资源。完整包应在清除这些覆盖后仍可通过验证。

## 验证分层

```bash
npm run typecheck
npm run test:unit
npm run build
CASY_TEST_DATA_DIR="$(mktemp -d /private/tmp/casy-review.XXXXXX)" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --jobs 2
cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models
node --test scripts/binary-architecture.test.mjs scripts/license-policy.test.mjs scripts/runtime-components.test.mjs
```

`tests/e2e/` 包含 17 个本地脚本，通常需要桥接 example、前端服务、浏览器和隔离资料目录；它们没有被普通 `npm run test:unit` 自动运行。真实模型用例中带 `#[ignore]` 的项目也必须按各文件说明单独运行。

端到端脚本为显式门控：`CASY_E2E=true npm run test:e2e`（需要预编译 `src-tauri/target/debug/examples/*_local_bridge`、`playwright` 模块与隔离 `CASY_QA_DIR`）。CI 中同名步骤同样只在 `CASY_E2E=true` 时运行，默认不执行；门禁未开启时不得把前端单测数字表述为端到端覆盖。

完整回归、真实样本和忽略项见 [验收记录](RELEASE_0.1.3.md)。本机同时运行多个重型编译时建议 `npx vitest run --maxWorkers=1`，避免测试资源竞争。

## 完整包

```bash
npm run release:validation
```

`build:desktop` 使用 `tauri.full.conf.json`，先准备 runtime 再构建前端。`tauri.dmg-only.conf.json` 跳过 beforeBuild，只能在资源和 dist 已明确更新时使用；普通 `tauri build` 不自动包含完整 runtime 资源配置。

`scripts/package-validation.mjs` 使用 app bundle，再验证版本、资源/许可/架构/签名、包内真实 OCR、E5 与 Zvec，最后通过 hdiutil 生成和校验 DMG，同时输出 SHA-256 和构建元数据。已有同名 DMG 不覆盖。`--skip-build` 只用于已确认当前源码构建完成的应用，仍运行全部包内核验。

发布完成标准：

1. 冻结源码和锁文件状态，保留版本与构建证据。
2. 完成相关测试与真实样本验收，记录失败/跳过，不以局部测试代替全套。
3. 准备模型/动态库/字体/许可，生成 manifest 并校验。
4. 构建主应用；检查 Info.plist、架构、签名和实际引擎能力。
5. 生成 DMG，校验镜像与 SHA-256，再挂载核验包内应用。
6. 更新项目状态与发布说明；源码版本和安装包版本一致后才宣布交付。

## CI 实际情况

[ci.yml](../.github/workflows/ci.yml) 的 frontend 跑类型、单测、构建；Rust 矩阵为 Ubuntu/macOS，Windows 已暂停。tag 打包仅 macOS。CI 还声明 Clippy/audit 和 bindings 漂移检查，但本次没有远端运行证据。

0.1.3 已加入引擎/脚本测试，tag 构建调用完整生产验证打包流程并上传证据；本轮未触发远端 CI。既有 Clippy/audit 门禁仍需远端或专门本地运行确认，公证与发行证书未完成。

开发预览的文件监听排除 `src-tauri/`、`outputs/` 和 `release/`，避免原生构建/模型资源及验收产物触发无关监听。前端源码仍保持热更新。并行原生编译时可能放大前端测试等待时间；复核时限制 worker，并记录实际超时和补跑结果，不修改业务断言。

## 日历布局隔离样例

开发服务器启动后访问 `http://127.0.0.1:1420/tests/fixtures/calendar-ui/index.html?mode=mixed&width=720&theme=rice-paper#/calendar?date=2026-09-25&view=timeline`。该页面直接挂载真实日历组件，但注入合成 services，不初始化插件系统，不读取或修改用户数据库，也不持久化主题；甘特计划可在 fixture 内存中保存，重载页面即重置。参数 `mode` 可选 mixed/holidays/empty/gantt/availability，`width` 控制日历容器宽度，`theme` 使用已有主题标识，`view` 可选六种视图。固定的 2026-09 日期仅用于回归，不作为节假日数据来源。该入口不被生产构建引用。

时间线交互回归：`npx vitest run tests/unit/calendarTimeline.test.ts`。视觉检查需同时核对行高、日期/休班标记交叠、超长内容和实际详情字段；jsdom 不验证 CSS 排版。

甘特入口：在上述 URL 中使用 `mode=gantt`、`view=forecast&layout=gantt`。`TaskGantt` 通过 `CalendarService.taskPlans/saveTaskPlan` 调用原生 `list_task_plans/save_task_plan`，入库至 v42 `task_plans`；计划起止与任务 startDate/dueDate 分开，NULL 起止代表取消独立计划，修订号保留。类型由 `cd src-tauri && cargo test export_bindings` 生成，禁止手改 bindings.ts。

定向验证：`npx vitest run tests/unit/taskPlanning.test.ts tests/unit/TaskGantt.test.ts tests/unit/calendarTimeline.test.ts tests/unit/calendarProjection.test.ts`；原生计划校验：`cd src-tauri && cargo test task_plans`，休息提醒：`cargo test rest_day_reminders`。

OCR 引擎验证（需本机 runtime 模型，路径用绝对路径）：

```bash
# 单元 + 算法测试（不需要模型）
cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models
# 真实模型回归（中文扫描 / 韩文 / 多语言含竖排，约 25s）
CASY_OCR_QA_DIR=$(mktemp -d) CASY_PPOCR_MODEL_DIR=$PWD/src-tauri/runtime/models/ppocrv6-medium CASY_KOREAN_MODEL_DIR=$PWD/src-tauri/runtime/models/korean-ppocrv5-mobile CASY_OCR_FONT=$PWD/src-tauri/runtime/fonts/NotoSansCJK-Regular.ttf   cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models -- --ignored real_
# 性能基准（cold/warm 每页耗时 + 峰值 RSS）
cargo build --release --features models --manifest-path tools/casy-doc-engine/Cargo.toml
echo '<ProcessRequest JSON>' | CASY_BENCH_RUNS=3 ./tools/casy-doc-engine/target/release/casy-doc-engine bench
```

文本行方向分类模型 `models/cls/ch_ppocr_mobile_v2.0_cls_infer.onnx`（约 570 KB，PP-OCRv2 mobile cls）随 runtime 分发后被自动发现并启用，也可用 `CASY_TEXT_LINE_ORIENTATION_MODEL` 指定；缺失时退回置信度启发式。完整包分发时需把该模型加入 runtime manifest（prepare 脚本）。

全量 Rust 测试含 MCP/WebDAV 回环监听，沙箱阻止绑定端口时应在获准的本机测试环境运行，不能把该错误归为业务通过。

时段 fixture：`mode=availability`、`date=2026-09-28&view=day`，预置 13:00–18:00 请假，支持表单内存保存与各视图切换。`tests/unit/personalAvailability.test.ts` 覆盖保存、失败草稿与时间范围；`task_plan_chain_test` 使用真正原生命令验证任务全链路及重复实例，`webdav_full_backup_test` 验证计划和休息时段随完整档案恢复。

旧 `.doc`：主路径为 anydoc **0.2.4**（`cargo update -p anydoc` 跟上游）；侧车兜底 `node scripts/prepare-doc2x.mjs` 钉住 `@jitword/doc2docx-<platform>@0.1.0` 并校验 tarball SHA-256，产出 `src-tauri/runtime/bin/doc2x` 与 BSD-3 声明。定向验证：`cargo test --lib parse::doc2docx`、`cargo test --lib text_document`（含 `anydoc_parses_legacy_doc_without_sidecar` 与 `extracts_legacy_doc_when_sidecar_ready`）。方案见 [DOC_IMPORT_PLAN](DOC_IMPORT_PLAN.md)。完整 `runtime:prepare` 会自动调用 prepare-doc2x。
