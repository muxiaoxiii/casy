# Casy — 跨端案件管理系统完整规划

> **版本**: v1.0  
> **日期**: 2026-07-30  
> **定位**: 本地优先的专利律师案件管理 + 文书生成 + 飞书同步 + AI 收件箱 + 案卷管理  
> **栈**: Tauri 2 + Vue 3 + SQLite + Rust

---

## 一、可行性评估

### 1.1 技术选型可行性总表

| 技术 | 可行性 | 风险 | 推荐方案 |
|------|--------|------|---------|
| Tauri 2 桌面框架 | ✅ 确定可行 | 无 | 与 Docsy 一致 |
| Vue 3 + Element Plus | ✅ 确定可行 | 无 | 与 Docsy 一致 |
| SQLite + FTS5 | ✅ 确定可行 | 无 | rusqlite bundled |
| WebDAV 同步 | ✅ 确定可行 | ETag 因服务器而异 | VACUUM INTO + 临时路径上传 |
| 飞书 Bitable API | ✅ 可行 | 限流 100次/秒 | 令牌桶 + 队列 |
| TipTap 编辑器 | ✅ 可行 | 中文 IME 在 WebKit 有 5 个 open issue | isComposing 守卫 + 防抖 |
| Tesseract OCR | ⚠ 有限 | 中文精度 70-85% | `rusty-tesseract` CLI 调用，简单文本够用 |
| PaddleOCR | ⚠ 需额外集成 | 无 Rust crate | 通过 Python 子进程调用（需用户安装 paddleocr） |
| Vision LLM | ⚠ 有风险 | 幻觉+隐私 | 仅作复杂文档补充，不做主引擎 |
| Vision LLM (GPT-4o) | ⚠ 有风险 | 幻觉风险、隐私 | 仅作补充，不做主引擎 |
| IMAP 邮件监听 | ✅ 确定可行 | 连接断开需重连 | async-imap + IDLE + 重连循环 |
| DOCX 导出 | ✅ 确定可行 | 复杂格式需定制 | html-to-docx 或 docx npm |
| Ollama 本地 AI | ✅ 可行 | 大模型需 16GB+ 内存 | qwen2.5:14b 或更小模型 |

### 1.2 关键技术决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| IMAP crate | `async-imap` (非 `imap`) | 原生 async + tokio，IDLE 推送通知 |
| OCR 引擎 | Tesseract (`rusty-tesseract`) 为主 + Vision LLM 补充 | Tesseract 本地免费，复杂文档用 Vision LLM 补充 |
| SQLite 安全拷贝 | `VACUUM INTO` | 原子操作，单文件输出，活数据库上安全 |
| WebDAV 上传 | 临时路径 → MOVE | 防止上传中断导致文件损坏 |
| 编辑器 | TipTap v3.29.2+ | Vue 3 原生支持，Suggestion 扩展 |
| DOCX 导出 | `html-to-docx` (原型) → `docx` (精调) | 快速原型 → 逐步精确控制 |
| AI 后端 | Ollama (本地) / OpenAI 兼容 API (远程) | 双模式，用户可选 |
| 节假日数据 | 内置当年 + JSON 文件更新 | 不依赖网络，用户可手动更新 |

### 1.3 SQLite 性能预估

| 数据 | 记录数 | 预估大小 |
|------|--------|---------|
| cases | ~100 | ~50 KB |
| case_logs | ~500 | ~150 KB |
| hearings | ~200 | ~80 KB |
| tasks | ~100 | ~20 KB |
| officials | ~50 | ~10 KB |
| knowledge_items | ~200 | ~100 KB |
| case_files | ~300 | ~30 KB |
| 全文索引 (FTS5) | - | ~200 KB |
| **总计** | | **< 1 MB** |

全量 WebDAV 同步 <1MB，宽带 <1秒，移动网络 1-3 秒。无需增量同步。

---

