<script setup>
/**
 * Dashboard —— 数据可视化仪表盘（B4 · 对标 index-v2 dashboard 设计）
 *
 * 四图布局：月度任务趋势(面积线) · 案件状态(环图) · 轨道分布(横条) · 近期庭审(甘特)
 * 数据全部来自后端聚合命令（dashboard 服务），零前端拉全量。
 */
import { ref, onMounted, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { TrendCharts, PieChart, Histogram, Calendar, MagicStick } from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import { casyContext } from '../../core/plugin/context'
import AreaLineChart from '../../shared/charts/AreaLineChart.vue'
import DonutChart from '../../shared/charts/DonutChart.vue'
import HBarChart from '../../shared/charts/HBarChart.vue'
import GanttTimeline from '../../shared/charts/GanttTimeline.vue'

const router = useRouter()
const trend = ref([])
const statusDist = ref([])
const trackDist = ref([])
const hearings = ref([])
const kpis = ref({ today_events: 0, due_today: 0, waiting_overdue: 0, review_due: 0 })
const aiInsight = ref('')
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

function onStatusSelect(d) {
  const map = { active: '/projects', paused: '/projects', done: '/projects', archived: '/projects' }
  void map
  router.push('/projects')
}

const trackColor = label =>
  label.includes('无效') ? '#6C6A9C' : label.includes('行政') ? '#B0823A' : '#3E5C9A'

async function load() {
  loading.value = true
  try {
    const [t, s, tr, h, k, aiRes] = await Promise.all([
      casyContext.dashboard.monthlyTaskTrend(6),
      casyContext.dashboard.projectStatusDistribution(),
      casyContext.dashboard.trackDistribution(),
      casyContext.dashboard.upcomingHearings(30),
      casyContext.dashboard.todayKpis(),
      casyContext.ai.askAi('请根据当前的日期，给律师一句简短的早安问候和一天工作重点的建议（不超过50字）。')
    ])
    if (t.ok) trend.value = t.data || []
    if (s.ok) statusDist.value = s.data || []
    if (tr.ok) trackDist.value = tr.data || []
    if (h.ok) hearings.value = h.data || []
    if (k.ok && k.data) kpis.value = k.data
    if (aiRes.ok && aiRes.text) aiInsight.value = aiRes.text
    if (!t.ok && !s.ok && !tr.ok && !h.ok) ElMessage.error(t.error || '仪表盘数据加载失败')
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : '仪表盘数据加载失败')
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  load()
})
</script>

<template>
  <div class="dash-page" v-loading="loading">
    <div class="dash-header">
      <h2 class="page-title">数据看板</h2>
      <div v-if="aiInsight" class="ai-insight-banner">
        <el-icon><MagicStick /></el-icon>
        <span class="ai-text">{{ aiInsight }}</span>
      </div>
    </div>

    <!-- KPI 行（点击下钻） -->
    <div class="kpi-row">
      <div class="kpi-card" @click="$router.push('/calendar')">
        <span class="kv" :style="{ color: 'var(--c-primary)' }">{{ kpis.today_events }}</span>
        <span class="kk">今日日程</span>
      </div>
      <div class="kpi-card" @click="$router.push({ name: 'tasks', query: { tab: 'today' } })">
        <span class="kv">{{ kpis.due_today }}</span>
        <span class="kk">今日到期</span>
      </div>
      <div class="kpi-card" @click="$router.push({ name: 'tasks', query: { tab: 'waiting' } })">
        <span class="kv" :style="{ color: kpis.waiting_overdue > 0 ? 'var(--c-warning)' : undefined }">{{ kpis.waiting_overdue }}</span>
        <span class="kk">等待超时</span>
      </div>
      <div class="kpi-card" @click="$router.push({ name: 'tasks', query: { tab: 'review' } })">
        <span class="kv" :style="{ color: kpis.review_due > 0 ? 'var(--c-danger)' : undefined }">{{ kpis.review_due }}</span>
        <span class="kk">需回顾</span>
      </div>
    </div>

    <div class="charts-row">
      <div class="card">
        <div class="card-title"><el-icon :size="15"><TrendCharts /></el-icon> 月度任务趋势</div>
        <AreaLineChart v-if="trend.length" :data="trend" />
        <div v-else class="card-empty">近 6 个月暂无数据</div>
      </div>
      <div class="card">
        <div class="card-title"><el-icon :size="15"><PieChart /></el-icon> 非案件项目状态</div>
        <DonutChart
          v-if="statusDonutData.length"
          :data="statusDonutData"
          :size="170"
          center-sub="非案件项目"
          @select="onStatusSelect"
        />
        <div v-else class="card-empty">暂无非案件项目</div>
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
          @select="() => $router.push({ name: 'cases-kanban' })"
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
.dash-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 18px;
}
.page-title {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
  color: var(--c-text);
}
.ai-insight-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  background: var(--c-primary-light);
  border: 1px solid var(--c-primary-soft);
  border-radius: 8px;
  color: var(--c-primary);
  font-size: 13px;
  font-weight: 500;
  max-width: 500px;
}
.ai-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.kpi-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}
.kpi-card {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 14px 16px;
  display: flex;
  align-items: baseline;
  gap: 8px;
  cursor: pointer;
  transition:
    transform var(--motion-fast) var(--ease-out),
    box-shadow var(--motion-fast) var(--ease-out);
}
.kpi-card:hover {
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
}
.kv { font-size: 26px; font-weight: 700; color: var(--c-text); }
.kk { font-size: 12px; color: var(--c-text-secondary); }

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
