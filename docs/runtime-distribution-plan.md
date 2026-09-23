# 运行时分发与后续拆包

代码核对日期：2026-09-23。当前方案为完整离线包，没有启用首次启动下载模型。0.1.3 为生产验证版，详见[项目状态](../Casy-STATUS.md)。

## 当前交付边界

已验证安装包平台为 Apple Silicon / macOS 26+；[tauri.full.conf.json](../src-tauri/tauri.full.conf.json) 设置最低系统 26.0、ad-hoc 签名和 runtime 资源目录。没有完成 Developer ID 公证。不能把 Rust 的 Linux/macOS 测试矩阵等同于三平台安装包可用。

完整包包括文档引擎、Poppler、OCR/版面模型、E5 embedding、Noto 字体、Zvec 运行库、旧 Word `.doc` 侧车 `doc2x` 及对应许可证和需要随附的源码。推理不依赖用户安装 Python、Node 或另启 Python 服务；开发准备阶段仍需要工具链与网络下载。

| 组件 | 当前用途 |
| --- | --- |
| PP-OCRv6 medium | 主检测与识别 |
| Korean PP-OCRv5 mobile | 独立韩文候选识别，配套字典 |
| PP-DocLayout Plus-L | 版面检测，配合表格/裁图逻辑 |
| multilingual E5-base INT8 | 768 维本地 embedding |
| Poppler / casy-doc-engine | PDF 渲染与文档处理 |
| doc2x（b2xtranslator 派生） | 旧 `.doc → .docx` 侧车，再进 anydoc |
| Noto 字体 | 多语言派生 PDF 文字层等 |
| Zvec | FP16 HNSW 向量索引 |
| 第三方声明与匹配源码 | 依赖许可交付 |

## 清单与验证

[runtime-components.mjs](../scripts/runtime-components.mjs) 定义分组；[prepare-runtime.mjs](../scripts/prepare-runtime.mjs) 准备资源并生成 `runtime/manifest.json`。其 `schemaVersion=2`、`delivery=full`，components 全部标记 bundled，每个组件记录文件列表/大小，files 记录逐文件 SHA-256。

[verify-bundle.mjs](../scripts/verify-bundle.mjs) 校验签名、平台架构、文件长度/哈希、组件覆盖无重复、模型存在、动态库路径、声明及所需源码；清除 CASY 环境覆盖后执行模型探测、embedding 和 Zvec 验证。`smoke-bundle.mjs` 另行对包内引擎运行真实图文 OCR 与 searchable PDF 样本；`package-validation.mjs` 串联这两项及 DMG/SHA 校验，原生界面仍需单独验收。

2026-09-22 的 0.1.3 runtime 清单含 2181 个文件；下表为未压缩字节数，不等于 DMG 大小。

| 组件 ID | 字节 |
| --- | ---: |
| compliance | 327,897,531 |
| document-runtime | 78,217,232 |
| embedding-e5-base | 295,266,822 |
| fonts | 52,582,333 |
| layout | 129,738,167 |
| ocr-korean-ppocrv5-mobile | 13,514,826 |
| ocr-ppocrv6-medium | 138,739,282 |
| vector-runtime | 23,045,329 |

更新资源后应重新读取清单，不沿用历史“模型约 139 MB”“整包小于 200 MB”或旧组件体积。

## 构建缓存与许可

SDK 下载归档放在 `target/runtime-cache`；运行时保留库、声明和固定版本构建信息。依赖声明递归保留。许可策略允许的宽松许可包源码进入构建缓存，清单记录版本、来源与哈希；需要随附的 copyleft/未知许可或缺少独立声明的依赖源码继续随包。

相关代码：[prepare-notices.mjs](../scripts/prepare-notices.mjs)、[license-policy.mjs](../scripts/license-policy.mjs)、[prepare-zvec.mjs](../scripts/prepare-zvec.mjs)、[prepare-doc2x.mjs](../scripts/prepare-doc2x.mjs)。Poppler 包含 GPL 组件，不能笼统宣称产品没有 GPL 依赖；机械校验不能替代许可审定。`doc2x` 为 BSD-3-Clause（b2xtranslator 派生），声明与 provenance 随 `runtime/licenses/` 分发。

旧合规原文迁至 [Archive](Archive/2026-09-22/compliance/)，`docs/compliance` 保留兼容符号链接，构建读取的 LICENSES/MinerU/E5 文本未改写。隐私和密钥实际边界见[数据与安全](DATA_AND_SECURITY.md)。

## 未来双包方案（未启用）

候选划分为应用包和离线资源包。组件清单可用来分组，尚未提供资源安装/更新协议。启用前至少完成：

1. 应用、引擎、模型和索引版本的兼容契约及资源查找顺序。
2. 下载/离线导入校验、原子安装、失败回滚与升级签名。
3. 缺失/损坏/不兼容资源的能力提示，首次离线安装验证。
4. 卸载/升级不会误删用户文档，旧版本资源可回退。
5. E5 模型标识、向量维度和索引版本绑定，模型改变后显式重建，禁止混写向量。

当前不切换模型精度，不以删去许可或必要源码作为减包方案。发布 CI 缺口见[审阅 R-08](CODE_REVIEW_2026-09-22.md)；实施顺序见[更新计划](UPDATE_PLAN.md)。完整构建命令见[开发与验证](DEVELOPMENT.md)。
