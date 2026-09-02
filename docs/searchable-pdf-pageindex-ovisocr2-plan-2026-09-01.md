# Casy 可搜索 PDF、OvisOCR2 与 PageIndex 实施计划

日期：2026-09-01  
状态：实施基线（Implementation Plan）

## 1. 目标

把 Casy 现有“提取一点文本并标记 OCR 完成”的流程，改造成可追溯的本地文档处理流水线：

1. 原始 PDF 永远只读，先计算 SHA-256，任何衍生处理都不得覆盖原件。
2. 对扫描页生成带坐标的 OCR 文本层，并输出独立的 `*.searchable.pdf`。
3. 使用 OvisOCR2 生成保留标题、表格、公式和阅读顺序的 Markdown 页面内容。
4. 将坐标 OCR 与 OvisOCR2 结果合并成统一 Page IR，作为搜索、引用和 PageIndex 的事实来源。
5. PageIndex 只基于真实页面内容和标题层级建树，按页读取真实文本；移除固定 20 页切块和演示数据冒充真实检索的路径。
6. 后台任务具备排队、运行、进度、失败原因、重试和取消状态，模型或渲染器缺失时明确失败，不伪装完成。
7. 修复 AI Diff 授权：批准的工具、实体、变更内容和执行前实体状态必须与实际写入完全一致，一次性令牌才可消费。

## 2. 不在本轮伪装完成的事项

- 不自动下载约 1.7 GB 的 OvisOCR2 权重；应用只负责探测、配置和调用本地模型目录。
- 不宣称完整移植 VectifyAI PageIndex 云端/官方实现。本轮实现为 PageIndex-inspired 的本地结构树与按页推理读取。
- 没有真实模型权重时，不把编译通过写成 OCR 精度验证通过。
- 不修改有签名的原 PDF；可搜索 PDF 始终是衍生件。

## 3. 架构与数据流

```text
原 PDF（只读）
  ├─ SHA-256 与元数据快照
  ├─ 电子文本快速提取
  └─ 页面渲染
       ├─ 坐标 OCR（PP-OCRv5 / OAR）→ bbox、文本、置信度
       └─ OvisOCR2（OAR-VL）          → Markdown、表格、公式、阅读顺序
                 │
                 ▼
          Page IR（逐页 JSON）
            ├─ searchable.pdf（不可见文字层）
            ├─ document.md
            ├─ SQLite document_pages
            └─ PageIndex-inspired 标题树
                         │
                         ▼
                  按节点、按页检索与引用
```

## 4. 衍生文件布局

每个源文件使用独立目录，目录名包含文件 ID 和源文件哈希前缀：

```text
<Casy data>/document-artifacts/<file-id>/<sha256-prefix>/
  source.searchable.pdf
  source.document.json
  source.md
```

数据库记录完整源哈希、引擎/模型版本、文件路径和错误。再次处理时若源哈希一致可复用；哈希变化必须新建任务和衍生目录。

## 5. 数据库升级（Schema v21）

### `document_processing_jobs`

- 文件、源哈希、状态、引擎、模型版本
- 当前页/总页数/进度
- searchable PDF、Page IR、Markdown 路径
- 结构化错误码和错误信息
- 创建、开始、完成、更新时间

### `document_pages`

- job/file/page 唯一键
- 页面尺寸、纯文本、Markdown、区域 JSON、平均置信度
- PageIndex 和按页读取只从该表读取已落库内容

### `case_files` 扩展

- `source_sha256`
- `searchable_pdf_path`
- `document_ir_path`
- `ocr_markdown_path`
- `ocr_engine`
- `ocr_error`

## 6. 本地 OCR 客户端

新建独立 Rust sidecar `tools/casy-doc-engine`，避免 OAR 新工具链和推理依赖污染 Casy 主程序的最低 Rust 版本。

协议采用逐行 JSON：

- `probe`：返回版本、渲染器、坐标模型、OvisOCR2 模型和字体状态。
- `process`：输入源路径、输出目录、模型目录、语言与设备偏好；持续输出进度事件，最后输出结果清单。
- `cancel`：主程序终止对应子进程并将任务落为 cancelled。

模型特性独立编译：CPU 为基础目标，Apple Silicon 可开启 Metal。PDF 的不可见文字层由纯 Rust PDF 写入库生成；中文字体路径显式配置，禁止静默退回不含中文字形的字体。

## 7. PageIndex 接入原则

1. 优先从 Ovis Markdown 标题识别层级，并使用标题首次出现页作为范围起点。
2. 无标题时逐页建立叶节点，而不是构造虚假的固定分卷。
3. 节点保存页面摘要/内容片段和真实页码范围。
4. `read_pages` 必须验证请求范围属于目标节点与文件，并从 `document_pages` 精确读取。
5. 回答必须带页码引用；没有页面内容时明确要求先处理文档。

## 8. AI Diff 修复

授权提案采用三重绑定：

1. **身份绑定**：工具名、实体类型、实体 ID。
2. **内容绑定**：提案 payload 与写命令真正要执行的 payload 做递归键排序后的规范 JSON 哈希比较。
3. **状态绑定**：后端创建提案时自行读取目标实体并计算状态哈希；执行前再次读取和计算。前端提供的状态哈希不再作为信任来源。

令牌校验和业务写入必须处于同一数据库事务；任何一项不匹配均拒绝且不消费令牌。

## 9. 实施顺序

1. 修复 AI Diff 网关、写命令调用点和回归测试。
2. 升级 schema 并添加任务/Page IR 数据结构。
3. 实现文档引擎探测、任务命令和可靠后台调度。
4. 建立 sidecar 协议、OAR/OvisOCR2 客户端和 searchable PDF 写入器。
5. 用真实 Page IR 重写 PageIndex 建树与按页读取。
6. 文件界面展示引擎状态、进度、失败原因、重试和打开衍生 PDF。
7. 执行前端类型检查/测试/构建、Rust 单元与集成测试、sidecar 检查和差异检查。
8. 写 walkthrough，逐项区分“已实现”“已验证”“需要模型权重或样本实测”。

## 10. 完成标准

- 原 PDF 的处理前后 SHA-256 一致。
- 任务状态不会永久卡在 processing；崩溃任务可恢复为 queued/failed。
- 至少有测试证明 searchable PDF 输出到不同路径并含可提取文字层。
- PageIndex 的页码读取不再返回整份 PDF 开头的固定 3000 字。
- OvisOCR2 未配置时 UI 显示缺失项和修复方式，不写 completed。
- AI 提案 payload 被替换、实体被修改或令牌被重放时，后端测试均拒绝写入。
- walkthrough 列出实际执行的验证和仍未越过的边界。
