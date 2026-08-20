<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import { 
  ChatDotRound, 
  List, 
  Document, 
  DataBoard,
  Setting,
  Warning
} from '@element-plus/icons-vue'
import AIChatPanel from '../components/AIChatPanel.vue'
import AIAuditView from './AIAuditView.vue'
import DecisionsView from './DecisionsView.vue'
import { useTasksStore } from '../../../stores/tasks'

const tasksStore = useTasksStore()

const route = useRoute()

// 工具统计（插件系统异步初始化，就绪后刷新）
const tools = ref([])
const toolStats = computed(() => {
  const categories = {}
  tools.value.forEach(t => {
    const cat = t.category || 'other'
    categories[cat] = (categories[cat] || 0) + 1
  })
  return { total: tools.value.length, categories }
})

function refreshTools() {
  tools.value = casyContext.getTools()
}

onMounted(() => {
  refreshTools()
  casyContext.on('plugins:ready', () => refreshTools())
  // 防御：插件可能已在挂载前就绪（事件已错过），延迟兜底刷新一次
  setTimeout(refreshTools, 600)
})

// 支持从外部跳转定位 tab（如决策复核横幅 → /ai?tab=decisions）
// 工具系统未接入（getTools 恒空）时不开放 tools tab
const validTabs = computed(() => toolStats.value.total > 0
  ? ['recommend', 'chat', 'audit', 'decisions', 'tools']
  : ['recommend', 'chat', 'audit', 'decisions'])
const activeTab = ref(validTabs.value.includes(route.query.tab) ? route.query.tab : 'chat')

watch(() => route.query.tab, (tab) => {
  if (validTabs.value.includes(tab)) activeTab.value = tab
})
const aiConfig = ref(null)
const aiUsage = ref(null)
const loading = ref(false)

// AI 状态摘要
const statusSummary = computed(() => {
  const todayCalls = aiUsage.value?.todayCalls ?? 0
  const dailyLimit = aiConfig.value?.dailyLimit ?? 50
  const remaining = dailyLimit === 0 ? '不限' : Math.max(0, dailyLimit - todayCalls)
  const mode = aiConfig.value?.mode ?? 'noop'

  return {
    todayCalls,
    dailyLimit,
    remaining,
    mode,
    modeLabel: mode === 'noop' ? '规则匹配' : mode === 'ollama' ? 'Ollama' : 'OpenAI',
    modeColor: mode === 'noop' ? '#909399' : '#67C23A',
  }
})

async function loadAIData() {
  loading.value = true
  const [configResult, usageResult] = await Promise.all([
    tauriCallSafe('get_ai_config'),
    tauriCallSafe('get_ai_usage'),
  ])

  if (configResult.ok) {
    aiConfig.value = configResult.data
  }
  if (usageResult.ok) {
    aiUsage.value = usageResult.data
  }
  loading.value = false
}

// ============================================================
// 今日推荐（真实后端：get_today_recommendations，规则排序 §11.6）
// ============================================================
const recommendations = ref([])
const followupSuggestions = ref([])
const recommendSource = ref('rule')
const recommendDegraded = ref(false)
const recommendLoading = ref(false)

/** 规则兜底：后端不可用时按 due_date 本地排序 */
function buildFallbackRecommendations() {
  const today = new Date().toISOString().split('T')[0]
  return [...tasksStore.pendingTasks]
    .sort((a, b) => {
      const da = a.dueDate || '9999'
      const db = b.dueDate || '9999'
      if (da !== db) return da.localeCompare(db)
      return (b.flagged || 0) - (a.flagged || 0)
    })
    .slice(0, 5)
    .map(t => ({
      taskId: t.id,
      taskName: t.taskName,
      caseName: null,
      reason: t.dueDate
        ? (t.dueDate < today ? `已逾期，截止 ${t.dueDate}` : `截止 ${t.dueDate}`)
        : '按优先级规则排序',
      dueDate: t.dueDate || null,
      estimatedMinutes: t.estimatedMinutes || null,
    }))
}

async function loadRecommendations() {
  recommendLoading.value = true
  const result = await tauriCallSafe('get_today_recommendations')
  recommendLoading.value = false
  if (result.ok && result.data) {
    recommendations.value = result.data.recommendations || []
    followupSuggestions.value = result.data.followupSuggestions || []
    recommendSource.value = result.data.source || 'rule'
    recommendDegraded.value = false
  } else {
    // 降级：本地规则排序
    recommendations.value = buildFallbackRecommendations()
    followupSuggestions.value = []
    recommendDegraded.value = true
  }
}

