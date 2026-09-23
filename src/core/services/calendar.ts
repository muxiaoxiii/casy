import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { CalendarEvent, DashboardStats } from '../../types'
import type { TaskPlanInput, CalendarEventRow, CalendarSyncReport, DeadlineResult, DeadlineWarning } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'
import type { CalendarEventInput, HolidayCalendarPayload } from '../../types/ipc'

type CalendarSyncStatus = CommandMap['get_calendar_sync_status']['result']
type SmartSummaryRow = CommandMap['get_today_brief']['result']
type DailyBriefPayload = CommandMap['generate_daily_brief_cmd']['result']
type TodayRecommendations = CommandMap['get_today_recommendations']['result']

/** 日历与期限服务：ctx.calendar */
export class CalendarService extends Service {
  static inject: string[] = []

  async events(year?: number, month?: number, monthCount?: number): Promise<{ ok: boolean; data?: CalendarEvent[]; error?: string }> {
    const result = await tauriCallSafe('get_calendar_events', {
      monthCount,
      year: year ?? new Date().getFullYear(),
      month: month ?? new Date().getMonth() + 1,
    })
    if (!result.ok || !result.data) return { ok: false, error: result.error }
    return {
      ok: true,
      data: result.data.map(event => ({
        ...event,
        type: event.eventType,
        color: event.eventType === 'hearing' || event.eventType === 'deadline_red'
          ? 'var(--c-danger)' : event.eventType.startsWith('deadline') ? 'var(--c-warning)' : 'var(--c-primary)',
      })),
    }
  }

  // ── D-7 独立日程（calendar_events · M-CAL-1）──

  /** 区间查询独立日程 */
  async listEvents(startDate: string, endDate: string): Promise<{ ok: boolean; data?: CalendarEventRow[]; error?: string }> {
    return tauriCallSafe('list_calendar_events', { startDate, endDate })
  }

  /** 新建独立日程。data: { title, eventDate, startTime?, endTime?, allDay?, color?, location?, notes?, caseId?, taskId? } */
  async createEvent(data: CalendarEventInput): Promise<{ ok: boolean; data?: CalendarEventRow; error?: string }> {
    return this.mutation('calendar', tauriCallSafe('create_calendar_event', { data }))
  }

  /**
   * 更新日程：title/eventDate 必传，缺省字段保留，null 显式清空。
   */
  async updateEvent(id: string, data: CalendarEventInput): Promise<{ ok: boolean; error?: string }> {
    return this.mutation('calendar', tauriCallSafe('update_calendar_event', { id, data }))
  }

  /** 拖拽改期/改时刻 */
  async moveEvent(id: string, newDate: string, newStart?: string | null): Promise<{ ok: boolean; error?: string }> {
    return this.mutation('calendar', tauriCallSafe('move_calendar_event', { id, newDate, newStart: newStart ?? null }))
  }

  async removeEvent(id: string): Promise<{ ok: boolean; error?: string }> {
    return this.mutation('calendar', tauriCallSafe('delete_calendar_event', { id }))
  }

  async taskPlans() { return tauriCallSafe('list_task_plans', {}) }

  async saveTaskPlan(data: TaskPlanInput) {
    const result = await this.mutation('plan', tauriCallSafe('save_task_plan', { data }))
    if (result.ok) this.ctx.emit('task:changed', { id: data.taskId })
    return result
  }

  async deadlineWarnings(): Promise<{ ok: boolean; data?: DeadlineResult[]; error?: string }> {
    return tauriCallSafe('get_deadline_warnings', {})
  }

  /** 中国法定节假日与调休工作日（内置数据 + 收件箱确认导入）。 */
  async holidays(year: number): Promise<{ ok: boolean; data?: HolidayCalendarPayload; error?: string }> {
    return tauriCallSafe('get_holiday_calendar', { year })
  }

  async dashboardStats(): Promise<{ ok: boolean; data?: DashboardStats; error?: string }> {
    return tauriCallSafe('get_dashboard_stats', {})
  }

  // ── 日历同步（CalDAV / SMTP·ICS 邀请） ──

  /** 发送 SMTP / ICS 日历邀请 */
  async sendIcsInvitation(opts: {
    to: string
    subject: string
    description: string
    startIso: string
    durationMinutes: number
    alarmMinutes: number
  }): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('send_ics_invitation_cmd', {
      to: opts.to,
      subject: opts.subject,
      description: opts.description,
      startIso: opts.startIso,
      durationMinutes: opts.durationMinutes,
      alarmMinutes: opts.alarmMinutes,
    })
  }

  async testCaldavConnection(): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('test_caldav_connection', {})
  }

  /** CalDAV 同步状态（{ enabled, configured, syncedCount, ... }） */
  async calendarSyncStatus(): Promise<{ ok: boolean; data?: CalendarSyncStatus; error?: string }> {
    return tauriCallSafe('get_calendar_sync_status', {})
  }

  /** 立即补同步：把提醒推送到日历 */
  async syncRemindersToCalendar(): Promise<{ ok: boolean; data?: CalendarSyncReport; error?: string }> {
    return tauriCallSafe('sync_reminders_to_calendar', {})
  }

  // ── 分级期限预警与每日早报（HomeView 使用） ──

  /** 分级期限预警（R1-R4，含 days_left / level_label / message） */
  async deadlineWarningsWithLevels(): Promise<{ ok: boolean; data?: DeadlineWarning[]; error?: string }> {
    return tauriCallSafe('get_deadline_warnings_with_levels', {})
  }

  /** 今日早报（后端规则版 Markdown；smart_summaries 行） */
  async todayBrief(): Promise<{ ok: boolean; data?: SmartSummaryRow; error?: string }> {
    return tauriCallSafe('get_today_brief', {})
  }

  /** 重新生成每日早报（返回 DailyBrief，markdown 字段） */
  async generateDailyBrief(): Promise<{ ok: boolean; data?: DailyBriefPayload; error?: string }> {
    return tauriCallSafe('generate_daily_brief_cmd', {})
  }

  /** 今日智能推荐（get_today_recommendations） */
  async todayRecommendations(): Promise<{ ok: boolean; data?: TodayRecommendations; error?: string }> {
    return tauriCallSafe('get_today_recommendations', {})
  }
}
