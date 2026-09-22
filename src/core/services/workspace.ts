import { Service } from '../plugin/types'

/** A live projection over the source modules, never a second copy of their records. */
export class WorkspaceService extends Service {
  async caseContext(caseId: string, startDate: string, endDate: string) {
    const [matter, tasks, calendar, knowledge, docs, files] = await Promise.all([
      this.ctx.cases.get(caseId),
      this.ctx.tasks.list({ caseId }),
      this.ctx.calendar.listEvents(startDate, endDate),
      this.ctx.knowledge.list({ caseId }),
      this.ctx.docs.listDrafts(),
      this.ctx.files.list(caseId),
    ])
    const sections = { matter, tasks, calendar, knowledge, docs, files }
    const errors = Object.entries(sections).filter(([, result]) => !result.ok)
      .map(([source, result]) => ({ source, error: result.error || '读取失败' }))
    return {
      ok: matter.ok,
      error: matter.error,
      data: {
        caseId, startDate, endDate, errors,
        matter: matter.data,
        tasks: tasks.data || [],
        calendar: (calendar.data || []).filter(event => event.caseId === caseId),
        knowledge: (knowledge.data || []).map(({ content, ...note }) => ({ ...note, excerpt: content.slice(0, 240) })),
        docs: (docs.data || []).filter(doc => doc.caseId === caseId).map(({ content, ...doc }) => doc),
        files: files.data || [],
      },
    }
  }
}
