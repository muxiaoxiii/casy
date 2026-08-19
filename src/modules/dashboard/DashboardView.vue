<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { tauriCallSafe } from '../../core/tauriBridge'
import {
  Briefcase, Clock, Warning, Folder, Calendar,
  DataBoard, PieChart, Histogram,
  RefreshRight, Bell, Timer, Finished,
} from '@element-plus/icons-vue'

const router = useRouter()
const loading = ref(false)
const cases = ref([])
const tasks = ref([])
const events = ref([])

onMounted(async () => {
  loading.value = true
  await Promise.all([
    loadCases(),
    loadTasks(),
    loadEvents(),
  ])
  loading.value = false
})

async function loadCases() {
  const result = await tauriCallSafe('list_cases', { filter: {} })
  if (result.ok) cases.value = result.data?.items || []
}

async function loadTasks() {
  // 拉全部任务（含已完成），供趋势统计使用；展示统计时再按 completed 过滤
  const result = await tauriCallSafe('list_tasks', { filter: {} })
  if (result.ok) tasks.value = result.data || []
}

async function loadEvents() {
  const now = new Date()
  const result = await tauriCallSafe('get_calendar_events', {
    year: now.getFullYear(),
    month: now.getMonth() + 1,
  })
  if (result.ok) events.value = result.data || []
}

// ── 统计数据 ──────────────────────────────────────────────
const stats = computed(() => ({
  activeCases: cases.value.filter(c => c.caseStatus !== '已完结').length,
  waiting: tasks.value.filter(t => t.taskType === 'waiting' && !t.completed).length,
  closed: cases.value.filter(c => c.caseStatus === '已完结').length,
  overdue: tasks.value.filter(t => {
    if (t.completed) return false
    const due = t.dueDate || t.deadline
    return due && due < new Date().toISOString().split('T')[0]
  }).length,
}))

// ── 轨道分布数据 ──────────────────────────────────────────
const trackDistribution = computed(() => {
  const active = cases.value.filter(c => c.caseStatus !== '已完结')
  const tracks = [
    { label: '专利无效', key: 'patent_invalidation', color: '#6C6A9C' },
    { label: '民事侵权', key: 'civil_tort', color: '#3E5C9A' },
    { label: '行政诉讼', key: 'admin_litigation', color: '#B0823A' },
    { label: '其他', key: 'other', color: '#9BA2AF' },
  ]
  return tracks.map(t => ({
    ...t,
    count: active.filter(c => c.track === t.key).length,
    percent: active.length > 0 ? (active.filter(c => c.track === t.key).length / active.length * 100) : 0,
  }))
})

// ── 案件状态分布（环形图数据）───────────────────────────────
const statusDistribution = computed(() => {
  const total = cases.value.length || 1
  const groups = [
    { label: '进行中', color: '#3E5C9A', count: cases.value.filter(c => c.caseStatus === '进行中').length },
    { label: '等待中', color: '#B0823A', count: cases.value.filter(c => c.caseStatus === '等待中').length },
    { label: '已完结', color: '#4C8067', count: cases.value.filter(c => c.caseStatus === '已完结').length },
  ]
  return groups.map(g => ({ ...g, percent: (g.count / total * 100) }))
})

// ── 月度趋势（近6个月，真实数据：新建任务 + 新增案件）──────────────
const trendData = computed(() => {
  const now = new Date()
  const buckets = []
  for (let i = 5; i >= 0; i--) {
    const d = new Date(now.getFullYear(), now.getMonth() - i, 1)
    buckets.push({
      key: `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`,
      month: `${d.getMonth() + 1}月`,
      tasks: 0,
      cases: 0,
    })
  }
  const index = Object.fromEntries(buckets.map((b, i) => [b.key, i]))
  for (const t of tasks.value) {
    const key = (t.createdDate || '').slice(0, 7)
    if (key in index) buckets[index[key]].tasks++
  }
  for (const c of cases.value) {
    const key = (c.createdAt || '').slice(0, 7)
    if (key in index) buckets[index[key]].cases++
  }
  return buckets
})

