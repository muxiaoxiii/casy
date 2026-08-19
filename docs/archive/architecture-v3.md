# Casy 架构设计 v3.0

> **版本**: v3.0  
> **日期**: 2026-08-18  
> **状态**: 插件化架构已完成  
> **关联**: `docs/architecture-plugin-system.md` / `docs/casy-design-philosophy.md` / `docs/modules/`

---

## 一、架构概述

### 1.1 核心理念

Casy v3.0 采用**插件化架构**，借鉴 Cordis 框架和 DeepSeek Harness 的设计：

```
Cordis: "Everything is a Plugin"
DSH: "Every tool is a Cordis plugin"
Casy: "Every module is a plugin, every AI capability is a tool"
```

### 1.2 设计原则

1. **模块化** — 每个业务模块都是独立的插件
2. **可扩展** — 新功能通过插件添加，不修改核心
3. **AI 原生** — 工具/技能机制让 AI 能力自然集成
4. **可测试** — 依赖注入，容易 mock
5. **事件驱动** — 插件间通过事件通信，松耦合

---

## 二、系统分层

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────┐
│ Vue 前端层                                               │
│ 组件 · 视图 · 状态管理                                    │
├─────────────────────────────────────────────────────────┤
│ 插件层                                                   │
│ CasesPlugin · TasksPlugin · KnowledgePlugin · ...       │
├─────────────────────────────────────────────────────────┤
│ 核心容器层                                               │
│ CasyContext · 事件系统 · 工具注册 · 技能注册              │
├─────────────────────────────────────────────────────────┤
│ AI 层                                                   │
│ 模型提供商 · 统一接口 · 工具调用                          │
├─────────────────────────────────────────────────────────┤
│ 数据层                                                   │
│ Tauri Bridge · SQLite · 文件系统                         │
└─────────────────────────────────────────────────────────┘
```

### 2.2 各层职责

| 层 | 职责 | 关键组件 |
|---|------|---------|
| **Vue 前端层** | 用户界面、交互逻辑 | Vue 3 + Element Plus + Pinia |
| **插件层** | 业务逻辑封装 | 9 个业务插件 |
| **核心容器层** | 插件管理、工具注册、事件系统 | CasyContext |
| **AI 层** | 模型接入、工具调用 | OllamaProvider, OpenAIProvider, AIToolCaller |
| **数据层** | 数据持久化、外部服务 | Tauri Bridge, SQLite |

---

## 三、核心容器（CasyContext）

### 3.1 接口定义

```typescript
class CasyContext {
  // 插件管理
  use(plugin: CasyPlugin): Promise<void>
  unuse(name: string): Promise<void>
  
  // 工具管理
  registerTool(tool: CasyTool): void
  executeTool(name: string, params: any): Promise<any>
  getToolDefinitions(): any[]
  
  // 技能管理
  registerSkill(skill: CasySkill): void
  executeSkill(name: string, context: any): Promise<any>
  
  // AI 管理
  registerProvider(provider: ModelProvider): void
  getModels(): { provider: string; model: string }[]
  
  // 事件系统
  on(event: CasyEventType, handler: EventHandler): void
  emit(event: CasyEventType, data: any): Promise<void>
  
  // 画像管理
  setProfile(profile: LawyerProfile): void
  getProfile(): LawyerProfile | null
  
  // 确认机制
  calculateEffectiveLevel(params: {...}): ConfirmLevel
  requestConfirm(request: ConfirmRequest): Promise<boolean>
}
```

### 3.2 使用方式

```typescript
import { casyContext } from '@/core/plugin'

// 安装插件
await casyContext.use(new CasesPlugin())

// 执行工具
const result = await casyContext.executeTool('list_cases', { filter: {} })

// 监听事件
casyContext.on('case:created', (data) => {
  console.log('Case created:', data)
})
```

---

## 四、插件系统

### 4.1 插件接口

```typescript
interface CasyPlugin {
  name: string
  version: string
  description?: string
  
