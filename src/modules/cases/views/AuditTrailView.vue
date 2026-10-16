<script setup>
/**
 * 操作记录（A3）
 *
 * `audit_events` 由数据库触发器与 AI/MCP 写路径写入（开庭改期、白板事实增改删、
 * AI 工具执行），带 actor 归因（user/ai/system/mcp/skill）——但此前前端零入口，
 * 用户无法回答"这个开庭日期为什么变了 / 这条事实是谁改的"。
 *
 * 与案件详情里的开庭变更链是同一套数据的两个切面：那里按开庭组织，这里全局按时间组织。
 */
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import { Refresh } from '../../../shared/icons'
import EmptyState from '../../../shared/components/EmptyState.vue'

const router = useRouter()

const events = ref([])
const loading = ref(false)
const error = ref('')
const actorFilter = ref('')
const typeFilter = ref('')
const caseFilter = ref('')

const ACTOR_LABELS = {
  user: '本人操作',
  ai: 'AI 执行',
  system: '系统（触发器）',
  mcp: 'MCP 工具',
  skill: '技能',
}

const TYPE_LABELS = {
  hearing_updated: '开庭改期 / 状态变更',
  whiteboard_fact_added: '新增事实',
  whiteboard_fact_updated: '修改事实',
  whiteboard_fact_deleted: '删除事实',
  ai_tool_executed: 'AI 工具执行',
}

const typeOptions = computed(() => {
  const set = new Set(events.value.map((item) => item.eventType))
  return [...set].sort()
})

const filtered = computed(() => {
  let list = events.value
  if (actorFilter.value) list = list.filter((item) => item.actor === actorFilter.value)
  if (typeFilter.value) list = list.filter((item) => item.eventType === typeFilter.value)
  const keyword = caseFilter.value.trim().toLowerCase()
  if (keyword) {
    list = list.filter((item) => {
      const text = JSON.stringify(item.payload || {}).toLowerCase()
      return text.includes(keyword) || item.aggregateId.toLowerCase().includes(keyword)
    })
  }
  return list
})

async function load() {
  loading.value = true
  error.value = ''
  const result = await casyContext.cases.entityAuditEvents(null, null)
  loading.value = false
  if (result.ok && Array.isArray(result.data)) events.value = result.data
  else { events.value = []; error.value = result.error || '操作记录读取失败' }
}

function typeLabel(type) { return TYPE_LABELS[type] || type }
function actorLabel(actor) { return ACTOR_LABELS[actor] || actor }
function timeOf(item) { return String(item.createdAt || '').replace('T', ' ').slice(0, 19) }

/** payload 的 before/after 摘要（开庭改期最有信息量） */
function changeSummary(item) {
  const payload = item.payload || null
  if (!payload) return ''
  const before = payload.before, after = payload.after
  if (before && after) {
    const parts = []
    for (const key of Object.keys(after)) {
      if (String(before[key]) !== String(after[key])) parts.push(`${key}: ${before[key] ?? '—'} → ${after[key] ?? '—'}`)
    }
    return parts.join('；')
  }
  if (payload.hearingId) return `开庭 ${payload.hearingId}`
  if (payload.reason) return `依据：${payload.reason}`
  return JSON.stringify(payload).slice(0, 120)
}

function openCase(item) {
  router.push({ name: 'case-detail', params: { id: item.aggregateId } })
}

onMounted(load)
</script>

<template>
  <section class="audit-page ui-col">
    <header class="audit-head ui-row ui-row--between ui-row--wrap">
      <div>
        <h2>操作记录</h2>
        <p class="audit-sub">开庭改期、事实变更与 AI / MCP 写操作的留痕。每条都可回溯到具体案件。</p>
      </div>
      <el-button :icon="Refresh" circle :loading="loading" title="刷新" aria-label="刷新" @click="load" />
    </header>

    <div class="audit-filters ui-row ui-row--wrap">
      <el-select v-model="actorFilter" clearable placeholder="操作来源" size="small" style="width: 140px" aria-label="按操作来源筛选">
        <el-option v-for="(label, key) in ACTOR_LABELS" :key="key" :value="key" :label="label" />
      </el-select>
      <el-select v-model="typeFilter" clearable placeholder="变更类型" size="small" style="width: 200px" aria-label="按变更类型筛选">
        <el-option v-for="type in typeOptions" :key="type" :value="type" :label="typeLabel(type)" />
      </el-select>
      <el-input v-model="caseFilter" clearable placeholder="搜索案件号或变更内容" size="small" style="width: 240px" aria-label="搜索变更内容" />
    </div>

    <el-alert v-if="error" :title="error" type="error" :closable="false">
      <el-button text @click="load">重试</el-button>
    </el-alert>

    <EmptyState
      v-else-if="!loading && !filtered.length"
      type="custom"
      compact
      hide-action
      :title="events.length ? '当前筛选下没有记录' : '暂无操作记录'"
      :description="events.length ? '调整筛选条件试试' : '开庭改期、事实变更或 AI 写操作后，这里会出现对应留痕'"
    />

    <ol v-else class="audit-list">
      <li v-for="item in filtered" :key="item.id" class="audit-row">
        <div class="audit-row-head ui-row ui-row--between ui-row--wrap">
          <span class="audit-type">{{ typeLabel(item.eventType) }}</span>
          <span class="audit-meta">
            <el-tag size="small" :type="item.actor === 'ai' || item.actor === 'mcp' ? 'warning' : 'info'">{{ actorLabel(item.actor) }}</el-tag>
            <span class="audit-time">{{ timeOf(item) }}</span>
          </span>
        </div>
        <p v-if="changeSummary(item)" class="audit-change">{{ changeSummary(item) }}</p>
        <el-button text type="primary" size="small" @click="openCase(item)">查看案件 →</el-button>
      </li>
    </ol>
  </section>
</template>

<style scoped>
.audit-page { gap: 14px; }
.audit-head h2 { margin: 0; font-size: 16px; }
.audit-sub { margin: 4px 0 0; font-size: 12px; color: var(--c-text-secondary); line-height: 1.6; }
.audit-filters { gap: 8px; }
.audit-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.audit-row { padding: 10px 12px; border: 1px solid var(--c-border-light); border-radius: 8px; }
.audit-row-head { gap: 8px; }
.audit-type { font-size: 13px; font-weight: 600; }
.audit-meta { display: inline-flex; align-items: center; gap: 8px; font-size: 11px; color: var(--c-text-secondary); }
.audit-time { font-variant-numeric: tabular-nums; }
.audit-change { margin: 6px 0 4px; font-size: 12px; color: var(--c-text-regular); line-height: 1.7; word-break: break-all; }
</style>
