import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { CalendarEvent, DashboardStats } from '../../types'

/** 日历与期限服务：ctx.calendar */
export class CalendarService extends Service {
  static inject: string[] = []

  async events(year?: number, month?: number): Promise<{ ok: boolean; data?: CalendarEvent[]; error?: string }> {
    return tauriCallSafe<CalendarEvent[]>('get_calendar_events', {
      year: year ?? new Date().getFullYear(),
      month: month ?? new Date().getMonth() + 1,
    })
  }

  async deadlineWarnings(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_deadline_warnings', {})
  }

  async dashboardStats(): Promise<{ ok: boolean; data?: DashboardStats; error?: string }> {
    return tauriCallSafe<DashboardStats>('get_dashboard_stats', {})
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
    return tauriCallSafe<void>('send_ics_invitation_cmd', {
      to: opts.to,
      subject: opts.subject,
      description: opts.description,
      startIso: opts.startIso,
      durationMinutes: opts.durationMinutes,
      alarmMinutes: opts.alarmMinutes,
    })
  }

  async testCaldavConnection(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('test_caldav_connection', {})
  }

  /** CalDAV 同步状态（{ enabled, configured, syncedCount, ... }） */
  async calendarSyncStatus(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_calendar_sync_status', {})
  }

  /** 立即补同步：把提醒推送到日历 */
  async syncRemindersToCalendar(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('sync_reminders_to_calendar', {})
  }

  // ── 分级期限预警与每日早报（HomeView 使用） ──

  /** 分级期限预警（R1-R4，含 days_left / level_label / message） */
  async deadlineWarningsWithLevels(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_deadline_warnings_with_levels', {})
  }

  /** 今日早报（后端规则版 Markdown；smart_summaries 行） */
  async todayBrief(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_today_brief', {})
  }

  /** 重新生成每日早报（返回 DailyBrief，markdown 字段） */
  async generateDailyBrief(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('generate_daily_brief_cmd', {})
  }

  /** 今日智能推荐（get_today_recommendations） */
  async todayRecommendations(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_today_recommendations', {})
  }
}
