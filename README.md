# Casy

> 飞书多维表格的律师专业版 — 本地优先的专利律师案件管理系统

Casy 起点是飞书多维表格的数据结构，但比多维表格更好用：动态字段、跨类型筛选、期限引擎、多通道提醒、知识库、文书生成、大口袋收件箱。

**架构**：cordis 风格内核（借鉴 DeepSeek Harness / @deepseek-ai/cordis）——Context 服务解析 + Service 注入 + Fiber 生命周期，9+ 个业务服务（ctx.cases / ctx.tasks / ...）构成数据通路：视图 → ctx 服务 → tauriBridge → Rust 命令（写入口唯一）。AI 能力以工具/技能形式接入。现行产品设计基准为 docs/casy-product-design-v3.md（v2.4 归档于 docs/casy-design-philosophy.md）。

---

## 技术栈

| 层 | 技术 |
|----|------|
| 框架 | Tauri 2 |
| 前端 | Vue 3 + Element Plus + Pinia + Vue Router（TypeScript） |
| 编辑器 | TipTap (ProseMirror) |
| 后端 | Rust |
| 数据库 | SQLite (SQLCipher 加密) + FTS5 全文搜索（Schema v18，含子任务/重复规则/项目垂直拆表/独立日程实体） |
| 公式引擎 | Rust nom 解析器 |
| 同步 | WebDAV + 飞书 Bitable API |
| 提醒 | 本地弹窗 + macOS 通知 + 飞书消息 + 飞书任务（R1-R4 分级预警） |
| AI | Ollama (本地) / OpenAI 兼容 API (远程) + 命令路由 + 审计日志 |
| 邮件 | async-imap + IDLE |
| OCR | Tesseract (可选) |

---

## 核心功能

### 业务模块
- **首页 · 今日焦点** — 早报 + 分级期限预警 + 四卡概览（点击下钻）
- **案件管理** — CRUD + 多条件筛选 + 分组/排序/分页 + 关系网络
- **项目** — 个人项目直管 + 法律项目镜像透出（垂直拆表架构）
- **任务管理** — GTD 透视工作台（收件箱/今天/计划中/随时/等待/回顾/某天）+ 自然语言捕获（日期+时刻解析落库）+ 一键完成 + ⌘Z 撤销 + 拖拽排序（实时持久化）+ 今日重点★与「整理今天」分流仪式 + 子任务 + 重复任务（完成后自动生成下一实例）+ 时间盒展示
- **日历** — 年（负载热力）/月/周（时长定位+当前时刻线）/日（小时时间轴排期）四视图 + 独立日程实体 + NL 直建日程 + 双向拖拽改期
- **收件箱（大口袋）** — 快速捕获 + 厘清工作台 ⚠️（AI 分析/语音/截图等入口为占位，见下）
- **文书工坊** — TipTap 编辑器 + Copilot 知识检索侧栏 + AI 写作辅助
- **知识库** — CRUD + FTS5 + 语义向量检索 + 风格标注
- **数据看板** — 手绘 SVG 四图：月度任务趋势 / 项目状态环图 / 轨道分布 / 近期庭审甘特（SQL 聚合 + 点击下钻）
- **卷宗管理** — 12 阶段目录树 + 访达拖入归档（重名去重）+ 既有卷宗扫描入库 + 智能重命名工作台 + Finder 定位/打开 + 预览侧板

### 全局能力
- **期限引擎** — 15 条法定规则 + 中国节假日 + 工作日顺延
- **公式引擎** — 飞书公式语法解析 + 本地计算 + 依赖图重算
- **多通道提醒** — 本地弹窗/系统通知/飞书消息/飞书任务，规则引擎调度（⚠️ 飞书通道送达状态机重构中：当前失败仍可能记 sent）
- **动态字段** — 按案由/审级自动显示/隐藏字段
- **⌘K 全局搜索** — 项目/任务/案件/知识分栏结果 + 键盘导航
- **KeyboardCenter** — 全局快捷键统一注册（冲突检测）