/** 采纳推荐：记录决策 + 移入今日 */
async function adoptRecommendation(rec) {
  await tauriCallSafe('record_decision', {
    entityType: 'task',
    entityId: rec.taskId,
    decisionType: 'recommend_today',
    decision: 'adopt',
    basis: rec.reason || null,
    status: 'confirmed',
  })
  const moveResult = await tasksStore.moveToToday(rec.taskId)
  if (moveResult.ok) {
    ElMessage.success(`已将「${rec.taskName}」移入今日`)
    recommendations.value = recommendations.value.filter(r => r.taskId !== rec.taskId)
  } else {
    ElMessage.error(moveResult.error || '移入今日失败')
  }
}

/** 拒绝推荐：记录决策 */
async function rejectRecommendation(rec) {
  const result = await tauriCallSafe('record_decision', {
    entityType: 'task',
    entityId: rec.taskId,
    decisionType: 'recommend_today',
    decision: 'reject',
    basis: rec.reason || null,
    status: 'rejected',
  })
  if (result.ok) {
    recommendations.value = recommendations.value.filter(r => r.taskId !== rec.taskId)
    ElMessage.info('已记录拒绝，后续推荐将参考')
  }
}

// ============================================================
// 学习洞察（§11.9 行为学习闭环）
// ============================================================
const learningAnalysis = ref(null)
const learningDegraded = ref(false)
const calibrating = ref(false)

async function loadLearningAnalysis() {
  const result = await tauriCallSafe('get_learning_analysis')
  if (result.ok && result.data) {
    learningAnalysis.value = result.data
    learningDegraded.value = false
  } else {
    learningAnalysis.value = null
    learningDegraded.value = true
  }
}

async function applyCalibration() {
  calibrating.value = true
  const result = await tauriCallSafe('apply_learning_calibration')
  calibrating.value = false
  if (result.ok && result.data) {
    ElMessage.success(`已校准 ${result.data.calibratedCount} 条任务的预估耗时`)
    await loadLearningAnalysis()
  } else {
    ElMessage.error(result.error || '校准失败')
  }
}

/** 耗时统计行的准确度展示 */
function accuracyLabel(accuracy) {
  if (accuracy == null) return '-'
  return `${Math.round(accuracy * 100)}%`
}

// ============================================================
// 记忆确认区（蒸馏候选）
// ============================================================
const pendingMemories = ref([])
const memoriesDegraded = ref(false)

async function loadPendingMemories() {
  const result = await tauriCallSafe('list_pending_memories')
  if (result.ok) {
    pendingMemories.value = result.data || []
    memoriesDegraded.value = false
  } else {
    pendingMemories.value = []
    memoriesDegraded.value = true
  }
}

