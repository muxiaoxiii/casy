<script setup>
import { useRouter } from 'vue-router'
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
const router = useRouter()
import { Bell, Warning, CircleCheck, View, Hide, RefreshRight, Timer, AlarmClock, Notification } from '../../../shared/icons'

// ============================================================
// 数据
// ============================================================
const logs = ref([])
const loading = ref(false)
const activeTab = ref('all')

// ============================================================
// 解析提醒消息，提取结构化信息
// ============================================================
function parseReminderMessage(message) {
  const result = { caseName: '', type: '', dueDate: '', daysLeft: null, status: '' }
  const lines = message.split('\n')
  for (const line of lines) {
    if (line.startsWith('案件:')) result.caseName = line.replace('案件:', '').trim()
    if (line.startsWith('期限:')) result.type = line.replace('期限:', '').trim()
    if (line.startsWith('任务:')) result.type = line.replace('任务:', '').trim()
    if (line.startsWith('庭审:')) result.type = line.replace('庭审:', '').trim()
    if (line.startsWith('截止日期:')) result.dueDate = line.replace('截止日期:', '').trim()
    if (line.startsWith('日期:')) result.dueDate = line.replace('日期:', '').trim()
    if (line.startsWith('剩余:')) {
      const m = line.match(/-?\d+/)
      if (m) result.daysLeft = parseInt(m[0])
    }
    if (line.startsWith('状态:')) result.status = line.replace('状态:', '').trim()
  }
  return result
}

// ============================================================
// R1-R4 分级计算
// 优先使用后端 reminder_log.level，回退到前端解析（兼容旧数据）
// ============================================================
function classifyLevel(entry) {
  if (entry.level && ['R1', 'R2', 'R3', 'R4'].includes(entry.level)) {
    return entry.level
  }
  const parsed = parseReminderMessage(entry.message)
  const days = parsed.daysLeft

  if (days === null) {
    // 无法解析天数，根据 ruleId 关联规则或回退
    if (parsed.status.includes('逾期') || parsed.status.includes('overdue')) return 'R4'
    return 'R1'
  }
  if (days < 0) return 'R4'
  if (days === 0) return 'R3'
  if (days <= 1) return 'R2'
  return 'R1'
}

const enrichedLogs = computed(() => {
  return logs.value.map(entry => {
    const parsed = parseReminderMessage(entry.message)
    const level = classifyLevel(entry)
    return { ...entry, parsed, level }
  })
})

const levelConfig = {
  R1: { label: 'R1 温和', color: '#E6A23C', tagType: 'warning', desc: '截止前 T-3 天' },
  R2: { label: 'R2 明确', color: '#E6A23C', tagType: 'warning', desc: '截止前 T-1 天' },
  R3: { label: 'R3 强提醒', color: '#F56C6C', tagType: 'danger', desc: '到期当天' },
  R4: { label: 'R4 逾期', color: '#C00000', tagType: '', desc: '超过截止' },
}

// ============================================================
// 统计
// ============================================================
const stats = computed(() => {
  const map = { R1: 0, R2: 0, R3: 0, R4: 0 }
  for (const item of enrichedLogs.value) {
    map[item.level]++
  }
  return map
})

// ============================================================
// 当前 Tab 过滤
// ============================================================
const filteredLogs = computed(() => {
  if (activeTab.value === 'all') return enrichedLogs.value
  return enrichedLogs.value.filter(item => item.level === activeTab.value)
})

const tabs = [
  { key: 'all', label: '全部' },
  { key: 'R1', label: 'R1 温和' },
  { key: 'R2', label: 'R2 明确' },
  { key: 'R3', label: 'R3 强提醒' },
  { key: 'R4', label: 'R4 逾期' },
]

// ============================================================
// 操作
// ============================================================
const loadError = ref('')

async function loadLogs() {
  loading.value = true
  loadError.value = ''
  const res = await casyContext.reminder.log(200)
  if (res.ok && res.data) {
    logs.value = res.data
  } else {
    // 审计 P2：失败不得展示虚假"已发送"数据——显式错误态 + 可重试
    loadError.value = res.error || '提醒记录加载失败'
    logs.value = []
  }
  loading.value = false
}

function getDaysTagType(days) {
  if (days < 0) return 'danger'
  if (days === 0) return 'danger'
  if (days <= 1) return 'warning'
  return 'info'
}

function getDaysText(days) {
  if (days === null) return '-'
  if (days < 0) return `逾期 ${Math.abs(days)} 天`
  if (days === 0) return '今天到期'
  return `${days} 天后`
}

function viewDetail(entry) {
  if (entry.caseId) router.push({ name: 'case-detail', params: { id: entry.caseId } })
  else if (entry.taskId) router.push({ path: '/tasks', query: { edit: entry.taskId } })
}

// ============================================================
// 生命周期
// ============================================================
onMounted(loadLogs)
</script>