### 占位功能（诚实声明 · 审计 P1#3）
以下能力在 UI 中有入口但后端为占位实现，将在后续迭代补齐或显式移除：
- 收件箱 AI 自动分类、送达文书下载、截图/剪贴板导入、语音录入、批量处理、重试队列
- WebDAV 同步状态显示（恒为未连接）

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
│   │   ├── tasks/          # 任务管理（7 透视 + 自然语言捕获）
│   │   ├── calendar/       # 日历（月视图 + Forecast）
│   │   ├── inbox/          # 收件箱（大口袋）
│   │   ├── docs/           # 文书工坊（编辑器+Copilot）
│   │   ├── knowledge/      # 知识库
│   │   ├── files/          # 案卷文件
│   │   ├── ai/             # AI 智伴（路由/确认/审计/对话）
│   │   ├── reminder/       # 提醒（R1-R4 预警）
│   │   ├── sync/           # 同步状态
│   │   ├── settings/       # 设置（飞书/AI/WebDAV/IMAP/提醒）
│   │   └── home/           # 首页 Dashboard
│   ├── core/               # tauriBridge + services（cordis 数据通路）+ plugins
│   │   └── plugins/        # 9 个业务插件 + CasyContext 容器
│   ├── stores/             # Pinia Stores（TypeScript）
│   ├── shared/             # 共享组件（AIStatusBadge/ReminderToast/...）
│   ├── types/              # TypeScript 类型定义
│   └── router/             # 路由
├── docs/                   # 设计文档
│   ├── casy-design-philosophy.md # ★ 设计哲学（唯一总纲：原则/模块蓝图/UI 规范/路线图）
│   ├── devlog/             # 开发日志
│   └── archive/            # 数据模型口径（architecture.md）+ 法条依据 + feishu 数据
├── designs/                # UI 设计稿（11 张屏幕 PNG+SVG + HTML 原型）
├── Casy-STATUS-v3.md       # 项目状态（当前）
└── README.md               # 本文件
```

---

## 文档层级

```
README.md（你在这里）
  ├─ docs/casy-design-philosophy.md ← 设计哲学（唯一总纲：八大原则、模块蓝图、UI 规范、路线图）
  ├─ docs/devlog/                   ← 开发日志（按日期 + TODO.md 待办清单）
  └─ docs/archive/                  ← 数据模型口径（architecture.md，被设计哲学引用）+ 法条依据 + feishu-base 数据
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
| 代码行数 | ~43,300（Rust ~20k + Vue ~19.4k + TS/JS） |
| Rust 命令 | 232（specta 类型导出 58） |
| Vue 组件 | 44 |
| 业务插件 | 9（v3.0 插件化架构） |
| 注册工具 | 38 |
| 路由 | 18 |
| 编译错误 | 0 |

---

## 附录 B：改动登记

- 2026-08-20 — **文档清理与更新**：删除过时文档（Casy-SPEC / Casy-STATUS v0.2 / log / prompt / 全部旧规格与旧架构设计 / 过时的对标落实方案），docs 精简为「设计哲学（唯一总纲）+ devlog（开发日志）+ archive（数据模型口径 / 法条依据 / 飞书数据）」；README 与 Casy-STATUS-v3 同步最新状态（cordis 内核、7 透视、schema v13、时间模型、提醒交接、数据链）。
- 2026-08-19 — **插件系统补齐为真实实现**（架构收口）：`src/core/plugin/` 从占位符重写为真实容器（types.ts 新建、context.ts 真实 CasyContext、initializer.ts 安装 9 插件 38 工具 + 注册 AI 提供商），`src/core/ai/tool-caller.ts` 实现多轮对话 + 工具调用循环；后端新增 `ai_chat` 多轮对话命令（过 ai_runs 审计 + 每日限额）；修复 AI 对话面板运行即崩；浏览器预览模式补 ai_chat mock。详见 `docs/devlog/2026-08-19-plugin-real.md`。
- 2026-08-18 — v3.0 文档同步：项目指标刷新（43.3k 行 / 148 命令 / 44 组件 / 9 插件 / 38 工具），项目结构补全 ai/reminder/core/plugins/types 模块，文档层级补 architecture-v3 与 architecture-plugin-system，新增 designs/ 与 Casy-STATUS-v3.md。详见 `docs/devlog/2026-08-18.md`。