async function confirmMemory(item) {
  const result = await tauriCallSafe('confirm_memory', { id: item.id, sinkToKnowledge: true })
  if (result.ok) {
    pendingMemories.value = pendingMemories.value.filter(m => m.id !== item.id)
    ElMessage.success('已采纳并沉淀到知识库')
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

async function dismissMemory(item) {
  const result = await tauriCallSafe('dismiss_memory', { id: item.id })
  if (result.ok) {
    pendingMemories.value = pendingMemories.value.filter(m => m.id !== item.id)
    ElMessage.info('已忽略')
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

function memoryLayerLabel(layer) {
  const map = { working: '工作记忆', episodic: '情景记忆', semantic: '语义记忆' }
  return map[layer] || layer
}

// ============================================================
// 关联洞察确认区（隐性关联学习，照记忆确认区模式）
// ============================================================
const pendingInsights = ref([])
const insightsDegraded = ref(false)
const generatingInsights = ref(false)

async function loadPendingInsights() {
  const result = await tauriCallSafe('list_pending_insights')
  if (result.ok) {
    pendingInsights.value = result.data || []
    insightsDegraded.value = false
  } else {
    pendingInsights.value = []
    insightsDegraded.value = true
  }
}

async function runInsightsAnalysis() {
  generatingInsights.value = true
  const result = await tauriCallSafe('generate_insights_cmd')
  generatingInsights.value = false
  if (result.ok && result.data) {
    const inserted = result.data.inserted ?? 0
    ElMessage.success(inserted > 0 ? `新增 ${inserted} 条关联洞察` : '分析完成，暂无新增洞察')
    insightsDegraded.value = false
    await loadPendingInsights()
  } else {
    insightsDegraded.value = true
  }
}

async function confirmInsight(item, sinkToKnowledge) {
  const result = await tauriCallSafe('confirm_insight', { id: item.id, sinkToKnowledge })
  if (result.ok) {
    pendingInsights.value = pendingInsights.value.filter(i => i.id !== item.id)
    ElMessage.success(sinkToKnowledge ? '已采纳并沉淀到知识库' : '已确认')
    loadPendingInsights()
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

async function dismissInsight(item) {
  const result = await tauriCallSafe('dismiss_insight', { id: item.id })
  if (result.ok) {
    pendingInsights.value = pendingInsights.value.filter(i => i.id !== item.id)
    ElMessage.info('已忽略')
    loadPendingInsights()
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

/** sourceRef 为 JSON [{table, id}]，显示为「表名:记录」 */
function insightSourceLabel(sourceRef) {
  const refs = Array.isArray(sourceRef) ? sourceRef : sourceRef ? [sourceRef] : []
  return refs
    .map(r => (r && r.table ? `${r.table}:${r.id ?? ''}` : null))
    .filter(Boolean)
    .join('、')
}

// ============================================================
// 报表历史（smart_summaries，§11.3 报表浏览）
// ============================================================
const summaryTab = ref('daily') // daily 每日早报 / weekly 每周总结
const summaries = ref([])
const summariesDegraded = ref(false)
const summariesLoading = ref(false)
const expandedSummaryId = ref(null)

async function loadSummaries() {
  summariesLoading.value = true
  const result = await tauriCallSafe('list_summaries', { summaryType: summaryTab.value, limit: 20 })
  summariesLoading.value = false
  if (result.ok) {
    summaries.value = result.data || []
    summariesDegraded.value = false
  } else {
    summaries.value = []
    summariesDegraded.value = true
  }
}

watch(summaryTab, () => {
  expandedSummaryId.value = null
  loadSummaries()
})

function toggleSummary(id) {
  expandedSummaryId.value = expandedSummaryId.value === id ? null : id
}

/** 极简 Markdown 渲染：标题/列表/加粗（同 HomeView 早报，不引入新依赖） */
function renderMarkdown(md) {
  if (!md) return ''
  const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const inline = (s) => esc(s).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  let html = ''
  let inList = false
  for (const line of md.split('\n')) {
    const t = line.trim()
    if (t.startsWith('- ') || t.startsWith('* ')) {
      if (!inList) { html += '<ul>'; inList = true }
      html += `<li>${inline(t.slice(2))}</li>`
      continue
    }
    if (inList) { html += '</ul>'; inList = false }
    if (!t) continue
    if (t.startsWith('### ')) html += `<h5>${inline(t.slice(4))}</h5>`
    else if (t.startsWith('## ')) html += `<h4>${inline(t.slice(3))}</h4>`
    else if (t.startsWith('# ')) html += `<h3>${inline(t.slice(2))}</h3>`
    else html += `<p>${inline(t)}</p>`
  }
  if (inList) html += '</ul>'
  return html
}

onMounted(async () => {
  loadAIData()
  await tasksStore.loadTasks()
  loadRecommendations()
  loadLearningAnalysis()
  loadPendingMemories()
  loadPendingInsights()
  loadSummaries()
})
</script>

<template>
  <div class="ai-companion-page">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h3>AI 智伴</h3>
        <span class="header-desc">智能对话 · 工具调用 · 审计追踪</span>
      </div>
    </div>

    <!-- AI 状态摘要 -->
    <div class="status-bar">
      <div class="status-item">
        <span class="status-label">AI 模式</span>
        <el-tag :color="statusSummary.modeColor" effect="dark" size="small">
          {{ statusSummary.modeLabel }}
        </el-tag>
      </div>
      <el-divider direction="vertical" />
      <div class="status-item">
        <span class="status-label">今日调用</span>
        <span class="status-value">{{ statusSummary.todayCalls }}</span>
      </div>
      <el-divider direction="vertical" />
      <div class="status-item">
        <span class="status-label">剩余配额</span>
        <span class="status-value" :class="{ 'text-warning': statusSummary.remaining !== '不限' && statusSummary.remaining < 10 }">
          {{ statusSummary.remaining }}
        </span>
      </div>
      <el-divider direction="vertical" />
      <div class="status-item">
        <span class="status-label">已注册工具</span>
        <span class="status-value">{{ toolStats.total }}</span>
      </div>
    </div>

    <!-- 主要内容区 -->
    <el-tabs v-model="activeTab" class="main-tabs">
      <el-tab-pane name="recommend">
        <template #label>
          <el-icon><DataBoard /></el-icon>
          <span>今日推荐</span>
        </template>
        <div class="recommend-section" v-loading="recommendLoading">
          <div class="recommend-header">
            <h4>AI 推荐决策引擎</h4>
            <span class="recommend-sub">
              {{ recommendSource === 'ai' ? 'AI 智能推荐' : '基于规则排序（due_date + flagged + 案件阶段）' }}
            </span>
          </div>

          <!-- 降级提示条：推荐服务不可用 → 本地规则排序兜底 -->
          <div v-if="recommendDegraded" class="degrade-banner">
            <el-icon><Warning /></el-icon>
            推荐服务不可用，已按截止日期规则本地排序
          </div>

          <div class="recommend-cards">
            <div class="recommend-card">
              <div class="recommend-card-title">
                今日任务推荐
              </div>
              <div class="recommend-card-body">
                <div v-for="(rec, idx) in recommendations" :key="rec.taskId" class="recommend-item">
                  <span class="recommend-order">{{ idx + 1 }}</span>
                  <div class="recommend-info">
                    <div class="recommend-name">{{ rec.taskName }}</div>
                    <div class="recommend-reason">
                      {{ rec.reason }}
                      <template v-if="rec.estimatedMinutes"> · 预估 {{ rec.estimatedMinutes }} 分钟</template>
                      <template v-if="rec.caseName"> · {{ rec.caseName }}</template>
                    </div>
                  </div>
                  <el-button size="small" type="primary" plain @click="adoptRecommendation(rec)">采纳</el-button>
                  <el-button size="small" text @click="rejectRecommendation(rec)">拒绝</el-button>
                </div>
                <div v-if="recommendations.length === 0" class="recommend-empty">
                  暂无推荐
                </div>
              </div>
            </div>

            <div class="recommend-card">
              <div class="recommend-card-title">
                等待跟进建议
              </div>
              <div class="recommend-card-body">
                <div v-for="item in followupSuggestions" :key="item.taskId" class="recommend-item">
                  <div class="recommend-info">
                    <div class="recommend-name">{{ item.taskName }}</div>
                    <div class="recommend-reason">
                      {{ item.reason }}
                      <template v-if="item.waitingFor"> · 等 {{ item.waitingFor }}</template>
                      <template v-if="item.waitingDays"> · 已等 {{ item.waitingDays }} 天</template>
                    </div>
                  </div>
                </div>
                <div v-if="followupSuggestions.length === 0" class="recommend-empty">
                  无需跟进
                </div>
              </div>
            </div>
          </div>

          <!-- 学习洞察（§11.9 行为学习闭环） -->
          <div class="recommend-card insight-card">
            <div class="recommend-card-title">
              学习洞察
              <el-button
                size="small"
                type="primary"
                plain
                class="insight-action"
                :loading="calibrating"
                :disabled="learningDegraded || !learningAnalysis || !learningAnalysis.pendingCalibrationCount"
                @click="applyCalibration"
              >
                一键校准预估
                <template v-if="learningAnalysis && learningAnalysis.pendingCalibrationCount">
                  （{{ learningAnalysis.pendingCalibrationCount }} 条待校准）
                </template>
              </el-button>
            </div>
            <div class="recommend-card-body">
              <div v-if="learningDegraded" class="degrade-banner">
                <el-icon><Warning /></el-icon>
                学习分析数据暂不可用
              </div>
              <template v-else-if="learningAnalysis">
                <div v-if="learningAnalysis.lastCalibratedAt" class="insight-meta">
                  上次校准：{{ learningAnalysis.lastCalibratedAt }}，累计校准 {{ learningAnalysis.calibratedTaskCount }} 条
                </div>
                <div class="insight-grid">
                  <div class="insight-block">
                    <div class="insight-block-title">耗时校准</div>
                    <div v-if="learningAnalysis.durationStats.length === 0" class="recommend-empty">样本不足</div>
                    <div v-for="s in learningAnalysis.durationStats" :key="s.taskPattern" class="insight-row">
                      <span class="insight-key">{{ s.taskPattern }}</span>
                      <span class="insight-val">
                        预估 {{ Math.round(s.avgEstimated) }}′ → 实际 {{ Math.round(s.avgActual) }}′
                        · 准确度 {{ accuracyLabel(s.accuracy) }} · {{ s.sampleCount }} 样本
                      </span>
                    </div>
                  </div>
                  <div class="insight-block">
                    <div class="insight-block-title">活跃时段</div>
                    <div v-if="learningAnalysis.activityPatterns.length === 0" class="recommend-empty">样本不足</div>
                    <div v-for="p in learningAnalysis.activityPatterns.slice(0, 5)" :key="p.hour" class="insight-row">
                      <span class="insight-key">{{ p.hour }}:00</span>
                      <span class="insight-val">完成 {{ p.completions }} 次 · {{ Math.round(p.percentage * 100) }}%</span>
                    </div>
                  </div>
                  <div class="insight-block">
                    <div class="insight-block-title">延期模式</div>
                    <div v-if="learningAnalysis.delayPatterns.length === 0" class="recommend-empty">无明显延期模式</div>
                    <div v-for="d in learningAnalysis.delayPatterns" :key="d.caseType" class="insight-row">
                      <span class="insight-key">{{ d.caseType }}</span>
                      <span class="insight-val">
                        平均延期 {{ d.avgDelayDays.toFixed(1) }} 天 · 延期率 {{ Math.round(d.delayRate * 100) }}%
                      </span>
                    </div>
                  </div>
                </div>
              </template>
              <div v-else class="recommend-empty">加载中…</div>
            </div>
          </div>

          <!-- 记忆确认区（蒸馏候选） -->
          <div class="recommend-card insight-card">
            <div class="recommend-card-title">
              记忆确认区
              <span class="recommend-sub" style="margin-left: 8px">蒸馏候选需人工确认后才入库</span>
            </div>
            <div class="recommend-card-body">
              <div v-if="memoriesDegraded" class="degrade-banner">
                <el-icon><Warning /></el-icon>
                记忆服务暂不可用
              </div>
              <template v-else>
                <div v-for="m in pendingMemories" :key="m.id" class="recommend-item">
                  <div class="recommend-info">
                    <div class="recommend-name">{{ m.content }}</div>
                    <div class="recommend-reason">
                      {{ memoryLayerLabel(m.layer) }} · 置信度 {{ Math.round((m.confidence || 0) * 100) }}%
                      <template v-if="m.createdAt"> · {{ m.createdAt.substring(0, 10) }}</template>
                    </div>
                  </div>
                  <el-button size="small" type="primary" plain @click="confirmMemory(m)">采纳入库</el-button>
                  <el-button size="small" text @click="dismissMemory(m)">忽略</el-button>
                </div>
                <div v-if="pendingMemories.length === 0" class="recommend-empty">
                  没有待确认的候选记忆
                </div>
              </template>
            </div>
          </div>

          <!-- 关联洞察确认区（隐性关联学习 §3.2 通道 B） -->
          <div class="recommend-card insight-card">
            <div class="recommend-card-title">
              关联洞察
              <span class="recommend-sub" style="margin-left: 8px">隐性关联需人工确认后才生效</span>
              <el-button
                size="small"
                type="primary"
                plain
                class="insight-action"
                :loading="generatingInsights"
                @click="runInsightsAnalysis"
              >
                立即分析
              </el-button>
            </div>
            <div class="recommend-card-body">
              <div v-if="insightsDegraded" class="degrade-banner">
                <el-icon><Warning /></el-icon>
                关联洞察服务暂不可用
              </div>
              <template v-else>
                <div v-for="ins in pendingInsights" :key="ins.id" class="recommend-item">
                  <div class="recommend-info">
                    <div class="recommend-name">{{ ins.title }}</div>
                    <div class="recommend-reason">{{ ins.content }}</div>
                    <div class="recommend-reason">
                      置信度 {{ Math.round((ins.confidence || 0) * 100) }}%
                      <template v-if="insightSourceLabel(ins.sourceRef)">
                        · 来源 {{ insightSourceLabel(ins.sourceRef) }}
                      </template>
                    </div>
                  </div>
                  <el-button size="small" type="primary" plain @click="confirmInsight(ins, true)">采纳入库</el-button>
                  <el-button size="small" plain @click="confirmInsight(ins, false)">确认</el-button>
                  <el-button size="small" text @click="dismissInsight(ins)">忽略</el-button>
                </div>
                <div v-if="pendingInsights.length === 0" class="recommend-empty">
                  没有待确认的关联洞察，可点击「立即分析」手动生成
                </div>
              </template>
            </div>
          </div>

          <!-- 报表历史（smart_summaries，§11.3 报表浏览） -->
          <div class="recommend-card insight-card">
            <div class="recommend-card-title">
              报表历史
              <el-radio-group v-model="summaryTab" size="small" class="insight-action">
                <el-radio-button value="daily">每日早报</el-radio-button>
                <el-radio-button value="weekly">每周总结</el-radio-button>
              </el-radio-group>
            </div>
            <div class="recommend-card-body" v-loading="summariesLoading">
              <div v-if="summariesDegraded" class="degrade-banner">
                <el-icon><Warning /></el-icon>
                报表服务暂不可用
              </div>
              <template v-else>
                <div v-for="s in summaries" :key="s.id" class="summary-item">
                  <div class="summary-head" @click="toggleSummary(s.id)">
                    <span class="summary-period">{{ s.periodStart || '-' }} ~ {{ s.periodEnd || '-' }}</span>
                    <el-tag
                      size="small"
                      :type="s.narrativeSource === 'ai' ? 'success' : 'info'"
                      effect="plain"
                    >
                      {{ s.narrativeSource === 'ai' ? 'AI' : '规则' }}
                    </el-tag>
                    <span class="summary-time">{{ (s.createdAt || '').substring(0, 16) }}</span>
                  </div>
                  <div
                    v-if="expandedSummaryId === s.id"
                    class="summary-content"
                    v-html="renderMarkdown(s.content)"
                  ></div>
                </div>
                <div v-if="summaries.length === 0 && !summariesLoading" class="recommend-empty">
                  暂无{{ summaryTab === 'daily' ? '每日早报' : '每周总结' }}记录，生成后会出现在这里
                </div>
              </template>
            </div>
          </div>

          <div class="recommend-note">
            <el-icon><Warning /></el-icon>
            推荐与校准由规则引擎生成。配置 AI 后端后可启用智能推荐。
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane name="chat">
        <template #label>
          <el-icon><ChatDotRound /></el-icon>
          <span>AI 对话</span>
        </template>
        <AIChatPanel />
      </el-tab-pane>

      <el-tab-pane name="audit">
        <template #label>
          <el-icon><List /></el-icon>
          <span>审计日志</span>
        </template>
        <AIAuditView />
      </el-tab-pane>

      <el-tab-pane name="decisions">
        <template #label>
          <el-icon><Document /></el-icon>
          <span>决策记录</span>
        </template>
        <DecisionsView />
      </el-tab-pane>

      <el-tab-pane name="tools" v-if="toolStats.total > 0">
        <template #label>
          <el-icon><Setting /></el-icon>
          <span>工具管理</span>
        </template>
        <div class="tools-section">
          <el-card shadow="never">
            <template #header>
              <div class="card-header">
                <span>已注册工具</span>
                <el-tag size="small">{{ toolStats.total }} 个</el-tag>
              </div>
            </template>
            
            <div class="tool-categories">
              <el-tag 
                v-for="(count, category) in toolStats.categories" 
                :key="category"
                class="category-tag"
              >
                {{ category }}: {{ count }}
              </el-tag>
            </div>
            
            <div class="tool-list">
              <div 
                v-for="tool in tools" 
                :key="tool.name"
                class="tool-item"
              >
                <div class="tool-info">
                  <span class="tool-name">{{ tool.name }}</span>
                  <span class="tool-category">{{ tool.category || 'other' }}</span>
                </div>
                <div class="tool-description">{{ tool.description }}</div>
              </div>
            </div>
          </el-card>
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<style scoped>
.ai-companion-page {
  padding: 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.page-header h3 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
}

.header-desc {
  color: #6b7280;
  font-size: 13px;
  margin-left: 12px;
}

.status-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: #f9fafb;
  border-radius: 8px;
  margin-bottom: 16px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-label {
  color: #6b7280;
  font-size: 13px;
}

.status-value {
  font-weight: 500;
}

.text-warning {
  color: #e6a23c;
}

.main-tabs {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.main-tabs :deep(.el-tabs__content) {
  flex: 1;
  overflow: hidden;
}

.main-tabs :deep(.el-tab-pane) {
  height: 100%;
}

.tools-section {
  height: 100%;
  overflow-y: auto;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.tool-categories {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 16px;
}

.category-tag {
  text-transform: capitalize;
}

.tool-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tool-item {
  padding: 12px;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
}

.tool-info {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.tool-name {
  font-weight: 500;
  color: #3b82f6;
}

.tool-category {
  font-size: 12px;
  color: #6b7280;
  background: #f3f4f6;
  padding: 2px 6px;
  border-radius: 4px;
}

.tool-description {
  font-size: 13px;
  color: #6b7280;
}

/* 推荐引擎样式（设计哲学 §11.6） */
.recommend-section {
  padding: 16px 0;
}

.recommend-header {
  margin-bottom: 16px;
}

.recommend-header h4 {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 600;
  color: #1F2430;
}

.recommend-sub {
  font-size: 12px;
  color: #9BA2AF;
}

.recommend-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  margin-bottom: 16px;
}

.recommend-card {
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  overflow: hidden;
}

.recommend-card-title {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  font-size: 14px;
  font-weight: 600;
  color: #1F2430;
  border-bottom: 1px solid #EEF0F3;
}

.recommend-icon {
  font-size: 18px;
}

.recommend-card-body {
  padding: 8px;
}

.recommend-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 6px;
  border: 1px solid #E0E3E9;
  margin-bottom: 8px;
  transition: all 0.15s;
}

.recommend-item:hover {
  border-color: #C3CFE3;
  background: #EDF1F8;
}

.recommend-order {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  background: #EDF1F8;
  color: #3E5C9A;
  font-size: 12px;
  font-weight: 700;
  display: grid;
  place-items: center;
  flex-shrink: 0;
}

.recommend-info {
  flex: 1;
  min-width: 0;
}

.recommend-name {
  font-size: 13px;
  font-weight: 500;
  color: #1F2430;
}

.recommend-reason {
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 2px;
}

.recommend-empty {
  text-align: center;
  padding: 24px;
  color: #9BA2AF;
  font-size: 13px;
}

.recommend-note {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: #F7F1E3;
  border: 1px solid #E4D3A8;
  border-radius: 6px;
  font-size: 12px;
  color: #7A5B24;
}

/* 降级提示条（琥珀底） */
.degrade-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  margin-bottom: 12px;
  background: #F7F1E3;
  border: 1px solid #E4D3A8;
  border-radius: 6px;
  font-size: 12px;
  color: #7A5B24;
}

/* 学习洞察 / 记忆确认 */
.insight-card {
  margin-bottom: 16px;
}

.insight-action {
  margin-left: auto;
}

.insight-meta {
  font-size: 12px;
  color: #9BA2AF;
  padding: 4px 12px 8px;
}

.insight-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  padding: 4px 12px 12px;
}

.insight-block {
  border: 1px solid #EEF0F3;
  border-radius: 6px;
  padding: 8px 10px;
}

.insight-block-title {
  font-size: 12px;
  font-weight: 600;
  color: #4B5160;
  margin-bottom: 6px;
}

.insight-row {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
  padding: 3px 0;
}

.insight-key {
  color: #1F2430;
  font-weight: 500;
  flex-shrink: 0;
}

.insight-val {
  color: #9BA2AF;
  text-align: right;
}

/* 报表历史 */
.summary-item {
  border: 1px solid #E0E3E9;
  border-radius: 6px;
  margin: 0 4px 8px;
  overflow: hidden;
}

.summary-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  cursor: pointer;
  transition: background 0.15s;
}

.summary-head:hover {
  background: #EDF1F8;
}

.summary-period {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 500;
  color: #1F2430;
}

.summary-time {
  font-size: 11px;
  color: #9BA2AF;
  flex-shrink: 0;
}

.summary-content {
  padding: 4px 12px 12px;
  border-top: 1px solid #EEF0F3;
  font-size: 13px;
  color: #4B5160;
  line-height: 1.7;
}

.summary-content :deep(h3),
.summary-content :deep(h4),
.summary-content :deep(h5) {
  margin: 8px 0 4px;
  font-size: 13px;
  color: #1F2430;
}

.summary-content :deep(ul) {
  margin: 4px 0;
  padding-left: 18px;
}

.summary-content :deep(p) {
  margin: 4px 0;
}
</style>