  install(ctx: CasyContext): void | Promise<void>
  uninstall?(ctx: CasyContext): void | Promise<void>
}
```

### 4.2 已注册插件（9 个）

| 插件 | 版本 | 工具数 | 说明 |
|------|------|--------|------|
| CasesPlugin | 1.0.0 | 6 | 案件管理 |
| TasksPlugin | 1.0.0 | 5 | 任务管理 |
| KnowledgePlugin | 1.0.0 | 5 | 知识库 |
| CalendarPlugin | 1.0.0 | 3 | 日历与期限 |
| InboxPlugin | 1.0.0 | 5 | 收件箱 |
| ReminderPlugin | 1.0.0 | 4 | 提醒 |
| FilesPlugin | 1.0.0 | 3 | 文件管理 |
| SyncPlugin | 1.0.0 | 4 | 同步 |
| SettingsPlugin | 1.0.0 | 3 | 设置 |

### 4.3 工具清单（38 个）

#### 案件管理（6 个）
- `list_cases` — 获取案件列表
- `get_case` — 获取案件详情
- `create_case` — 创建案件
- `update_case` — 更新案件
- `delete_case` — 删除案件（需 L3 确认）
- `search_cases` — 搜索案件

#### 任务管理（5 个）
- `list_tasks` — 获取任务列表
- `create_task` — 创建任务
- `toggle_task` — 切换完成状态
- `update_task` — 更新任务
- `delete_task` — 删除任务

#### 知识库（5 个）
- `list_knowledge` — 获取知识列表
- `search_knowledge` — 搜索知识
- `create_knowledge` — 创建知识
- `update_knowledge` — 更新知识
- `delete_knowledge` — 删除知识

#### 日历与期限（3 个）
- `get_calendar_events` — 获取日历事件
- `get_deadline_warnings` — 获取期限预警
- `get_dashboard_stats` — 获取仪表盘统计

#### 收件箱（5 个）
- `list_inbox_items` — 获取收件箱列表
- `add_inbox_item` — 添加收件箱条目
- `process_inbox_item` — 处理收件箱条目
- `file_inbox_item` — 归档到案件
- `dismiss_inbox_item` — 忽略收件箱条目

#### 提醒（4 个）
- `list_reminder_rules` — 获取提醒规则
- `create_reminder_rule` — 创建提醒规则
- `get_reminder_log` — 获取提醒日志
- `start_reminder_engine` — 启动提醒引擎

#### 文件管理（3 个）
- `list_case_files` — 获取案件文件列表
- `add_case_file` — 上传文件到案件
- `delete_case_file` — 删除案件文件

#### 同步（4 个）
- `get_sync_status` — 获取同步状态
- `test_webdav_connection` — 测试 WebDAV 连接
- `manual_sync_push` — 手动推送同步
- `manual_sync_pull` — 手动拉取同步

#### 设置（3 个）
- `get_settings` — 获取设置
- `save_settings` — 保存设置
- `configure_ai` — 配置 AI 后端

---

## 五、AI 系统

### 5.1 模型提供商

| 提供商 | 说明 | 状态 |
|--------|------|------|
| Ollama | 本地模型 | ✅ 已实现 |
| OpenAI | GPT 系列 | ✅ 已实现 |
| DeepSeek | DeepSeek 系列 | ✅ 已实现 |
| 其他 | 任何 OpenAI 兼容 API | ✅ 已实现 |

### 5.2 AI 工具调用流程

```
用户输入 → AI 分析意图 → 选择工具 → 确认策略 → 执行工具 → 返回结果
```

### 5.3 确认机制

| 级别 | 场景 | 做法 |
|------|------|------|
| **L1** | 查询操作 | 直接执行 |
| **L2** | 创建/更新操作 | 需要用户确认 |
| **L3** | 删除/关键操作 | 需要二次确认 |

有效级别计算：
```
effective_policy = max(system_minimum, scenario, model, user)
```

---

## 六、事件系统

### 6.1 事件类型

```typescript
type CasyEventType = 
  | 'plugin:installed' | 'plugin:uninstalled'
  | 'tool:registered' | 'tool:executed'
  | 'skill:registered' | 'skill:executed'
  | 'ai:request' | 'ai:response' | 'ai:error'
  | 'case:created' | 'case:updated' | 'case:deleted'
  | 'task:created' | 'task:completed' | 'task:overdue'
