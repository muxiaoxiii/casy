/**
 * 业务服务注册（cordis 数据通路）
 *
 * 对标 DeepSeek Harness / @deepseek-ai/cordis：
 * 每个业务模块是一个 Service，注册后可通过 ctx.<name> 访问——
 *   ctx.cases.list() / ctx.tasks.create() / ctx.knowledge.search() / ...
 * 服务内部封装 tauriBridge（写入口唯一，双路径铁律），视图/插件/AI 共用此通路。
 *
 * 类型增强：下面 declare module 让 ctx.cases 等属性获得完整类型提示。
 */

import { casyContext } from '../plugin/context'
import { CasesService } from './cases'
import { TasksService } from './tasks'
import { KnowledgeService } from './knowledge'
import { CalendarService } from './calendar'
import { InboxService } from './inbox'
import { ReminderService } from './reminder'
import { FilesService } from './files'
import { SyncService } from './sync'
import { SettingsService } from './settings'
import { AiService } from './ai'
import { DocsService } from './docs'

declare module '../plugin/types' {
  interface CasyContext {
    cases: CasesService
    tasks: TasksService
    knowledge: KnowledgeService
    calendar: CalendarService
    inbox: InboxService
    reminder: ReminderService
    files: FilesService
    sync: SyncService
    settings: SettingsService
    ai: AiService
    docs: DocsService
  }
}

/**
 * 注册全部业务服务到 CasyContext。
 * 依赖顺序：settings 无依赖先注册；其余服务若有跨模块依赖可在 inject 声明。
 */
export async function registerServices(): Promise<void> {
  casyContext.provide('settings', new SettingsService(casyContext), [])
  casyContext.provide('cases', new CasesService(casyContext), ['settings'])
  casyContext.provide('tasks', new TasksService(casyContext), ['cases'])
  casyContext.provide('knowledge', new KnowledgeService(casyContext), [])
  casyContext.provide('calendar', new CalendarService(casyContext), [])
  casyContext.provide('inbox', new InboxService(casyContext), ['cases', 'knowledge'])
  casyContext.provide('reminder', new ReminderService(casyContext), ['cases', 'tasks', 'calendar'])
  casyContext.provide('files', new FilesService(casyContext), ['cases'])
  casyContext.provide('sync', new SyncService(casyContext), ['settings'])
  casyContext.provide('ai', new AiService(casyContext), [])
  casyContext.provide('docs', new DocsService(casyContext), [])

  casyContext.logger.info(
    '业务服务已注册: ' + casyContext.getServiceNames().join(', ')
  )
}

export {
  CasesService,
  TasksService,
  KnowledgeService,
  CalendarService,
  InboxService,
  ReminderService,
  FilesService,
  SyncService,
  SettingsService,
  AiService,
  DocsService,
}
