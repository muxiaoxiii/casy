# Casy 可搜索 PDF、OvisOCR2、PageIndex 与 AI Diff 实施 Walkthrough

日期：2026-09-01  
对应计划：`searchable-pdf-pageindex-ovisocr2-plan-2026-09-01.md`

## 1. 本轮结果

本轮已经把原先互相脱节、部分带演示性质的 OCR 和 PageIndex 代码，改造成一个可安装模型的本地文档处理闭环：

```text
原 PDF（只读 + SHA-256）
  → pdftoppm 页面渲染
  → OAR PP-OCRv5 坐标文字
  → OAR-VL OvisOCR2 页面 Markdown
  → Page IR（逐页文本、Markdown、bbox、置信度）
  → harumi 不可见文字层
  → 独立的 source.searchable.pdf
  → SQLite document_pages
  → PageIndex-inspired 标题树与真实按页读取
```

AI Diff 同时完成了后端安全修复：授权现在绑定工具/实体、实际 payload 和执行前实体状态，不再只是“拿到一个批准令牌就能换参数执行”。

## 2. 原文件保护与衍生件

主程序在排队时计算源 PDF 的 SHA-256，sidecar 开始和结束时再次校验。任何一次不一致都会以 `SOURCE_CHANGED` 失败，不写完成状态。

衍生件写入：

```text
<Casy data>/document-artifacts/<file-id>/<sha256-prefix>/
  source.searchable.pdf
  source.document.json
  source.md
```

`source.searchable.pdf` 从原 PDF 读取后写到新路径。坐标从页面图像的左上角像素坐标换算为 PDF 左下角点坐标，再添加 render mode 3 的不可见文字。原 PDF 不覆盖，因此原件签名和哈希不会因 OCR 处理被改变；衍生 PDF 本身不继承“原件签名有效”的含义。

## 3. 独立 OvisOCR2 客户端

新增 `tools/casy-doc-engine`，与主程序保持进程隔离。这样 OAR/OvisOCR2 所需 Rust 1.95、Candle、Metal 和 ONNX Runtime 不会抬高 Casy 主程序声明的 Rust 1.77.2 下限。

sidecar 提供两个命令：

- `probe`：检查二进制、`pdftoppm`、坐标 OCR 模型、OvisOCR2 模型和 CJK 字体。
- `process`：stdin 接收 JSON，执行页面渲染、双 OCR、PDF 文字层和三类衍生件输出，stdout 只返回机器可读结果，错误写 stderr。

模型版构建：

```bash
cd tools/casy-doc-engine
cargo build --release --features models

# Apple Silicon 推荐
cargo build --release --features metal
```

运行配置：

```bash
export CASY_DOC_ENGINE=/absolute/path/to/casy-doc-engine
export CASY_PPOCR_MODEL_DIR=/absolute/path/to/ppocr-models
export CASY_OVISOCR2_MODEL_DIR=/absolute/path/to/OvisOCR2
export CASY_OCR_FONT=/absolute/path/to/NotoSansCJK-Regular.ttf
export CASY_OCR_DEVICE=metal
```

PP-OCR 模型目录采用明确文件名：

```text
det.onnx
rec.onnx
dict.txt
```

OvisOCR2 目录应是完整 Hugging Face 模型目录，至少包含配置、tokenizer 和 safetensors 权重。Casy 不会在用户不知情时自动下载约 1.7 GB 权重。

## 4. 持久任务与故障恢复

Schema 已升级到 v21：

- `document_processing_jobs` 保存源哈希、状态、引擎、模型、页数、进度、衍生路径和结构化错误。
- `document_pages` 保存每页纯文本、Markdown、区域 JSON、页面尺寸和平均置信度。
- `case_files` 保存当前源哈希和最新衍生件指针。

后台处理采用数据库原子抢占，同一时间只运行一个重模型任务。状态只有：

```text
queued → running → completed
                 ↘ failed
queued → cancelled
```

应用异常退出后，遗留的 `running` 任务会转为 `failed / INTERRUPTED`，文件状态同步修复，不再永远卡在 processing。缺少 sidecar、模型、字体或渲染器时会写入明确错误，不会标记完成。

文件界面现在可以：

- 查看引擎缺失项；
- 创建 PDF 文档处理任务；
- 查看排队、运行、失败或可搜索状态；
- 查看失败原因并重试；
- 直接打开生成的可搜索 PDF。

## 5. PageIndex 接入变化

原实现已移除以下不真实路径：

- “Authentic VectifyAI Port”的不准确声明；
- 无条件按 20 页构造虚假分卷；
- `/demo` 返回虚构案情；
- 模型请求 42–50 页、后端却抽取整份 PDF 开头 3000 字。

新实现明确称为 PageIndex-inspired：

