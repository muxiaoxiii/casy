# Casy 插件化架构设计文档

> **版本**: v1.0  
> **日期**: 2026-08-18  
> **状态**: 实施中  
> **参考**: Cordis 框架 + DeepSeek Harness + Pi Agent

---

## 一、设计哲学

### 1.1 核心理念

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

## 二、架构概览

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────┐
│ Vue 前端层                                               │
│ 组件 · 视图 · 状态管理                                    │
├─────────────────────────────────────────────────────────┤
│ 插件层                                                   │
│ CasesPlugin · TasksPlugin · KnowledgePlugin · AIPlugin  │
├─────────────────────────────────────────────────────────┤
│ 核心容器层                                               │
│ CasyContext · 事件系统 · 工具注册 · 技能注册              │
├─────────────────────────────────────────────────────────┤
│ AI 层                                                   │
│ 模型提供商 · 统一接口 · 流式输出                          │
├─────────────────────────────────────────────────────────┤
│ 数据层                                                   │
│ Tauri Bridge · SQLite · 文件系统                         │
└─────────────────────────────────────────────────────────┘
```

### 2.2 核心组件

| 组件 | 职责 | 参考 |
|------|------|------|
| **CasyContext** | 插件容器，管理所有插件、工具、技能 | Cordis Context |
| **CasyPlugin** | 插件接口，每个业务模块都是一个插件 | Cordis Plugin |
| **CasyTool** | 工具接口，AI 可调用的能力 | DSH Tool |
| **CasySkill** | 技能接口，法律领域的复合能力 | DSH Skill |
| **ModelProvider** | 模型提供商接口，统一多模型接入 | Pi Agent pi-ai |
| **LawyerProfile** | 律师画像，个性化配置 | DSH Preset |
| **ConfirmPolicy** | 确认策略，AI 输出的安全控制 | Casy 设计哲学 11.4 |

---

## 三、核心容器（CasyContext）

### 3.1 接口定义

```typescript
class CasyContext {
  // 插件管理
  use(plugin: CasyPlugin): Promise<void>
  unuse(name: string): Promise<void>
  getPlugin(name: string): CasyPlugin | undefined
  getPlugins(): CasyPlugin[]
  
  // 工具管理
  registerTool(tool: CasyTool): void
  unregisterTool(name: string): void
  getTool(name: string): CasyTool | undefined
  getTools(filter?: { category?: string }): CasyTool[]
  executeTool(name: string, params: any): Promise<any>
  
  // 技能管理
  registerSkill(skill: CasySkill): void
  unregisterSkill(name: string): void
  getSkill(name: string): CasySkill | undefined
  getSkills(filter?: { category?: string }): CasySkill[]
  findSkillForIntent(intent: string): CasySkill | undefined
  executeSkill(name: string, context: any): Promise<any>
  
  // AI 模型管理
  registerProvider(provider: ModelProvider): void
  getProvider(name: string): ModelProvider | undefined
  getProviders(): ModelProvider[]
  getModels(): { provider: string; model: string }[]
  
  // 事件系统
  on<T>(event: CasyEventType, handler: EventHandler<T>): void
  off<T>(event: CasyEventType, handler: EventHandler<T>): void
  emit<T>(event: CasyEventType, data: T): Promise<void>
  
  // 律师画像
  setProfile(profile: LawyerProfile): void
  getProfile(): LawyerProfile | null
  
  // 确认机制
  setConfirmPolicy(policy: Partial<ConfirmPolicy>): void
  getConfirmPolicy(): ConfirmPolicy
  calculateEffectiveLevel(params: {...}): ConfirmLevel
  requestConfirm(request: ConfirmRequest): Promise<boolean>
  
  // 工具定义（供 AI 使用）
  getToolDefinitions(filter?: { category?: string }): any[]
}
```

### 3.2 使用示例

```typescript
import { casyContext } from '@/core/plugin'

// 安装插件
await casyContext.use(new CasesPlugin())
await casyContext.use(new TasksPlugin())

// 注册工具
casyContext.registerTool({
  name: 'custom_tool',
  description: '自定义工具',
  execute: async (params) => { ... },
})

// 执行工具
const result = await casyContext.executeTool('list_cases', { filter: {} })

// 监听事件
casyContext.on('case:created', (data) => {
  console.log('Case created:', data)
})

// 获取工具定义（供 AI 调用）
const toolDefs = casyContext.getToolDefinitions()
```

---

## 四、插件接口（CasyPlugin）

### 4.1 接口定义

```typescript
interface CasyPlugin {
  name: string
  version: string
  description?: string
  
  install(ctx: CasyContext): void | Promise<void>
  uninstall?(ctx: CasyContext): void | Promise<void>
}
```

### 4.2 实现示例

```typescript
class CasesPlugin implements CasyPlugin {
  name = 'cases'
  version = '1.0.0'
  
  async install(ctx: CasyContext): Promise<void> {
    // 注册工具
    ctx.registerTool({
      name: 'list_cases',
      description: '获取案件列表',
      execute: async (params) => { ... },
    })
    
    // 注册技能
    ctx.registerSkill({
      name: 'case_analysis',
      description: '案件分析',
      canHandle: (intent) => intent.includes('分析'),
      execute: async (context) => { ... },
    })
    
    // 监听事件
    ctx.on('case:created', (data) => { ... })
  }
}
```

---

## 五、工具接口（CasyTool）

### 5.1 接口定义

```typescript
interface CasyTool {
  name: string
  description: string
  category?: string
  parameters?: Record<string, any>  // JSON Schema
  