const trendMax = computed(() =>
  Math.max(1, ...trendData.value.flatMap(d => [d.tasks, d.cases]))
)

// 数据不足（近6个月无任何新建）→ 显示空态而非空坐标系
const hasTrendData = computed(() =>
  trendData.value.some(d => d.tasks > 0 || d.cases > 0)
)

// ── 近期庭审时间线 ────────────────────────────────────────
const upcomingEvents = computed(() => {
  return events.value
    .filter(e => e.type === 'hearing' || e.type === 'deadline')
    .sort((a, b) => (a.date || '').localeCompare(b.date || ''))
    .slice(0, 6)
})

// ── SVG 环形图路径计算 ────────────────────────────────────
function donutPath(cx, cy, r, startAngle, endAngle) {
  const start = {
    x: cx + r * Math.cos(startAngle),
    y: cy + r * Math.sin(startAngle),
  }
  const end = {
    x: cx + r * Math.cos(endAngle),
    y: cy + r * Math.sin(endAngle),
  }
  const largeArc = endAngle - startAngle > Math.PI ? 1 : 0
  return `M ${start.x} ${start.y} A ${r} ${r} 0 ${largeArc} 1 ${end.x} ${end.y}`
}

// ── SVG 折线图路径 ────────────────────────────────────────
function trendPath(data, maxVal, width, height, padding) {
  if (!data.length) return ''
  const stepX = (width - padding * 2) / (data.length - 1)
  return data.map((d, i) => {
    const x = padding + i * stepX
    const y = height - padding - (d / maxVal * (height - padding * 2))
    return `${i === 0 ? 'M' : 'L'} ${x} ${y}`
  }).join(' ')
}
</script>