## 二、整体架构

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Casy 桌面应用                               │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    前端层 (Vue 3 + Element Plus)               │  │
│  │  ┌────────┬────────┬────────┬────────┬────────┬────────┐     │  │
│  │  │ 案件   │ 时间线 │ 任务   │ 日历   │ 收件箱 │ 文书   │     │  │
│  │  │ 管理   │ 视图   │ 四象限 │ 月视图 │ 大口袋 │ 工坊   │     │  │
│  │  ├────────┼────────┼────────┼────────┼────────┼────────┤     │  │
│  │  │ 案卷   │ 设置   │ 同步   │ 知识库 │ Skill  │ 邮件   │     │  │
│  │  │ 文件   │        │ 状态   │        │ 工作流 │ 记录   │     │  │
│  │  └────────┴────────┴────────┴────────┴────────┴────────┘     │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Pinia Stores: cases | tasks | inbox | files | settings │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕ Tauri IPC                            │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    命令层 (Tauri Commands)                     │  │
│  │  ┌────────┬────────┬────────┬────────┬────────┬────────┐     │  │
│  │  │ cases  │ timeline│ tasks │calendar│ inbox  │ docsy  │     │  │
│  │  │ clients│ officials│ files│formula │ parse  │ skill  │     │  │
│  │  │ hearings│         │      │deadline│ email  │ draft  │     │  │
│  │  ├────────┼────────┼────────┼────────┼────────┼────────┤     │  │
│  │  │ sync_webdav │ sync_feishu │ ai_bridge │ settings │ system│ │  │
│  │  └────────┴────────┴────────┴────────┴────────┴────────┘     │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    业务层 (Rust Modules)                       │  │
│  │  ┌──────────────────────────────────────────────────────┐    │  │
│  │  │ db/          │ formula/    │ sync/      │ parse/     │    │  │
│  │  │ cases.rs     │ engine.rs   │ webdav.rs  │ summons.rs │    │  │
│  │  │ clients.rs   │ functions.rs│ feishu.rs  │ hearing.rs │    │  │
│  │  │ hearings.rs  │ holidays.rs │ conflict.rs│ judgment.rs│    │  │
│  │  │ tasks.rs     │ deadlines.rs│ queue.rs   │ ocr.rs     │    │  │
│  │  │ officials.rs │             │            │ ai_extract │    │  │
│  │  │ timeline.rs  │             │            │            │    │  │
│  │  │ relations.rs │             │            │            │    │  │
│  │  │ inbox.rs     │             │            │            │    │  │
│  │  │ files.rs     │             │            │            │    │  │
│  │  │ knowledge.rs │             │            │            │    │  │
│  │  │ email.rs     │             │            │            │    │  │
│  │  │ drafts.rs    │             │            │            │    │  │
│  │  └──────────────────────────────────────────────────────┘    │  │
│  │  ┌──────────────────────────────────────────────────────┐    │  │
│  │  │ ai/             │ files/           │ docsy/           │    │  │
│  │  │ mod.rs          │ classify.rs      │ bridge.rs        │    │  │
│  │  │ ollama.rs       │ rename.rs        │                  │    │  │
│  │  │ openai.rs       │ knowledge_extract│                  │    │  │
│  │  │ budget.rs       │                  │                  │    │  │
│  │  └──────────────────────────────────────────────────────┘    │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    存储层                                      │  │
│  │  SQLite (WAL) + 文件系统 (案件文件夹) + 外部 API               │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
         ↕ WebDAV              ↕ 飞书 API           ↕ Docsy IPC
┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐
│  WebDAV 服务器    │   │  飞书多维表格      │   │  Docsy 应用       │
└──────────────────┘   └──────────────────┘   └──────────────────┘
```

### 2.2 模块间数据流

```
                        ┌─────────┐
                        │ 收件箱   │
                        │ (Inbox) │
                        └────┬────┘
                             │ AI 分类 + 案件匹配
                             ↓
┌─────────┐  关联    ┌─────────┐  1:N    ┌─────────┐
│ 客户     │←────────│ 案件     │────────→│ 办案日志 │
│ (Client)│         │ (Case)  │         │ (Log)   │
└─────────┘         └────┬────┘         └─────────┘
                         │ 1:N
                         ├──────────→┌─────────┐
                         │           │ 庭审     │
                         │           │(Hearing)│
                         │           └────┬────┘
                         │                │ 自动生成
                         │                ↓
                         │           ┌─────────┐
                         ├──────────→│ 任务     │
                         │           │ (Task)  │
                         │           └─────────┘
                         │ 1:N
                         ├──────────→┌─────────┐
                         │           │ 案卷文件 │
                         │           │ (File)  │
                         │           └────┬────┘
                         │                │ 选择性
                         │                ↓
                         │           ┌─────────┐
                         │           │ 知识库   │
                         │           │(Knowled)│
                         │           └─────────┘
                         │ M:N
                         ├──────────→┌─────────┐
                         │           │ 官方人员 │
                         │           │(Official)│
                         │           └─────────┘
                         │ 自引用
                         └──────────→┌─────────┐
                                     │ 案件关系 │
                                     │(Relation)│
                                     └─────────┘

