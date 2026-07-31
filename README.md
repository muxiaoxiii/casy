# Casy

> 本地优先的专利律师案件管理系统

Casy 是一款基于 Tauri 2 的桌面应用，专为专利律师设计，集成案件管理、期限预警、文书生成、飞书同步、AI 收件箱、知识库等功能。

---

## 技术栈

| 层 | 技术 |
|----|------|
| 框架 | Tauri 2 |
| 前端 | Vue 3 + Element Plus + Pinia + Vue Router |
| 编辑器 | TipTap (ProseMirror) |
| 后端 | Rust |
| 数据库 | SQLite (SQLCipher 加密) + FTS5 全文搜索 |
| 同步 | WebDAV + 飞书 Bitable API |
| AI | Ollama (本地) / OpenAI 兼容 API (远程) |
| 邮件 | async-imap + IDLE |
| OCR | Tesseract (可选) |

---

## 核心功能

- **案件管理** — CRUD + 多条件筛选 + 分组/排序/分页 + 看板视图 + 关系网络
- **期限引擎** — 15 条法定规则 + 中国节假日 + 工作日顺延 + 预警
- **时间线** — 合并办案日志/庭审/任务，按月分组
- **任务管理** — 四象限 + 过期标红 + 庭审自动生成准备任务
- **日历** — 月视图 + 五色事件
- **收件箱（大口袋）** — 多入口（拖拽/托盘/快捷键/文件夹监听/IMAP） + AI 分类 + 案件匹配
- **文书工坊** — TipTap 编辑器 + Copilot 知识检索侧栏 + AI 写作辅助 + Docsy 模板 + DOCX 导出
- **知识库** — CRUD + FTS5 + 语义向量检索 + 风格标注
- **同步** — WebDAV (VACUUM INTO + ETag + 冲突解决) + 飞书双向同步
- **安全** — SQLCipher 加密 + OS Keychain 存储密码

---

## 项目结构

```
Casy/
├── src-tauri/          # Rust 后端
│   ├── src/
│   │   ├── commands/   # Tauri 命令 (70 个)
│   │   ├── db/         # 数据库操作
│   │   ├── formula/    # 期限引擎 + 节假日
│   │   ├── sync/       # WebDAV + 飞书同步
│   │   ├── parse/      # 文档解析
│   │   ├── ai/         # AI 后端
│   │   ├── files/      # 文件管理
│   │   └── docsy/      # Docsy 模板引擎
│   └── Cargo.toml
├── src/                # Vue 前端
│   ├── modules/        # 功能模块
│   │   ├── cases/      # 案件管理
│   │   ├── tasks/      # 任务管理
│   │   ├── calendar/   # 日历
│   │   ├── inbox/      # 收件箱
│   │   ├── documents/  # 文书工坊
│   │   ├── files/      # 案卷文件
│   │   ├── knowledge/  # 知识库
│   │   ├── sync/       # 同步状态
│   │   ├── settings/   # 设置
│   │   └── home/       # 首页
│   ├── stores/         # Pinia Stores (5 个)
│   ├── core/           # tauriBridge
│   ├── shared/         # 共享组件
│   ├── assets/         # 主题 CSS
│   └── router/         # 路由
├── docs/               # 文档
│   ├── inbox-system-design.md
│   ├── todo-features.md
│   └── archive/        # 归档文档
├── Casy-SPEC.md        # 综合技术规格
├── Casy-STATUS.md      # 项目状态与进度
└── README.md           # 本文件
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

## 文档

| 文档 | 说明 |
|------|------|
| [Casy-SPEC.md](./Casy-SPEC.md) | 综合技术规格（合并全部设计文档） |
| [Casy-STATUS.md](./Casy-STATUS.md) | 项目状态、已完成阶段、剩余工作 |
| [docs/inbox-system-design.md](./docs/inbox-system-design.md) | 收件箱（大口袋）完整设计 |
| [docs/todo-features.md](./docs/todo-features.md) | 未实现功能清单 |
| [docs/archive/](./docs/archive/) | 归档的历史文档 |

---

## 项目指标

| 指标 | 数值 |
|------|------|
| 代码行数 | 19,595 |
| Rust 命令 | 70 |
| Vue 组件 | 25 |
| 测试 | 16 (全部通过) |
| 编译错误 | 0 |