  execute(params: any, ctx: CasyContext): Promise<any>
}
```

### 5.2 工具分类

| 分类 | 说明 | 示例 |
|------|------|------|
| `cases` | 案件管理 | list_cases, create_case, update_case |
| `tasks` | 任务管理 | list_tasks, create_task, toggle_task |
| `knowledge` | 知识库 | search_knowledge, create_knowledge |
| `calendar` | 日历 | get_calendar_events, create_event |
| `ai` | AI 能力 | classify_document, extract_info |

---

## 六、技能接口（CasySkill）

### 6.1 接口定义

```typescript
interface CasySkill {
  name: string
  description: string
  category: 'contract' | 'litigation' | 'deadline' | 'knowledge' | 'other'
  
  canHandle(intent: string, context?: any): boolean
  execute(context: SkillContext): Promise<SkillResult>
}
```

### 6.2 技能示例

```typescript
const contractReviewSkill: CasySkill = {
  name: 'contract_review',
  description: '合同审查',
  category: 'contract',
  
  canHandle: (intent) => {
    return intent.includes('审查') && intent.includes('合同')
  },
  
  execute: async (context) => {
    // 1. 提取合同内容
    // 2. 调用 AI 分析
    // 3. 生成审查报告
    return { success: true, data: { ... } }
  },
}
```

---

## 七、AI 模型提供商（ModelProvider）

### 7.1 接口定义

```typescript
interface ModelProvider {
  name: string
  models: ModelInfo[]
  
  chat(request: ChatRequest): Promise<ChatResponse>
  stream(request: ChatRequest): AsyncIterable<ChatChunk>
}
```

### 7.2 支持的提供商

| 提供商 | 说明 | 状态 |
|--------|------|------|
| Ollama | 本地模型 | ✅ 已实现 |
| OpenAI | GPT 系列 | ✅ 已实现 |
| DeepSeek | DeepSeek 系列 | ✅ 已实现（通过 OpenAI 兼容接口） |
| 其他 | 任何 OpenAI 兼容 API | ✅ 已实现 |

---

## 八、事件系统

### 8.1 事件类型

```typescript
type CasyEventType = 
  | 'plugin:installed' | 'plugin:uninstalled'
  | 'tool:registered' | 'tool:executed'
  | 'skill:registered' | 'skill:executed'
  | 'ai:request' | 'ai:response' | 'ai:error'
  | 'case:created' | 'case:updated' | 'case:deleted'
  | 'task:created' | 'task:completed' | 'task:overdue'
```

### 8.2 使用示例

```typescript
// 监听事件
casyContext.on('case:created', (data) => {
  console.log('New case created:', data.id)
})

// 触发事件
await casyContext.emit('case:created', { id: '123', name: 'Test Case' })
```

---

## 九、确认机制

### 9.1 确认级别

| 级别 | 场景 | 做法 |
|------|------|------|
| **L1** | 总结/报表/建议 | 生成后进草稿箱，用户看一眼即可 |
| **L2** | AI 提取的结构化信息 | 逐项列出 + 标注置信度 + 用户逐条确认 |
| **L3** | 关键决策/对外文书 | 必须用户手动触发，生成后二次确认 |

### 9.2 有效级别计算

```typescript
effective_policy = max(
  system_minimum,  // 系统安全下限（外部写 = L3）
  scenario,        // 场景风险
  model,           // 模型质量（本地小模型 +1 级）
  user             // 用户设置
)
```

---

## 十、目录结构

```
src/core/
├── plugin/
│   ├── types.ts          # 类型定义
│   ├── context.ts        # 核心容器
│   ├── initializer.ts    # 初始化器
│   └── index.ts          # 入口
├── ai/
│   ├── providers/
│   │   ├── ollama.ts     # Ollama 提供商
│   │   └── openai.ts     # OpenAI 兼容提供商
│   └── index.ts          # 入口
├── plugins/
│   ├── cases-plugin.ts   # 案件管理插件
│   ├── tasks-plugin.ts   # 任务管理插件
│   └── index.ts          # 入口
├── tools/                # 工具定义（待扩展）
├── skills/               # 技能定义（待扩展）
└── tauriBridge.ts        # Tauri 桥接（已有）
```

---

## 十一、迁移策略

### 11.1 渐进式迁移

1. **Phase 1**：实现核心容器和插件系统 ✅
2. **Phase 2**：将现有模块迁移为插件（进行中）
3. **Phase 3**：实现 AI 工具调用流程
4. **Phase 4**：实现技能系统

### 11.2 兼容性保证

- 现有的 Tauri 命令保持不变
- 现有的 Vue 组件保持不变
- 插件系统是新增层，不破坏现有功能

---

## 十二、与现有设计的对齐

| 插件系统特性 | Casy 设计哲学对应 | 对齐状态 |
|-------------|-----------------|---------|
| 工具注册 | 11.6 推荐引擎（工具调用） | ✅ 已实现 |
| 技能注册 | 11.6 推荐引擎（技能执行） | ⚠️ 待实现 |
| 确认机制 | 11.4 AI 输出确认（L1/L2/L3） | ✅ 已实现 |
| 事件系统 | 11.9 数据支撑（事件记录） | ✅ 已实现 |
| 律师画像 | DSH Preset 机制 | ⚠️ 待完善 |
| 模型提供商 | Pi Agent pi-ai 层 | ✅ 已实现 |

---

## 十三、后续计划

1. **完善律师画像**：实现首次使用引导
2. **实现技能系统**：合同审查、诉讼分析等法律技能
3. **实现 AI 工具调用流程**：AI 自动调用工具完成任务
4. **迁移其他模块**：知识库、日历、收件箱等
5. **实现 MCP 接口**：对外暴露 Casy 的能力