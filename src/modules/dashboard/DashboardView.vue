<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { Refresh, ArrowRight, TrendCharts, PieChart, Histogram, Calendar } from '../../shared/icons'
import { useRouter } from 'vue-router'
import { casyContext } from '../../core/plugin/context'
import type { MonthTrendPoint, NameCount, TodayKpis, UpcomingHearing } from '../../types/bindings'
import AreaLineChart from '../../shared/charts/AreaLineChart.vue'
import DonutChart from '../../shared/charts/DonutChart.vue'
import HBarChart from '../../shared/charts/HBarChart.vue'
import GanttTimeline from '../../shared/charts/GanttTimeline.vue'

const router = useRouter()
const trend = ref<MonthTrendPoint[]>([])
const statusDist = ref<NameCount[]>([])
const trackDist = ref<NameCount[]>([])
const hearings = ref<UpcomingHearing[]>([])
const kpis = ref<TodayKpis | null>(null)
const loading = ref(false)
const months = ref(6)
const updatedAt = ref('')
const errors = ref<Record<string, string>>({})
const now = ref(new Date())
let refreshQueued = false
let active = true
let dayTimer: ReturnType<typeof setInterval>
const unlisteners: Array<() => void> = []
const statusLabels: Record<string, string> = { active: '在办案件', done: '已结案件' }
const statusColors: Record<string, string> = { active: 'var(--c-primary)', paused: 'var(--c-warning)', done: 'var(--c-success)', archived: 'var(--c-text-secondary)' }
const tracks: Record<string, { label: string; color: string }> = {
  patent_invalidation: { label: '专利无效', color: 'var(--c-info)' },
  civil_tort: { label: '民事诉讼', color: 'var(--c-primary)' },
  admin_litigation: { label: '行政诉讼', color: 'var(--c-warning)' },
  arbitration: { label: '仲裁', color: 'var(--c-success)' },
}
const statusDonutData = computed(() => statusDist.value.map(d => ({
  ...d, key: d.label, label: statusLabels[d.label] || d.label,
  color: statusColors[d.label] || 'var(--c-text-secondary)',
})))
const trackData = computed(() => trackDist.value.map(d => ({
  ...d, key: d.label, label: tracks[d.label]?.label || d.label, color: tracks[d.label]?.color || 'var(--c-info)',
})))
const metrics = computed(() => [
  { key: 'todayEvents', label: '今日庭审', value: kpis.value?.todayEvents, color: 'var(--c-primary)', target: { name: 'calendar', query: { view: 'day' } } },
  { key: 'dueToday', label: '今日到期', value: kpis.value?.dueToday, color: 'var(--c-text)', target: { name: 'tasks', query: { tab: 'all', metric: 'dueToday' } } },
  { key: 'waitingOverdue', label: '等待超时', value: kpis.value?.waitingOverdue, color: 'var(--c-warning)', target: { name: 'tasks', query: { tab: 'waiting', metric: 'waitingOverdue' } } },
  { key: 'reviewDue', label: '需回顾', value: kpis.value?.reviewDue, color: 'var(--c-danger)', target: { name: 'tasks', query: { tab: 'review' } } },
])