<template>
  <div class="dashboard-page" v-loading="loading">
    <!-- 统计卡片 -->
    <div class="stat-cards">
      <div class="stat-card" @click="router.push('/cases')">
        <div class="stat-icon blue"><el-icon :size="20"><Briefcase /></el-icon></div>
        <div>
          <div class="stat-value">{{ stats.activeCases }}</div>
          <div class="stat-label">活跃案件</div>
        </div>
      </div>
      <div class="stat-card" @click="router.push('/tasks')">
        <div class="stat-icon amber"><el-icon :size="20"><Clock /></el-icon></div>
        <div>
          <div class="stat-value">{{ stats.waiting }}</div>
          <div class="stat-label">等待中</div>
        </div>
      </div>
      <div class="stat-card" @click="router.push('/cases')">
        <div class="stat-icon green"><el-icon :size="20"><Folder /></el-icon></div>
        <div>
          <div class="stat-value">{{ stats.closed }}</div>
          <div class="stat-label">已结案</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon red"><el-icon :size="20"><Warning /></el-icon></div>
        <div>
          <div class="stat-value">{{ stats.overdue }}</div>
          <div class="stat-label">逾期</div>
        </div>
      </div>
    </div>

    <!-- 图表区域 -->
    <div class="charts-grid">
      <!-- 左栏：趋势折线图 -->
      <div class="card chart-card">
        <div class="card-header">
          <el-icon :size="14"><DataBoard /></el-icon>
          <span>月度趋势</span>
          <el-tag size="small" type="info">近6个月</el-tag>
        </div>
        <div class="chart-body">
          <div v-if="!hasTrendData" class="empty-timeline">
            近6个月暂无新建任务或案件，数据不足
          </div>
          <svg v-else viewBox="0 0 400 180" class="trend-chart">
            <!-- 网格线 -->
            <line v-for="i in 4" :key="i"
              :x1="40" :x2="380"
              :y1="20 + i * 32" :y2="20 + i * 32"
              stroke="#EEF0F3" stroke-width="1" />
            <!-- X 轴标签 -->
            <text v-for="(d, i) in trendData" :key="i"
              :x="40 + i * 68" :y="170"
              text-anchor="middle" fill="#9BA2AF" font-size="11">{{ d.month }}</text>
            <!-- 新建任务线 -->
            <path :d="trendPath(trendData.map(d => d.tasks), trendMax, 360, 160, 40)"
              fill="none" stroke="#3E5C9A" stroke-width="2.5" stroke-linecap="round"
              transform="translate(20, 10)" />
            <!-- 新增案件线 -->
            <path :d="trendPath(trendData.map(d => d.cases), trendMax, 360, 160, 40)"
              fill="none" stroke="#4C8067" stroke-width="2.5" stroke-linecap="round"
              transform="translate(20, 10)" />
            <!-- 数据点 -->
            <circle v-for="(d, i) in trendData" :key="'c'+i"
              :cx="60 + i * 68" :cy="170 - (d.tasks / trendMax * 140)"
              r="3.5" fill="#3E5C9A" stroke="white" stroke-width="2" />
            <circle v-for="(d, i) in trendData" :key="'d'+i"
              :cx="60 + i * 68" :cy="170 - (d.cases / trendMax * 140)"
              r="3.5" fill="#4C8067" stroke="white" stroke-width="2" />
            <!-- 图例 -->
            <circle cx="260" cy="12" r="4" fill="#3E5C9A" />
            <text x="268" y="16" fill="#4B5160" font-size="11">新建任务</text>
            <circle cx="324" cy="12" r="4" fill="#4C8067" />
            <text x="332" y="16" fill="#4B5160" font-size="11">新增案件</text>
          </svg>
        </div>
      </div>

      <!-- 右栏：案件状态环形图 -->
      <div class="card chart-card">
        <div class="card-header">
          <el-icon :size="14"><PieChart /></el-icon>
          <span>案件状态</span>
        </div>
        <div class="chart-body donut-body">
          <svg viewBox="0 0 160 160" class="donut-chart">
            <circle cx="80" cy="80" r="55" fill="none" stroke="#EEF0F3" stroke-width="14" />
            <circle v-for="(s, i) in statusDistribution" :key="i"
              cx="80" cy="80" r="55"
              fill="none" :stroke="s.color" stroke-width="14"
              :stroke-dasharray="`${s.percent * 3.45} ${345 - s.percent * 3.45}`"
              :stroke-dashoffset="`${-statusDistribution.slice(0, i).reduce((a, b) => a + b.percent, 0) * 3.45 + 86}`"
              stroke-linecap="round" />
            <text x="80" y="76" text-anchor="middle" fill="#1F2430" font-size="24" font-weight="700">
              {{ cases.length }}
            </text>
            <text x="80" y="96" text-anchor="middle" fill="#9BA2AF" font-size="11">总案件</text>
          </svg>
          <div class="donut-legend">
            <div v-for="s in statusDistribution" :key="s.label" class="legend-item">
              <span class="legend-dot" :style="{ background: s.color }" />
              <span>{{ s.label }}</span>
              <span class="legend-count">{{ s.count }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 第二行：轨道分布 + 近期庭审 -->
    <div class="charts-grid">
      <!-- 轨道分布条形图 -->
      <div class="card chart-card">
        <div class="card-header">
          <el-icon :size="14"><Histogram /></el-icon>
          <span>案件轨道分布</span>
          <span class="sub">共 {{ cases.filter(c => c.caseStatus !== '已完结').length }} 个进行中</span>
        </div>
        <div class="chart-body">
          <div v-for="track in trackDistribution" :key="track.key" class="dist-row">
            <span class="dist-label">
              <span class="track-dot" :style="{ background: track.color }" />
              {{ track.label }}
            </span>
            <div class="dist-bar">
              <div class="dist-fill" :style="{ width: track.percent + '%', background: track.color }" />
            </div>
            <span class="dist-val">{{ track.count }}</span>
          </div>
        </div>
      </div>

      <!-- 近期庭审时间线 -->
      <div class="card chart-card">
        <div class="card-header">
          <el-icon :size="14"><Calendar /></el-icon>
          <span>近期庭审 & 期限</span>
          <el-tag size="small" type="danger">{{ upcomingEvents.length }} 项</el-tag>
        </div>
        <div class="chart-body timeline-body">
          <div v-for="ev in upcomingEvents" :key="ev.id" class="timeline-row">
            <span class="timeline-time">{{ ev.date }}</span>
            <span class="timeline-dot" :class="ev.type" />
            <div class="timeline-content">
              <div class="timeline-title">{{ ev.title }}</div>
              <div class="timeline-meta" v-if="ev.caseName">{{ ev.caseName }}</div>
            </div>
            <el-tag size="small" :type="ev.type === 'hearing' ? 'danger' : 'warning'">
              {{ ev.type === 'hearing' ? '开庭/口审' : '期限' }}
            </el-tag>
          </div>
          <div v-if="upcomingEvents.length === 0" class="empty-timeline">
            近期无庭审或期限
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard-page {
  max-width: 1200px;
  margin: 0 auto;
}

/* 统计卡片 */
.stat-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
  margin-bottom: 14px;
}

