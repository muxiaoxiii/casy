import type { CasyContext, CasyPlugin } from '../plugin/types'
import { defineTool } from '../plugin/defineTool'

/** Read access to missing modules and the same connected context shown in the UI. */
export class WorkspacePlugin implements CasyPlugin {
  name = 'workspace'
  version = '1.0.0'
  description = '跨模块上下文、文书、项目与来源读取'
  private names: string[] = []

  install(ctx: CasyContext) {
    const tools = [
      defineTool<{ name: string }>({
        name: 'get_tool_schema', category: 'workspace', description: '按工具名读取完整参数结构（含嵌套字段）。不确定参数时先查询。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { name: { type: 'string' } }, required: ['name'] },
        execute: async p => {
          const tool = ctx.getTool(p.name)
          return tool && typeof tool.policy?.write === 'boolean'
            ? { ok: true, data: { name: tool.name, description: tool.description, parameters: tool.parameters, write: tool.policy.write } }
            : { ok: false, error: '工具不可用' }
        },
      }),
      defineTool<{ query: string }>({
        name: 'search_workspace', category: 'workspace', description: '跨知识笔记与案卷文件检索，返回可供继续读取和引用的来源 ID。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] },
        execute: p => ctx.knowledge.globalSearch(p.query),
      }),
      defineTool<{ caseId: string }>({
        name: 'get_case_timeline', category: 'cases', description: '读取案件时间线，核对任务及事件所处阶段。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { caseId: { type: 'string' } }, required: ['caseId'] },
        execute: p => ctx.cases.timeline(p.caseId),
      }),
      defineTool<{ caseId: string; startDate: string; endDate: string }>({
        name: 'get_case_context', category: 'workspace',
        description: '读取案件的任务、独立日程、关联笔记摘要、文书目录、案卷文件。含各来源失败状态；日期区间只限制日程。用详情工具读取正文，法定期限使用 get_deadline_warnings。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: {
          caseId: { type: 'string', description: '案件 ID' },
          startDate: { type: 'string', description: '日程开始日期 YYYY-MM-DD' },
          endDate: { type: 'string', description: '日程结束日期 YYYY-MM-DD' },
        }, required: ['caseId', 'startDate', 'endDate'] },
        execute: p => ctx.workspace.caseContext(p.caseId, p.startDate, p.endDate),
      }),
      defineTool<{ caseId?: string }>({
        name: 'list_drafts', category: 'docs', description: '读取文书目录与版本，可按案件筛选；正文用 get_draft 读取。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { caseId: { type: 'string' } } },
        execute: async p => {
          const result = await ctx.docs.listDrafts()
          return { ...result, data: result.data?.filter(d => !p.caseId || d.caseId === p.caseId).map(({ content, ...d }) => d) }
        },
      }),
      defineTool<{ id: string; offset?: number }>({
        name: 'get_draft', category: 'docs', description: '分段读取文书已保存正文（每段 6000 字符）、关联案件与版本。有 nextOffset 时继续读取。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { id: { type: 'string' }, offset: { type: 'integer', description: '字符偏移，默认 0' } }, required: ['id'] },
        execute: async p => {
          if ((p.offset ?? 0) < 0) return { ok: false, error: 'offset 不能小于 0' }
          const result = await ctx.docs.getDraft(p.id)
          if (!result.ok || !result.data) return result
          const { content, ...draft } = result.data, offset = p.offset || 0, text = content || ''
          return { ok: true, data: { ...draft, content: text.slice(offset, offset + 6000), offset, totalCharacters: text.length, nextOffset: offset + 6000 < text.length ? offset + 6000 : null } }
        },
      }),
      defineTool<{ id: string; offset?: number }>({
        name: 'get_knowledge_detail', category: 'knowledge', description: '分段读取笔记正文（每段 6000 字符）与关联案件。块正文按块 ID 再读，有 nextOffset 时继续读取。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { id: { type: 'string' }, offset: { type: 'integer', description: '字符偏移，默认 0' } }, required: ['id'] },
        execute: async p => {
          if ((p.offset ?? 0) < 0) return { ok: false, error: 'offset 不能小于 0' }
          const result = await ctx.knowledge.getWithBlocks(p.id)
          if (!result.ok || !result.data) return result
          const { content, ...item } = result.data.item, offset = p.offset || 0
          return { ok: true, data: { item: { ...item, content: content.slice(offset, offset + 6000) }, blocks: result.data.blocks.map(({ content, ...block }) => block), offset, totalCharacters: content.length, nextOffset: offset + 6000 < content.length ? offset + 6000 : null } }
        },
      }),
      defineTool<{ query?: string }>({
        name: 'list_projects', category: 'projects', description: '读取个人项目及案件项目，可按名称搜索。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { query: { type: 'string' } } },
        execute: p => ctx.projects.list(p.query),
      }),
      defineTool<{ startDate: string; endDate: string }>({
        name: 'list_calendar_events', category: 'calendar', description: '读取区间内独立日程、关联 taskId/caseId、起止时刻和地点。',
        policy: { write: false, level: 'L1' },
        parameters: { type: 'object', properties: { startDate: { type: 'string' }, endDate: { type: 'string' } }, required: ['startDate', 'endDate'] },
        execute: p => ctx.calendar.listEvents(p.startDate, p.endDate),
      }),
      defineTool<{}>({
        name: 'list_backup_metadata', category: 'backup', description: '读取备份文件目录、时间与大小。',
        policy: { write: false, level: 'L1' }, parameters: { type: 'object', properties: {} },
        execute: () => ctx.backup.list(),
      }),
    ]
    this.names = tools.map(tool => tool.name)
    tools.forEach(tool => ctx.registerTool(tool))
  }
  uninstall(ctx: CasyContext) { this.names.forEach(name => ctx.unregisterTool(name)) }
}