<template>
  <div class="reminder-view">
    <!-- 顶部统计卡片 -->
    <div class="stats-row">
      <div
        v-for="(cfg, key) in levelConfig"
        :key="key"
        class="stat-card"
        role="button" tabindex="0" :aria-pressed="activeTab === key" @keydown.enter="activeTab = key" @keydown.space.prevent="activeTab = key"
        :class="{ active: activeTab === key }"
        :style="{ borderColor: cfg.color }"
        @click="activeTab = key"
      >
        <div class="stat-card-head">
          <el-tag :type="cfg.tagType" size="small" effect="dark" round>{{ cfg.label }}</el-tag>
        </div>
        <div class="stat-card-count" :style="{ color: cfg.color }">{{ stats[key] }}</div>
        <div class="stat-card-desc">{{ cfg.desc }}</div>
      </div>
    </div>

    <!-- Tab 分页 + 列表 -->
    <div class="reminder-body">
      <div class="reminder-tabs">
        <div
          v-for="tab in tabs"
          :key="tab.key"
          :class="['tab-btn', { active: activeTab === tab.key }]"
          @click="activeTab = tab.key"
        >
          {{ tab.label }}
          <span v-if="tab.key !== 'all'" class="tab-count">{{ stats[tab.key] || 0 }}</span>
        </div>
        <div class="tab-spacer" />
        <el-button size="small" :icon="RefreshRight" @click="loadLogs" :loading="loading">刷新</el-button>
      </div>

      <!-- 列表 -->
      <div class="reminder-list" v-loading="loading">
        <div v-if="filteredLogs.length === 0" class="empty-state">
          <el-icon :size="48" color="#C0C4CC"><Bell /></el-icon>
          <p>暂无提醒记录</p>
        </div>

        <div
          v-for="item in filteredLogs"
          :key="item.id"
          class="reminder-item"
          :class="[`level-${item.level}`]"
        >
          <!-- 左侧级别标识 -->
          <div class="item-level" :style="{ background: levelConfig[item.level].color }">
            {{ item.level }}
          </div>

          <!-- 主体信息 -->
          <div class="item-body">
            <div class="item-top">
              <span class="item-case">{{ item.parsed.caseName || '未知案件' }}</span>
              <el-tag :type="levelConfig[item.level].tagType" size="small" effect="plain">
                {{ levelConfig[item.level].label }}
              </el-tag>
            </div>
            <div class="item-type">{{ item.parsed.type || '提醒' }}</div>
            <div class="item-meta">
              <span class="meta-date">
                <el-icon><Timer /></el-icon>
                {{ item.parsed.dueDate || '-' }}
              </span>
              <el-tag
                :type="getDaysTagType(item.parsed.daysLeft)"
                size="small"
                round
                effect="plain"
              >
                {{ getDaysText(item.parsed.daysLeft) }}
              </el-tag>
              <span class="meta-channel">{{ item.channel }}</span>
              <span class="meta-time">{{ item.sentAt || '' }}</span>
            </div>
          </div>

          <!-- 右侧操作 -->
          <div class="item-actions">
            <el-button v-if="item.caseId || item.taskId" size="small" text type="primary" @click="viewDetail(item)">
              <el-icon><View /></el-icon> 查看
            </el-button>

          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.reminder-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 20px;
  gap: 20px;
  overflow: hidden;
}

/* ── 统计卡片行 ──────────────────────────────── */
.stats-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  flex-shrink: 0;
}

.stat-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-left: 3px solid #ddd;
  border-radius: 8px;
  padding: 14px 16px;
  cursor: pointer;
  transition: all var(--motion-fast)  ease;
}

.stat-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
}

.stat-card.active {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
}

.stat-card-head {
  margin-bottom: 8px;
}

.stat-card-count {
  font-size: 28px;
  font-weight: 700;
  line-height: 1;
  margin-bottom: 4px;
}

.stat-card-desc {
  font-size: 12px;
  color: var(--gray-400);
}

/* ── Tab 行 ─────────────────────────────────── */
.reminder-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  overflow: hidden;
}

.reminder-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 10px 16px;
  border-bottom: 1px solid #F0F0F0;
  flex-shrink: 0;
}

.tab-btn {
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--c-text-regular);
  cursor: pointer;
  transition: all var(--motion-fast)  ease;
  display: flex;
  align-items: center;
  gap: 6px;
}

.tab-btn:hover {
  background: var(--gray-50);
}

.tab-btn.active {
  background: #EFF6FF;
  color: #2563EB;
  font-weight: 500;
}

.tab-count {
  font-size: 11px;
  background: var(--c-border);
  color: #606266;
  border-radius: 999px;
  padding: 1px 6px;
  min-width: 18px;
  text-align: center;
}

.tab-btn.active .tab-count {
  background: #BFDBFE;
  color: #2563EB;
}

.tab-spacer {
  flex: 1;
}

/* ── 提醒列表 ──────────────────────────────── */
.reminder-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 300px;
  color: var(--gray-300);
}

.empty-state p {
  margin-top: 12px;
  font-size: 14px;
}

.reminder-item {
  display: flex;
  align-items: stretch;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid #F0F0F0;
  border-radius: 8px;
  margin-bottom: 8px;
  transition: all var(--motion-fast)  ease;
  background: var(--c-bg-card);
}

.reminder-item:hover {
  border-color: #DCDFE6;
  box-shadow: 0 1px 6px rgba(0, 0, 0, 0.04);
}

.reminder-item.level-R4 {
  border-left: 3px solid #C00000;
  background: #FFF5F5;
}

.reminder-item.level-R3 {
  border-left: 3px solid #F56C6C;
}

.reminder-item.level-R2 {
  border-left: 3px solid #E6A23C;
}

.reminder-item.level-R1 {
  border-left: 3px solid #E6A23C;
  opacity: 0.85;
}

/* 左侧级别标识 */
.item-level {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  min-height: 36px;
  border-radius: 6px;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  flex-shrink: 0;
  align-self: center;
}

/* 主体 */
.item-body {
  flex: 1;
  min-width: 0;
}

.item-top {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.item-case {
  font-size: 14px;
  font-weight: 600;
  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-type {
  font-size: 13px;
  color: #606266;
  margin-bottom: 6px;
}

.item-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  color: var(--gray-400);
}

.meta-date {
  display: flex;
  align-items: center;
  gap: 3px;
}

.meta-channel {
  padding: 1px 6px;
  background: var(--gray-50);
  border-radius: 4px;
}

/* 右侧操作 */
.item-actions {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  flex-shrink: 0;
}
</style>