期限引擎 ← 读取 cases + deadline_rules → 输出 DeadlineResult
日历 ← 合并 hearings + deadlines + tasks → CalendarEvent
时间线 ← 合并 case_logs + hearings + tasks → TimelineEvent
文书工坊 ← 读取 cases → 调用 Docsy → 写入 case_files + case_logs
```

### 2.3 事件系统

模块间通过 Pinia store 的 watch 和 Tauri 的 emit/listen 通信：

```javascript
// 案件创建 → 自动创建文件夹 + 初始化关联
casesStore.$onAction(({ name, after }) => {
  if (name === 'createCase') {
    after((result) => {
      // 自动创建文件夹
      tauriCallSafe('ensure_case_folder', { caseId: result.id })
      // 初始化关联（从其他案件推断）
      tauriCallSafe('detect_case_relations', { caseId: result.id })
    })
  }
})

// 庭审创建 → 自动生成准备任务
hearingsStore.$onAction(({ name, after }) => {
  if (name === 'createHearing') {
    after((result) => {
      tauriCallSafe('generate_hearing_prep_tasks', { hearing: result })
    })
  }
})

// 收件箱归档 → 自动记录日志
inboxStore.$onAction(({ name, after }) => {
  if (name === 'fileToCase') {
    after(() => {
      timelineStore.loadTimeline()
      casesStore.loadCases()
    })
  }
})
```

---

## 三、模块详细设计

> 以下为各模块的实现规格摘要。完整 SQL、Rust 函数签名、Vue 组件代码见补充文档：
> - `Casy-实现规格-数据层.md`
> - `Casy-实现规格-核心模块.md`
> - `Casy-实现规格-同步与公式.md`
> - `Casy-实现规格-收件箱与解析.md`
> - `Casy-实现规格-UI与Copilot.md`

### 3.1 案件管理

| 功能 | 实现 |
|------|------|
| CRUD | Tauri command + rusqlite，ID 用 UUID v4 |
| 列表 | 分页(50/页) + 多条件筛选 + 排序 + 分组(客户/轨道/法院/审级) |
| 全文搜索 | FTS5 虚拟表，MATCH 查询 |
| 详情 | 三栏布局(信息+时间线+关联)，自动保存(2秒防抖) |
| 状态计算 | 从 case_result 实时算，不存库 |
| 行颜色 | 🔴3天内期限 🟡14天内 ⬜已完结（与期限引擎阈值一致） |
| 文件夹 | 创建案件时自动建 7 个子文件夹 |

### 3.2 时间线

| 功能 | 实现 |
|------|------|
| 事件合并 | UNION 3 张表(case_logs+hearings+tasks) |
| 颜色图标 | submitted=📤绿 received=📥蓝 record=📝灰 hearing=📅蓝 task=📌紫 |
| 筛选排序 | 按类型筛选，按日期排序(升/降) |
| 内联操作 | 添加/编辑/删除事件 |

### 3.3 任务管理

| 功能 | 实现 |
|------|------|
| 四象限 | urgent_important/important/urgent/normal |
| 截止日 | 倒计时计算，过期标红 |
| 自动任务 | 创建庭审时自动生成准备任务(材料/联系/证据) |
| 批量操作 | 批量完成/删除 |

### 3.4 日历

| 功能 | 实现 |
|------|------|
| 月视图 | 7列网格，日期格子显示事件色块 |
| 颜色编码 | 蓝=口审 红=开庭 黄=二审 橙=期限 紫=任务 |
| 交互 | 点击日期展开事件列表，点击事件跳转案件 |
| 数据源 | 合并 hearings+deadlines+tasks |

### 3.5 期限引擎

| 功能 | 实现 |
|------|------|
| 规则表 | deadline_rules 声明式配置，按 track 匹配 |
| 工作日 | 中国法定节假日+调休，内置当年数据，JSON 文件可更新 |
| 计算 | day/workday/month 三种偏移单位 |
| 预警 | 🔴≤3天 🟡4-14天 🟢>14天 |
| 每日重算 | 启动时 + 每天 00:01 |

### 3.6 案件关系

| 功能 | 实现 |
|------|------|
| 关系类型 | same_patent/same_party/appeal_of/cross_reference |
| 双向存储 | A→B 和 B→A 同时写入 |
| 自动检测 | 按专利号/客户名自动建议关联 |
| UI | 列表展示，按关系类型分组，点击跳转 |

### 3.7 飞书迁移

| 功能 | 实现 |
|------|------|
| 导入 | 解析 feishu-full-dump.json，逐表逐条导入 |
| 字段映射 | 飞书字段类型→Casy 类型(文本/单选/多选/日期/关联) |
| 幂等 | 用飞书 record_id 作为 Casy id，INSERT OR REPLACE |
| 进度 | 实时显示导入计数+错误列表 |

### 3.8 大口袋（统一信息入口与数据更新管道）

> 详细设计见 `docs/inbox-system-design.md`

| 功能 | 实现 |
|------|------|
| **入口** | UI 拖拽 / 系统托盘 / 全局快捷键 / 悬浮窗 / 文件夹监听 / 邮件监听 |
| **文本提取** | PDF→OCR DOCX→XML 图片→OCR .eml→mailparse 剪贴板→直接 |
| **AI 分类** | 11 种类型（传票/口审/判决/证据/法条/节假日/案由/邮件/笔记等） |
| **路由** | 案件关联 / 知识库 / 内部数据库更新 / 日历 / 待办 |
| **内部数据库更新** | 节假日→日历 / 案由→案由表 / 法条→知识库 / 法院→officials |
| **知识库** | 法条/笔记/邮件/摘要，FTS5 全文搜索，版本追踪 |
| **参考项目** | LawRefBook（中国法律 Markdown）/ LawVault（Tauri+SQLite+向量搜索）/ Paperless-ngx（自动分类管道） |

### 3.9 文档解析

| 文档类型 | 解析方案 |
|---------|---------|
| 传票 | 正则(案号/日期/法院/法官)+AI增强 |
| 口审通知书 | 正则(案件编号/专利号/请求人/合议组)+AI增强 |
| 判决书 | 正则(案号/当事人/判决结果)+AI增强 |
| 通用 | Vision LLM 图片识别(回退) |

### 3.10 案卷文件管理

| 功能 | 实现 |
|------|------|
| 分类 | 传票/证据/交文/收文/内部/通信/其他 |
| 自动命名 | `{日期}_{分类}_{案号}.{扩展名}` |
| 知识沉淀 | 标记→AI读取→提取摘要/关键词→写入knowledge_items |
| 搜索 | FTS5 索引文件名+摘要+关键词 |

### 3.11 文书工坊

| 功能 | 实现 |
|------|------|
| 模板生成 | 调用 Docsy render_docx_template，案件字段自动映射 |
| Copilot 撰写 | TipTap 编辑器 + 4种Suggestion扩展 |
| 补全触发 | {案件字段 【法条 @当事人 [案件引用 |
| 草稿管理 | auto-save + 版本号 + 导出Word |
| DOCX导出 | html-to-docx 或 docx npm |

### 3.12 邮件记录

| 功能 | 实现 |
|------|------|
| IMAP 监听 | async-imap + IDLE推送 + 29分钟超时重连 |
| 白名单 | 只处理指定发件人/主题关键词的邮件 |
| Token保护 | 日预算50次+长度截断3000字+批量延迟5分钟 |
| 自动处理 | AI提取→匹配案件→存入收件箱 |

### 3.13 同步引擎

| 功能 | 实现 |
|------|------|
| WebDAV | VACUUM INTO安全拷贝→临时路径上传→MOVE原子操作 |
| 冲突 | If-Match ETag检测 + 用户选择(本地/远程/另存) |
| 飞书 | PUSH/PULL + 令牌桶限流 + 字段映射 + 时间戳裁决 |
| 自动 | 启动时同步 + 5秒防抖自动PUSH + 15分钟定时PULL |

### 3.14 AI 模式

| 功能 | 实现 |
|------|------|
| 模式 | 本地(Ollama) / 远程(OpenAI兼容) / 无AI |
| 调用 | 统一AiBackend抽象层，按模式路由 |
| 用途 | 收件箱分类/文档解析/知识沉淀/上下文补全 |
| 保护 | TokenBudget日预算+用量统计 |

---

## 四、扩展性设计

### 4.1 新增案件类型

只需在 `deadline_rules` 表添加新规则：

```sql
INSERT INTO deadline_rules (id, track, trigger_field, offset_days, offset_unit, rule_name)
VALUES ('uuid', 'new_track', 'filing_date', 30, 'day', '新类型审限');
```

前端的轨道筛选器从数据库动态读取，无需改代码。

### 4.2 新增文档解析类型

在 `src-tauri/src/parse/` 下新增模块，实现 `DocumentParser` trait：

```rust
pub trait DocumentParser {
    fn name(&self) -> &str;
    fn can_parse(&self, text: &str) -> bool;
    fn parse(&self, text: &str) -> Result<ParsedDocument>;
}
```

注册到解析器列表即可自动识别。

### 4.3 新增 AI 后端

实现 `AiBackend` trait：

```rust
pub trait AiBackend: Send + Sync {
    async fn classify(&self, text: &str) -> Result<DocumentClassification>;
    async fn extract(&self, text: &str, doc_type: &str) -> Result<serde_json::Value>;
    async fn summarize(&self, text: &str) -> Result<String>;
    async fn call_raw(&self, prompt: &str) -> Result<String>;
}
```

已有：OllamaBackend, OpenAiBackend, NoOpBackend。

### 4.4 新增同步目标

实现 `SyncTarget` trait：

```rust
pub trait SyncTarget: Send + Sync {
    fn name(&self) -> &str;
    fn upload(&self, data: &[u8], path: &str) -> Result<String>;
    fn download(&self, path: &str) -> Result<(Vec<u8>, String)>;
    fn check_version(&self, path: &str) -> Result<Option<String>>;
}
```

已有：WebDavTarget, FeishuTarget。

### 4.5 插件架构（后续）

```
src-tauri/src/plugins/
├── mod.rs              # 插件注册表
├── trait.rs            # Plugin trait 定义
├── email_plugin.rs     # 邮件插件
├── calendar_plugin.rs  # 日历插件
└── ...
```

---

## 五、安全与隐私

### 5.1 数据加密

| 数据 | 加密方案 | 说明 |
|------|---------|------|
| SQLite 数据库 | SQLCipher（rusqlite `bundled-sqlcipher` feature） | AES-256 全库加密，密钥从 OS keychain 读取 |
| IMAP 密码 | OS Keychain（macOS Keychain / Windows Credential Manager） | 不存数据库 |
| WebDAV 密码 | OS Keychain | 同上 |
| AI API Key | OS Keychain | 同上 |
| 飞书 App Secret | OS Keychain | 同上 |

```toml
# Cargo.toml 加密方案
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
keyring = "3"  # 跨平台 OS keychain 访问
```

### 5.2 远程 AI 数据保护

当使用远程 AI（OpenAI/DeepSeek）时，发送给 API 的数据包含案件信息。保护措施：

| 措施 | 说明 |
|------|------|
| 用户确认 | 首次使用远程 AI 时弹窗告知数据流向 |
| PII 脱敏 | 发送前替换当事人姓名为代号（甲/乙/丙） |
| 最小化 | 只发送必要文本，不发送完整案件数据 |
| 本地优先 | 默认本地模式，远程 AI 需手动开启 |

### 5.3 数据保留

| 数据 | 保留策略 |
|------|---------|
| 案件数据 | 永久（用户手动删除） |
| 办案日志 | 随案件级联删除 |
| 收件箱 | 已归档的 30 天后可自动清理 |
| AI 分类结果 | 随收件箱项删除 |
| 邮件记录 | 永久（用户手动删除） |
| 知识库 | 永久（用户手动删除） |

## 六、错误处理

```rust
// 统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum CasyError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("网络错误: {0}")]
    Network(#[from] reqwest::Error),

    #[error("AI 调用失败: {0}")]
    Ai(String),

    #[error("同步冲突: {local} vs {remote}")]
    SyncConflict { local: String, remote: String },

    #[error("文档解析失败: {0}")]
    ParseError(String),

    #[error("模板渲染失败: {0}")]
    TemplateError(String),
}

