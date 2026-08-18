# Casy

> 飞书多维表格的律师专业版 — 本地优先的专利律师案件管理系统

Casy 起点是飞书多维表格的数据结构，但比多维表格更好用：动态字段、跨类型筛选、期限引擎、多通道提醒、知识库、文书生成、大口袋收件箱。

---

## 技术栈

| 层 | 技术 |
|----|------|
| 框架 | Tauri 2 |
| 前端 | Vue 3 + Element Plus + Pinia + Vue Router |
| 编辑器 | TipTap (ProseMirror) |
| 后端 | Rust |
| 数据库 | SQLite (SQLCipher 加密) + FTS5 全文搜索 |
| 公式引擎 | Rust nom 解析器 |
| 同步 | WebDAV + 飞书 Bitable API |
| 提醒 | 本地弹窗 + macOS 通知 + 飞书消息 + 飞书任务 |
| AI | Ollama (本地) / OpenAI 兼容 API (远程) |
| 邮件 | async-imap + IDLE |
| OCR | Tesseract (可选) |

---

## 核心功能

### 业务模块
- **案件管理** — CRUD + 多条件筛选 + 分组/排序/分页 + 看板视图 + 关系网络
- **收件箱（大口袋）** — 多入口 + AI 分类 + 推荐面板 + 安全拷贝 + 卷宗管理
- **文书工坊** — TipTap 编辑器 + Copilot 知识检索侧栏 + AI 写作辅助
- **知识库** — CRUD + FTS5 + 语义向量检索 + 风格标注
- **任务管理** — 四象限 + 庭审准备模板 + 飞书任务同步
- **日历** — 月视图 + 法定节假日 + 期限计算

### 全局能力
- **期限引擎** — 15 条法定规则 + 中国节假日 + 工作日顺延
- **公式引擎** — 飞书公式语法解析 + 本地计算 + 依赖图重算
- **多通道提醒** — 本地弹窗/系统通知/飞书消息/飞书任务，规则引擎调度
- **动态字段** — 按案由/审级自动显示/隐藏字段
- **跨类型筛选** — 不同案件类型的统一筛选维度

### 外部集成
- **飞书同步** — 连接任意多维表格 + 字段映射 + 比较 + 导入 + 双向同步
- **飞书消息** — Bot API 推送提醒消息卡片
- **飞书任务** — Task API 创建/同步任务
- **WebDAV** — 数据库同步 + 冲突解决
- **IMAP** — 邮件监听 + 自动导入

---

## 项目结构

```
Casy/
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── commands/       # Tauri 命令
│   │   ├── db/             # 数据库 schema + 迁移
│   │   ├── formula/        # 公式引擎 + 期限引擎 + 节假日
│   │   ├── sync/           # WebDAV + 飞书同步
│   │   ├── ai/             # AI 后端 + prompt
│   │   ├── parse/          # 文档解析
│   │   ├── files/          # 文件管理
│   │   └── docsy/          # Docsy 模板引擎
│   └── Cargo.toml
├── src/                    # Vue 前端
│   ├── modules/            # 功能模块
│   │   ├── cases/          # 案件管理（列表/详情/看板/关系图）
│   │   ├── tasks/          # 任务管理（四象限）
│   │   ├── calendar/       # 日历（月视图）
│   │   ├── inbox/          # 收件箱（大口袋）
│   │   ├── docs/           # 文书工坊（编辑器+Copilot）
│   │   ├── knowledge/      # 知识库
│   │   ├── files/          # 案卷文件
│   │   ├── sync/           # 同步状态
│   │   ├── settings/       # 设置（飞书/AI/WebDAV/IMAP）
│   │   └── home/           # 首页 Dashboard
│   ├── stores/             # Pinia Stores
│   ├── core/               # tauriBridge
│   ├── shared/             # 共享组件
│   └── router/             # 路由
├── docs/                   # 设计文档
│   ├── casy-design-philosophy.md # ★ 设计哲学（唯一总纲：原则/模块蓝图/UI 规范/路线图）
│   ├── architecture.md     # ★ 顶层架构设计（统领全局）
│   ├── modules/            # 各模块细节设计文档（00-README.md 索引）
│   └── archive/            # 归档文档（调研/旧版设计）
├── Casy-SPEC.md            # 综合技术规格
├── Casy-STATUS.md          # 项目状态与进度
└── README.md               # 本文件
```

---

## 文档层级

```
README.md（你在这里）
  ├─ docs/casy-design-philosophy.md ← 设计哲学：八大原则、模块蓝图、UI 规范、路线图
  └─ docs/architecture.md           ← 顶层架构：模块全景、数据模型、离线提醒决策
       └─ docs/modules/             ← 各模块细节设计
            ├─ 00-README.md         ← 模块索引与依赖关系
            ├─ 01-cases.md          ← 案件管理
            ├─ 02-status-machine.md ← 三轨状态机
            ├─ 03-tasks.md          ← 任务系统（GTD）
            ├─ 04-inbox.md          ← 收件箱（大口袋）
            ├─ 05-inbox-batch.md    ← 收件箱批处理
            ├─ 06-calendar-deadline.md ← 日历与期限引擎
            ├─ 07-knowledge.md      ← 知识库
            ├─ 08-docsy.md          ← 文书工坊
            ├─ 09-files.md          ← 文件管理
            ├─ 10-sync.md           ← 同步（WebDAV/飞书）
            ├─ 11-email.md          ← 邮件（IMAP）
            ├─ 12-reminder.md       ← 提醒系统
            ├─ 13-ai-companion.md   ← AI 智伴
            ├─ 14-data-layer.md     ← 数据层
            ├─ 15-observability-settings.md ← 可观测性与设置
            └─ 16-openness.md       ← 双向开放（MCP/Skill）
```

---

## 构建与运行

### 前置条件

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/) (latest stable)
- [Tauri CLI](https://tauri.app/)

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建

```bash
npm run tauri build
```

### 可选依赖

| 依赖 | 用途 | 安装 |
|------|------|------|
| Tesseract + 中文包 | OCR 文档识别 | `brew install tesseract tesseract-lang` |
| Ollama | 本地 AI 模式 | `brew install ollama` |

---

## 项目指标

| 指标 | 数值 |
|------|------|
| 代码行数 | ~25,000 |
| Rust 命令 | 70+ |
| Vue 组件 | 30 |
| 测试 | 61 (全部通过) |
| 编译错误 | 0 |
