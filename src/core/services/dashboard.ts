import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { MonthTrendPoint, NameCount, UpcomingHearing } from '../../types/bindings'

/** 仪表盘聚合服务（B4 · 数据可视化）：全部只读 GROUP BY，经后端聚合避免前端拉全量 */
export class DashboardService extends Service {
  static inject: string[] = []

  async projectStatusDistribution() {
    return tauriCallSafe('get_project_status_distribution', {})
  }

  async trackDistribution() {
    return tauriCallSafe('get_track_distribution', {})
  }

  async monthlyTaskTrend(months?: number) {
    return tauriCallSafe('get_monthly_task_trend', months ? { months } : {})
  }

  async upcomingHearings(days?: number) {
    return tauriCallSafe('get_upcoming_hearings', days ? { days } : {})
  }
}