// 前端统一错误处理
async function tauriCallSafe(command, args = {}) {
  try {
    const result = await invoke(command, args)
    return { ok: true, data: result }
  } catch (err) {
    logError(command, err)
    return { ok: false, error: err.message }
  }
}
```

---

## 七、测试策略

| 层 | 工具 | 覆盖范围 |
|----|------|---------|
| Rust 单元测试 | `#[test]` | 公式引擎、期限计算、日期解析、正则匹配 |
| Rust 集成测试 | `rusqlite` in-memory DB | CRUD、查询、迁移 |
| Vue 单元测试 | Vitest | Store actions、composables、工具函数 |
| E2E 测试 | Tauri WebDriver (可选) | 关键流程：创建案件→添加日志→生成文书 |
| 手动测试 | 实际飞书数据导入 | 迁移正确性 |

---

## 八、部署与打包

```toml
# Cargo.toml
[package]
name = "casy"
version = "0.1.0"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

```bash
# macOS
npm run tauri build -- --bundles dmg

# Windows
npm run tauri build -- --target x86_64-pc-windows-msvc --bundles nsis

# 自动更新（Tauri Updater 插件）
# 需要配置：
# 1. tauri.conf.json 中启用 updater 插件
# 2. 配置 endpoint（更新检查 URL）
# 3. 生成签名密钥对（npm run tauri signer generate）
# 4. 公钥配置到 tauri.conf.json
# 5. CI 中用私钥签名更新产物
# 6. 更新产物格式：.tar.gz (macOS) / .zip (Windows) + .sig 签名文件
# 详见 https://tauri.app/plugin/updater/
```

### 外部依赖

| 依赖 | macOS | Windows | 说明 |
|------|-------|---------|------|
| Tesseract | brew install tesseract tesseract-lang | 下载安装包 | 可选，OCR 功能需要 |
| Ollama | brew install ollama | 下载安装包 | 可选，AI 模式需要 |

---

## 九、实施路线图

### Phase 1：数据层 + 案件 CRUD（3 天）
- SQLite schema + 迁移框架
- 案件/客户/日志/庭审/任务/官员 CRUD
- 飞书数据导入脚本
- 单元测试

### Phase 2：核心 UI（4 天）
- 案件列表（分组+筛选+排序+分页）
- 案件详情（三栏布局+自动保存）
- 时间线视图
- 客户管理 + 官方人员通讯录

### Phase 3：期限 + 日历 + 任务（3 天）
- 期限引擎（规则计算+工作日+预警）
- 日历视图（月视图+事件标记）
- 任务四象限
- 提醒通知

### Phase 4：WebDAV 同步（2 天）
- WebDAV 客户端
- VACUUM INTO 安全拷贝
- 启动同步 + 自动PUSH + 冲突处理

### Phase 5：飞书同步（2 天）
- 飞书 API 客户端
- PULL/PUSH 流程
- 限流 + 重试 + 冲突

### Phase 6：收件箱 + 文档解析（3 天）
- 文本提取管道（PDF/DOCX/图片/.eml）
- 传票/口审通知书解析
- AI 分类 + 案件匹配
- 收件箱 UI + 归档流程

### Phase 7：案卷文件管理（2 天）
- 文件夹自动创建 + 分类规则
- 智能命名 + 文件管理 UI
- 知识沉淀流程

### Phase 8：文书工坊（3 天）
- Docsy 集成（IPC 桥接）
- 案件→模板字段映射
- TipTap 编辑器 + 4种补全
- 草稿管理 + DOCX 导出

### Phase 9：邮件 + AI 模式（3 天）
- IMAP 监听 + 邮件解析
- Token 预算保护
- AI 后端抽象层 + Ollama/OpenAI 集成
- 设置页 AI 配置

### Phase 10：关系网络 + 知识库（2 天）
- 案件关系 CRUD + 自动检测
- 知识库 CRUD + FTS5 搜索
- 关联网络 UI

### Phase 11：打磨（3 天）
- 数据统计面板
- 导出（Excel/CSV）
- 全文搜索
- 性能优化
- 文档

**总计：30 天**

---

## 十、补充文档索引

| 文档 | 内容 |
|------|------|
| `Casy-实现规格-数据层.md` | 完整 SQLite schema + Rust 结构体 + 查询目录 |
| `Casy-实现规格-核心模块.md` | 案件/任务/时间线/日历/期限/关系/迁移的完整实现代码 |
| `Casy-实现规格-同步与公式.md` | WebDAV 客户端 + 飞书 API + 公式引擎 |
| `Casy-实现规格-收件箱与解析.md` | 文本提取 + 传票/口审解析 + IMAP + Token 保护 |
| `Casy-实现规格-UI与Copilot.md` | TipTap 扩展 + Pinia stores + 路由 + Docsy 桥接 |