.stat-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.stat-card:hover {
  border-color: var(--c-primary-lighter);
  box-shadow: var(--shadow-md);
  transform: translateY(-1px);
}

.stat-icon {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.stat-icon.blue { background: #EDF1F8; color: #3E5C9A; }
.stat-icon.amber { background: #F7F1E3; color: #B0823A; }
.stat-icon.green { background: #EDF3EF; color: #4C8067; }
.stat-icon.red { background: #F6EDEC; color: #B4554F; }

.stat-value {
  font-size: 24px;
  font-weight: 700;
  color: #1F2430;
  line-height: 1;
}

.stat-label {
  font-size: 12px;
  color: #9BA2AF;
  margin-top: 2px;
}

/* 图表网格 */
.charts-grid {
  display: grid;
  grid-template-columns: 1.35fr 1fr;
  gap: 14px;
  margin-bottom: 14px;
}

.chart-card {
  padding: 14px 16px;
}

.chart-card .card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 700;
  color: #1F2430;
  padding-bottom: 10px;
  margin-bottom: 10px;
  border-bottom: 1px solid #EEF0F3;
}

.chart-card .card-header .sub {
  font-weight: 400;
  color: #9BA2AF;
  font-size: 11px;
  margin-left: auto;
}

.chart-body {
  min-height: 180px;
}

/* 趋势图 */
.trend-chart {
  width: 100%;
  height: auto;
}

/* 环形图 */
.donut-body {
  display: flex;
  align-items: center;
  gap: 24px;
}

.donut-chart {
  width: 140px;
  height: 140px;
  flex-shrink: 0;
}

.donut-legend {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #4B5160;
}

.legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.legend-count {
  margin-left: auto;
  font-family: var(--font-mono);
  font-weight: 600;
  color: #1F2430;
}

/* 分布条 */
.dist-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 0;
  font-size: 13px;
}

.dist-label {
  width: 84px;
  color: #4B5160;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 6px;
}

.track-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  display: inline-block;
}

.dist-bar {
  flex: 1;
  height: 7px;
  border-radius: 4px;
  background: #EEF0F3;
  overflow: hidden;
}

.dist-fill {
  height: 100%;
  border-radius: 4px;
  transition: width 0.6s ease;
}

.dist-val {
  width: 40px;
  text-align: right;
  font-family: var(--font-mono);
  color: #4B5160;
  font-size: 12px;
}

/* 时间线 */
.timeline-body {
  max-height: 260px;
  overflow-y: auto;
}

.timeline-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
  border-bottom: 1px solid #EEF0F3;
}

.timeline-row:last-child { border-bottom: none; }

.timeline-time {
  font-family: var(--font-mono);
  font-size: 12px;
  color: #4B5160;
  width: 80px;
  flex-shrink: 0;
}

.timeline-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.timeline-dot.hearing { background: #B4554F; }
.timeline-dot.deadline { background: #B0823A; }

.timeline-content {
  flex: 1;
  min-width: 0;
}

.timeline-title {
  font-size: 13px;
  color: #1F2430;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-meta {
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 1px;
}

.empty-timeline {
  text-align: center;
  padding: 24px;
  color: #9BA2AF;
  font-size: 13px;
}
</style>
