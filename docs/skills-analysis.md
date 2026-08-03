# 法律 Skills 生态分析

> 来源：legal-skills（539★）+ FachuanHybridSystem（205★）+ court-document-downloader
> 分析日期：2026-08-03

## 一、法律专业 Skills（18 个）

### 🔥 与 Casy 核心功能直接相关（优先集成）

| Skill | 功能 | 与 Casy 的关系 |
|-------|------|---------------|
| **court-sms** | 法院短信识别→下载文书→归档 | ✅ 已实现基础版（inbox.rs），可增强 |
| **new-case** | 创建案件标准化目录（诉讼12目录/商标/专利） | ✅ 与 Casy 案件管理直接对接 |
| **legal-case-analysis** | 案件材料→法律分析（要件式/风险评估） | 📌 Casy 文书工坊 Copilot 可集成 |
| **litigation-analysis** | 判决书分析→上诉/再审决策支持 | 📌 收到判决书后自动触发分析 |
| **contract-copilot** | 合同审查→风险清单→修订版 DOCX | 📌 文书工坊的合同审查入口 |
| **patent-analysis** | 专利权利要求分析/侵权比对/FTO | 📌 专利案件的专业分析模块 |
| **trademark-assistant** | 商标类别规划/可注册性初筛 | 📌 商标案件的专业分析模块 |
| **invoice-organizer** | 发票 PDF→按项目归档→报销清单 | 📌 收件箱的发票自动处理 |
| **patent-download** | 专利 PDF 批量下载（Google Patents） | 📌 知识库的专利文档采集 |
| **legal-visualization** | 法律关系图/时间轴/证据链→drawio+SVG | 📌 案件详情页的可视化组件 |

### 📋 补充能力

| Skill | 功能 |
|-------|------|
| **legal-ocr** | OCR 统一入口（PaddleOCR/MinerU 自动路由）|
| **pdf-organizer** | PDF 按内容拆分/合并/重命名 |
| **yuandian-law-search** | 元典法条+案例检索（API） |
| **legal-proposal-generator** | 诉讼方案/咨询报告/非诉方案生成 |
| **legal-text-format** | 法律文书排版 |
| **legal-qa-extractor** | 法律问答提取 |
| **opc-legal-counsel** | OPC/小微企业法律顾问分诊 |
| **clawhub-sync** | ClawHub 同步 |

## 二、工具类 Skills（17 个）

| Skill | 功能 | Casy 集成点 |
|-------|------|------------|
| **pdf-processor** | PDF 压缩/合并/裁边/页码 | 文件管理模块 |
| **img2pdf** | 图片→PDF | 收件箱图片导入 |
| **md2word** | Markdown→Word | 文书工坊导出 |
| **legal-ocr** | OCR 统一入口 | 收件箱扫描件处理 |
| **funasr-transcribe** | 通义听悟→文字转写 | 录音文件自动转文字 |
| **tingwu-asr** | 通义听悟 ASR | 同上 |
| **video-screenshot** | 视频截图 | 证据视频提取关键帧 |
| **video-compressor** | 视频压缩 | 证据视频压缩 |
| **universal-media-downloader** | 通用媒体下载 | 证据采集 |
| **douyin-batch-download** | 抖音批量下载 | 证据采集 |
| **agent-email** | 邮件收发 | 收件箱邮件导入 |
| **piclist-upload** | 图床上传 | 文书工坊图片上传 |
| **wechat-article-fetch** | 微信文章抓取 | 知识库采集 |

## 三、法穿（Fachuan）特有能力

| 能力 | 说明 |
|------|------|
| 法院短信自动处理 | 支持 6 种法院送达平台（比 legal-skills 更全） |
| 合同一次生成 | 结构化数据→Word 微服务→打印盖章即可 |
| OA 立案打通 | 数据只录入一次，多系统自动流转 |
| 一键归档 | 结案后自动收集→封面+目录+合并PDF+页码 |
| MCP 协议 | 200+ API 全面开放 |

## 四、Casy 集成方案：百宝囊模块

### 架构设计

```
Casy 百宝囊 (Toolbox)
├── 法律工具
│   ├── 法院短信处理 [court-sms] ← 已有基础版
│   ├── 案件建档 [new-case] ← 新建案件时调用
│   ├── 案件分析 [legal-case-analysis] ← 文书工坊集成
│   ├── 判决分析 [litigation-analysis] ← 收到判决后触发
│   ├── 合同审查 [contract-copilot] ← 文书工坊集成
│   ├── 专利分析 [patent-analysis] ← 专利案件专用
│   ├── 商标助手 [trademark-assistant] ← 商标案件专用
│   ├── 专利下载 [patent-download] ← 知识库集成
│   ├── 法条检索 [yuandian-law-search] ← 全局搜索
│   ├── 法律可视化 [legal-visualization] ← 案件详情页
│   └── 发票整理 [invoice-organizer] ← 收件箱集成
│
├── 文档工具
│   ├── OCR 识别 [legal-ocr] ← 收件箱扫描件
│   ├── PDF 整理 [pdf-organizer] ← 文件管理
│   ├── PDF 处理 [pdf-processor] ← 文件管理
│   ├── 图片转PDF [img2pdf] ← 收件箱图片
│   ├── Markdown转Word [md2word] ← 文书工坊导出
│   └── 法律排版 [legal-text-format] ← 文书工坊
│
├── 媒体工具
│   ├── 语音转写 [funasr/tingwu] ← 录音→文字
│   ├── 视频截图 [video-screenshot] ← 证据提取
│   ├── 视频压缩 [video-compressor] ← 证据压缩
│   └── 媒体下载 [universal-media] ← 证据采集
│
└── 集成工具
    ├── 邮件处理 [agent-email] ← 收件箱邮件
    └── 微信文章 [wechat-article] ← 知识库采集
```

### UI 入口设计

1. **侧边栏"百宝囊"图标** — 点击展开工具面板
2. **上下文菜单** — 右键文件/案件时显示可用工具
3. **收件箱集成** — 拖入文件后自动推荐匹配工具
4. **文书工坊集成** — 侧边栏显示可用分析/生成工具
5. **案件详情页** — 工具栏显示案件类型相关工具

### 实现方式

每个 Skill 是一个 **SKILL.md + scripts/** 的组合。Casy 需要：

1. **Skill 注册表** — SQLite 存储已安装的 skills 元数据
2. **Skill 运行时** — 调用 Python 脚本，传入文件路径/上下文
3. **UI 入口** — 根据 skill 类型和当前上下文动态显示
4. **结果展示** — skill 输出（文件/文本/图表）展示在 Casy UI 中

### 优先级

| 优先级 | Skills | 理由 |
|--------|--------|------|
| P0 | court-sms（增强）、new-case | 每天使用的核心流程 |
| P1 | legal-case-analysis、litigation-analysis | 文书工坊的核心分析能力 |
| P1 | legal-ocr、pdf-organizer | 收件箱的文档处理能力 |
| P2 | contract-copilot、patent-analysis | 专业领域深度分析 |
| P2 | yuandian-law-search、legal-visualization | 检索和可视化 |
| P3 | 其余 skills | 按需集成 |
