<script setup>
/**
 * Dashboard —— 数据可视化仪表盘（B4 · 对标 index-v2 dashboard 设计）
 *
 * 四图布局：月度任务趋势(面积线) · 案件状态(环图) · 轨道分布(横条) · 近期庭审(甘特)
 * 数据全部来自后端聚合命令（dashboard 服务），零前端拉全量。
 */
import { ref, onMounted, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { TrendCharts, PieChart, Histogram, Calendar } from '@element-plus/icons-vue'
import { casyContext } from '../../core/plugin/context'
import AreaLineChart from '../../shared/charts/AreaLineChart.vue'
import DonutChart from '../../shared/charts/DonutChart.vue'
import HBarChart from '../../shared/charts/HBarChart.vue'
import GanttTimeline from '../../shared/charts/GanttTimeline.vue'

const trend = ref([])
const statusDist = ref([])
const trackDist = ref([])
const hearings = ref([])
const loading = ref(true)

// 状态 → 语义色（Slate）
const STATUS_COLORS = {
  active: '#3E5C9A',
  paused: '#B0823A',
  done: '#4C8067',
  archived: '#9BA2AF',
}
const STATUS_LABELS = {
  active: '进行中',
  paused: '暂停',
  done: '已完成',
  archived: '已归档',
}

const statusDonutData = computed(() =>
  statusDist.value.map(d => ({
    label: STATUS_LABELS[d.label] || d.label,
    value: d.value,
    color: STATUS_COLORS[d.label] || '#9BA2AF',
  })),
)

const trackColor = label =>
  label.includes('无效') ? '#6C6A9C' : label.includes('行政') ? '#B0823A' : '#3E5C9A'

async function load() {
  loading.value = true
  const [t, s, tr, h] = await Promise.all([
    casyContext.dashboard.monthlyTaskTrend(6),
    casyContext.dashboard.projectStatusDistribution(),
    casyContext.dashboard.trackDistribution(),
    casyContext.dashboard.upcomingHearings(30),
  ])
  loading.value = false
  if (t.ok) trend.value = t.data || []
  if (s.ok) statusDist.value = s.data || []
  if (tr.ok) trackDist.value = tr.data || []
  if (h.ok) hearings.value = h.data || []
  if (!t.ok && !s.ok && !tr.ok && !h.ok) ElMessage.error(t.error || '仪表盘数据加载失败')
}

onMounted(() => {
  load()
})
</script>

<template>
  <div class="dash-page" v-loading="loading">
    <h2 class="page-title">数据看板</h2>

    <div class="charts-row">
      <div class="card">
        <div class="card-title"><el-icon :size="15"><TrendCharts /></el-icon> 月度任务趋势</div>
        <AreaLineChart v-if="trend.length" :data="trend" />
        <div v-else class="card-empty">近 6 个月暂无数据</div>
      </div>
      <div class="card">
        <div class="card-title"><el-icon :size="15"><PieChart /></el-icon> 项目状态</div>
        <DonutChart
          v-if="statusDonutData.length"
          :data="statusDonutData"
          :size="170"
          center-sub="总项目"
        />
        <div v-else class="card-empty">暂无项目</div>
      </div>
    </div>

    <div class="charts-row two">
      <div class="card">
        <div class="card-title"><el-icon :size="15"><Histogram /></el-icon> 案件轨道分布</div>
        <HBarChart
          :data="trackDist.map(d => ({
            label: d.label,
            value: d.value,
            color: trackColor(d.label),
          }))"
        />
      </div>
      <div class="card">
        <div class="card-title"><el-icon :size="15"><Calendar /></el-icon> 近期庭审 · 30 天</div>
        <GanttTimeline v-if="hearings.length" :items="hearings" :window-days="30" />
        <div v-else class="card-empty">未来 30 天没有庭审安排</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dash-page {
  padding: 20px 24px;
  max-width: 1100px;
  margin: 0 auto;
}
.page-title {
  margin: 0 0 18px;
  font-size: 20px;
  font-weight: 700;
  color: var(--c-text);
}
.charts-row {
  display: grid;
  grid-template-columns: 1.4fr 1fr;
  gap: 16px;
  margin-bottom: 16px;
}
.charts-row.two {
  grid-template-columns: 1fr 1.3fr;
}
@media (max-width: 980px) {
  .charts-row,
  .charts-row.two {
    grid-template-columns: 1fr;
  }
}
.card {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 14px 16px;
  min-width: 0;
}
.card-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 13.5px;
  font-weight: 600;
  color: var(--c-text);
  margin-bottom: 12px;
}
.card-title .el-icon { color: var(--c-text-secondary); }
.card-empty {
  padding: 26px 10px;
  text-align: center;
  font-size: 13px;
  color: var(--c-text-secondary);
}
</style>