1. 从 OvisOCR2 Markdown 的 `#`–`######` 标题建立层级。
2. 根据下一个同级或更高层标题计算真实页码范围。
3. 没有标题时按真实单页建立叶节点。
4. `read_pages` 校验节点、文件和页码边界，单次最多读取 20 页。
5. 内容从最新已完成 job 的 `document_pages` 精确查询，每页带 `[file ... p...]` 引用标记。
6. 读取预算耗尽时返回证据不足，不生成猜测性答案。

这不是 VectifyAI 官方 PageIndex SDK 的完整移植，但已经建立了“搜索树选择页面 → 读取真实 Page IR → 带页码回答”的可验证接口。

## 6. AI Diff 修复

### 内容绑定

提案 payload 和实际命令 payload 都会：

- 展开 `data` patch；
- 移除 `id`、`origin`、`proposalToken` 等身份/授权元数据；
- 递归排序 JSON object key；
- 计算 SHA-256 并比较。

批准后把 `dueDate` 从一个日期换成另一个日期，后端会以 `Payload mismatch` 拒绝，而且令牌不会被消费，仍可用原批准内容执行。

### 状态绑定

创建提案时，后端自行读取 tasks/cases 等白名单实体的完整数据库行并计算规范哈希，忽略前端提交的 `preStateHash`。执行前再次读取；实体在确认窗口内发生变化时，以 `State hash mismatch` 拒绝。

### 原子性与删除顺序

任务创建、更新、切换、删除和案件写入都在令牌消费与业务写入的同一数据库事务内。案件删除还修复了一个更危险的顺序问题：现在先完成授权和数据库删除，再尝试把关联目录移入系统废纸篓；未授权调用不会先碰文件夹，回收站失败也不再退化成永久删除。

## 7. 实际验证

本轮实际执行并通过：

| 检查 | 结果 |
|---|---|
| `cargo test --lib` | 120 passed；包含真实标题页码范围测试 |
| `cargo test --test ai_gateway_test` | 4 passed；未授权、单次令牌、状态漂移、payload 替换均覆盖 |
| `cargo test --test schema_v20_test` | 7 passed |
| `cargo test --test schema_v21_document_test` | 2 passed；表/列、状态和页码约束覆盖 |
| sidecar `cargo test` | 1 passed；输出路径不同、源哈希不变、衍生 PDF 可提取文本 |
| sidecar `cargo check --features metal` | 通过；OAR、OvisOCR2/Candle、Metal、harumi 实际编译 |
| `npm run typecheck` | 通过 |
| `npm run test:unit -- --run` | 2 files / 9 tests passed |
| `npm run build` | 通过；保留既有大 chunk 和浏览器 externalize 警告 |
| `git diff --check` | 通过 |

第一次尝试了不存在的 `npm run test:run`，npm 正确报 `Missing script`；随后按仓库实际脚本改用 `npm run test:unit -- --run` 并通过。

## 8. 尚未越过的验证边界

以下事项没有在本轮假装完成：

1. 本机没有配置 OvisOCR2 权重、PP-OCRv5 模型目录和指定 CJK 字体，因此没有跑真实中文法律扫描件的端到端推理。
2. 尚未建立中文简繁、英文、日文、旋转页、双栏、印章、手写、表格和公式的标注评测集；“比 Tesseract 更准”目前是方案预期，不是 Casy 自有基准结论。
3. 当前 sidecar 在单次任务内加载模型，任务进度是 queued/running/completed 的粗粒度状态；后续可升级为常驻进程并逐页输出 JSONL 进度，以免重复加载权重。
4. 运行中的任务暂不提供强制取消，只有 queued 可取消；这是为了避免在没有进程登记表时制造“UI 已取消、模型仍写文件”的假状态。
5. PageIndex 当前是本地结构树与 agentic page read，不包含官方 SDK 的全部策略或云端扫描件能力。
6. 没有执行全部 Rust 集成测试全集；本轮执行了主库和与改动直接相关的集成测试。仓库其他外部服务/钥匙串类测试不在本轮结论范围内。

## 9. 下一步验收建议

先准备 30–50 页可合法使用的混合样本，至少包括中文庭审材料、双语合同、表格证据和低清扫描件。安装模型后做三层验收：

1. **PDF 层**：原哈希不变、衍生 PDF 视觉无变化、复制顺序合理、中文搜索命中。
2. **Page IR 层**：bbox、置信度、Markdown 标题/表格/公式与页码一致。
3. **检索层**：对一组有标准答案的问题统计页召回、引用页准确率和回答忠实度，并与 Tesseract、纯 OvisOCR2、纯电子文本提取分别对照。

完成这一步后，才适合决定默认模型、低置信度阈值、是否逐字框，以及常驻模型服务的内存策略。