```

### 6.2 使用示例

```typescript
// 监听事件
casyContext.on('case:created', (data) => {
  console.log('New case created:', data.id)
})

// 触发事件
await casyContext.emit('case:created', { id: '123', name: 'Test Case' })
```

---

## 七、目录结构

```
src/
├── core/
│   ├── plugin/
│   │   ├── types.ts          # 类型定义
│   │   ├── context.ts        # 核心容器
│   │   ├── initializer.ts    # 初始化器
│   │   └── index.ts          # 入口
│   ├── ai/
│   │   ├── providers/
│   │   │   ├── ollama.ts     # Ollama 提供商
│   │   │   └── openai.ts     # OpenAI 兼容提供商
│   │   ├── tool-caller.ts    # AI 工具调用服务
│   │   └── index.ts          # 入口
│   ├── plugins/
│   │   ├── cases-plugin.ts   # 案件管理插件
│   │   ├── tasks-plugin.ts   # 任务管理插件
│   │   ├── knowledge-plugin.ts # 知识库插件
│   │   ├── calendar-plugin.ts # 日历插件
│   │   ├── inbox-plugin.ts   # 收件箱插件
│   │   ├── reminder-plugin.ts # 提醒插件
│   │   ├── files-plugin.ts   # 文件管理插件
│   │   ├── sync-plugin.ts    # 同步插件
│   │   ├── settings-plugin.ts # 设置插件
│   │   └── index.ts          # 入口
│   └── tauriBridge.ts        # Tauri 桥接
├── modules/
│   ├── ai/
│   │   ├── views/
│   │   │   ├── AICompanionView.vue # AI 智伴主页面
│   │   │   ├── AIAuditView.vue     # 审计日志
│   │   │   └── DecisionsView.vue   # 决策记录
│   │   └── components/
│   │       ├── AIChatPanel.vue     # AI 对话面板
│   │       └── ConfirmDialog.vue   # 确认对话框
│   ├── cases/
│   ├── tasks/
│   ├── knowledge/
│   ├── calendar/
│   ├── inbox/
│   ├── reminder/
│   └── settings/
├── stores/                   # Pinia 状态管理
├── shared/
│   └── components/           # 共享组件
└── main.js                   # 应用入口（初始化插件系统）
```

---

## 八、与旧架构的对比

| 维度 | v2.x 架构 | v3.0 插件化架构 |
|------|----------|----------------|
| 模块组织 | 按目录划分 | 插件化封装 |
| 工具注册 | Tauri 命令直接调用 | 统一工具注册表 |
| AI 集成 | 独立 AI 模块 | 工具调用机制 |
| 事件通信 | Pinia store watch | 统一事件系统 |
| 确认机制 | 分散实现 | 统一确认策略 |
| 可扩展性 | 修改核心代码 | 插件注册 |

---

## 九、后续计划

### 9.1 短期（P2）

1. **实现技能系统** — 合同审查、诉讼分析等法律技能
2. **完善律师画像** — 首次使用引导
3. **实现 MCP 接口** — 对外暴露 Casy 的能力

### 9.2 中期（P3）

1. **AI 推荐引擎** — 今日任务推荐、优先级排序
2. **自动报表** — 日/周/月报表生成
3. **数据蒸馏** — L2 蒸馏 + 确认区

### 9.3 长期（P4）

1. **双向开放** — MCP Server + Skill Runner
2. **移动端伴侣** — 仅当移动办公需求成立时立项

---

## 十、验收标准

### 10.1 插件系统验收

- [x] 核心容器（CasyContext）实现
- [x] 9 个业务插件注册
- [x] 38 个工具注册
- [x] 事件系统实现
- [x] 确认机制实现

### 10.2 AI 系统验收

- [x] 多模型提供商支持
- [x] AI 工具调用流程
- [x] AI 对话面板
- [x] 审计日志记录

### 10.3 文档验收

- [x] 架构设计文档
- [x] 插件系统文档
- [x] 开发日志记录
- [ ] 模块文档更新（待完成）