// Settle sections independently so one unavailable service cannot erase other results.
async function loadPart<T>(key: string, request: () => Promise<{ ok: boolean; data?: T; error?: string }>, assign: (data: T) => void) {
  try {
    const result = await request()
    if (!result.ok || result.data == null) throw new Error(result.error || '数据暂不可用')
    assign(result.data)
  } catch (error) {
    errors.value[key] = error instanceof Error ? error.message : '加载失败'
  }
}
async function load() {
  if (!active) return
  if (loading.value) { refreshQueued = true; return }
  loading.value = true
  errors.value = {}
  kpis.value = null
  trend.value = []; statusDist.value = []; trackDist.value = []; hearings.value = []
  await Promise.all([
    loadPart('trend', () => casyContext.dashboard.monthlyTaskTrend(months.value), data => { trend.value = data }),
    loadPart('status', () => casyContext.cases.stats(), data => { statusDist.value = [{ label: 'active', value: data.active }, { label: 'done', value: data.closed }] }),
    loadPart('track', () => casyContext.dashboard.trackDistribution(), data => { trackDist.value = data }),
    loadPart('hearings', () => casyContext.dashboard.upcomingHearings(30), data => { hearings.value = data }),
    loadPart('kpis', () => casyContext.dashboard.todayKpis(), data => { kpis.value = data }),
  ])
  updatedAt.value = new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  loading.value = false
  if (refreshQueued) { refreshQueued = false; await load() }
}
onMounted(() => {
  void load()
  for (const event of ['inbox:confirmed', 'case:created', 'case:updated', 'case:deleted', 'case:imported', 'task:created', 'task:completed']) {
    unlisteners.push(casyContext.on(event, () => { void load() }))
  }
  dayTimer = setInterval(() => {
    const date = new Date()
    if (date.toDateString() !== now.value.toDateString()) void load()
    now.value = date
  }, 60_000)
})
onUnmounted(() => { active = false; clearInterval(dayTimer); unlisteners.forEach(off => off()) })
</script>

