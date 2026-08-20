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
}