<template>
  <div class="dash-page" :aria-busy="loading">
    <header class="dash-header">
      <div><h1>数据看板</h1><p>{{ now.toLocaleDateString('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }) }}</p></div>
      <div class="dash-actions">
        <span v-if="updatedAt && !loading" class="updated-at" role="status">{{ Object.keys(errors).length ? '部分数据未更新' : `更新于 ${updatedAt}` }}</span>
        <el-button :icon="Refresh" :loading="loading" aria-label="刷新看板" title="刷新看板" @click="load" circle />
      </div>
    </header>
    <div v-if="errors.kpis" class="data-error" role="alert">今日指标加载失败 <el-button text @click="load">重试</el-button></div>
    <div class="kpi-row">
      <button v-for="metric in metrics" :key="metric.key" type="button" class="kpi-card" :disabled="metric.value == null" @click="router.push(metric.target)">
        <span class="kk">{{ metric.label }}<el-icon><ArrowRight /></el-icon></span>
        <span class="kv" :style="{ color: metric.color }">{{ metric.value ?? '待加载' }}<small v-if="metric.value != null">项</small></span>
      </button>
    </div>
    <div class="charts-grid">
      <section class="chart-section">
        <header class="section-header"><h2><el-icon><TrendCharts /></el-icon>月度任务趋势</h2>
          <el-radio-group v-model="months" size="small" :disabled="loading" aria-label="统计周期" @change="load">
            <el-radio-button :value="6">6 个月</el-radio-button><el-radio-button :value="12">12 个月</el-radio-button>
          </el-radio-group>
        </header>
        <el-skeleton v-if="loading" :rows="4" animated />
        <div v-else-if="errors.trend" class="data-error" role="alert">任务趋势加载失败 <el-button text @click="load">重试</el-button></div>
        <AreaLineChart v-else-if="trend.length" :data="trend" :height="210" />
        <div v-else class="chart-empty">近 {{ months }} 个月暂无任务记录</div>
      </section>
      <section class="chart-section">
        <header class="section-header"><h2><el-icon><PieChart /></el-icon>案件状态</h2><el-button text @click="router.push('/cases')">全部案件<el-icon><ArrowRight /></el-icon></el-button></header>
        <el-skeleton v-if="loading" :rows="4" animated />
        <div v-else-if="errors.status" class="data-error" role="alert">案件统计加载失败 <el-button text @click="load">重试</el-button></div>
        <DonutChart v-else-if="statusDonutData.some(d => d.value > 0)" :data="statusDonutData" :size="170" center-sub="案件总数" :interactive="false" />
        <div v-else class="chart-empty">暂无案件</div>
      </section>
      <section class="chart-section">
        <header class="section-header"><h2><el-icon><Histogram /></el-icon>案件轨道分布</h2><span v-if="!loading && !errors.track" class="section-meta">{{ trackDist.reduce((sum, d) => sum + d.value, 0) }} 件</span></header>
        <el-skeleton v-if="loading" :rows="3" animated />
        <div v-else-if="errors.track" class="data-error" role="alert">案件统计加载失败 <el-button text @click="load">重试</el-button></div>
        <HBarChart v-else :data="trackData" @select="item => router.push({ name: 'cases', query: { track: item.key } })" />
      </section>
      <section class="chart-section">
        <header class="section-header"><h2><el-icon><Calendar /></el-icon>近期庭审</h2><span class="section-meta">未来 30 天<span v-if="!loading && !errors.hearings"> · {{ hearings.length }} 场</span></span></header>
        <el-skeleton v-if="loading" :rows="3" animated />
        <div v-else-if="errors.hearings" class="data-error" role="alert">庭审排期加载失败 <el-button text @click="load">重试</el-button></div>
        <GanttTimeline v-else-if="hearings.length" :items="hearings" :window-days="30" @select="item => router.push(item.caseId ? { name: 'case-detail', params: { id: item.caseId } } : { name: 'calendar' })" />
        <div v-else class="chart-empty">未来 30 天暂无庭审安排</div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.dash-page { padding: 28px 32px 48px; max-width: 1320px; margin: 0 auto; }
.dash-header, .dash-actions, .section-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.dash-header { margin-bottom: 24px; }
.dash-header h1 { font-size: 24px; line-height: 1.4; margin: 0; color: var(--c-text-heading); }
.dash-header p, .updated-at, .section-meta { font-size: 12px; color: var(--c-text-secondary); }
.dash-header p { margin-top: 4px; }
.kpi-row { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); border-block: 1px solid var(--c-border); margin-bottom: 12px; }
.kpi-card { border: 0; border-right: 1px solid var(--c-border); background: transparent; text-align: left; font: inherit; padding: 20px; min-width: 0; cursor: pointer; transition: background var(--motion-base); }
.kpi-card:first-child { padding-left: 0; }
.kpi-card:last-child { border-right: 0; }
.kpi-card:hover:not(:disabled) { background: var(--c-bg-hover); }
.kpi-card:disabled { cursor: default; }
.kk { display: flex; justify-content: space-between; align-items: center; gap: 8px; font-size: 13px; color: var(--c-text-secondary); }
.kk .el-icon { opacity: .5; }
.kv { display: block; font-size: 32px; line-height: 1.2; font-weight: 650; font-variant-numeric: tabular-nums; margin-top: 12px; }
.kpi-card:disabled .kv { font-size: 16px; }
.kv small { font-size: 12px; font-weight: 400; margin-left: 8px; color: var(--c-text-secondary); }
.charts-grid { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr); column-gap: 32px; }
.chart-section { min-width: 0; padding: 24px 0; border-bottom: 1px solid var(--c-border); min-height: 230px; }
.section-header { min-height: 32px; margin-bottom: 20px; flex-wrap: wrap; }
.section-header h2 { display: flex; align-items: center; gap: 8px; font-size: 14px; font-weight: 600; margin: 0; }
.section-header h2 .el-icon { color: var(--c-text-secondary); }
.chart-empty, .data-error { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 160px; font-size: 13px; color: var(--c-text-secondary); }
.data-error { color: var(--c-danger); }
.dash-page > .data-error { min-height: 40px; justify-content: flex-start; }
@media (max-width: 1100px) { .charts-grid { grid-template-columns: minmax(0, 1fr); } }
@media (max-width: 600px) {
  .dash-page { padding: 20px 16px 32px; }
  .dash-header h1 { font-size: 22px; }
  .updated-at { display: none; }
  .kpi-row { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .kpi-card, .kpi-card:first-child { padding: 16px 12px; }
  .kpi-card:nth-child(2) { border-right: 0; }
  .kpi-card:nth-child(-n+2) { border-bottom: 1px solid var(--c-border); }
}
</style>
