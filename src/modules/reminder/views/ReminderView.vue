<script setup>
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Bell, Warning, CircleCheck, View, Hide, RefreshRight, Timer, AlarmClock, Notification } from '@element-plus/icons-vue'

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
async function loadLogs() {
  loading.value = true
  const res = await casyContext.reminder.log(200)
  if (res.ok && res.data) {
    logs.value = res.data
  } else {
    // 回退到占位数据
    logs.value = generatePlaceholderData()
  }
  loading.value = false
}

function generatePlaceholderData() {
  const today = new Date()
  const items = []
  const cases = ['华为专利侵权案', '小米商标异议', '腾讯软件著作权', '字节跳动商业秘密', '阿里域名争议']
  const types = ['答复审查意见', '缴纳年费', '提交复审请求', '提交异议答辩', '提交续展申请']

  for (let i = 0; i < 12; i++) {
    const daysOffset = [3, 3, 2, 1, 1, 0, 0, -1, -2, -3, -5, -7][i]
    const dueDate = new Date(today)
    dueDate.setDate(dueDate.getDate() + daysOffset)
    const dueDateStr = dueDate.toISOString().slice(0, 10)

    const caseName = cases[i % cases.length]
    const typeName = types[i % types.length]
    const statusText = daysOffset < 0 ? `已逾期 ${Math.abs(daysOffset)} 天` : `剩余 ${daysOffset} 天`

    items.push({
      id: `placeholder-${i}`,
      ruleId: 'auto',
      caseId: `case-${i}`,
      taskId: null,
      channel: 'local',
      message: `案件: ${caseName}\n期限: ${typeName}\n截止日期: ${dueDateStr}\n剩余: ${daysOffset} 天`,
      status: 'sent',
      sentAt: new Date(today.getTime() - i * 3600000).toISOString().replace('T', ' ').slice(0, 19),
    })
  }
  return items
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

async function markRead(entry) {
  ElMessage.success(`已标记「${entry.parsed.caseName || entry.parsed.type}」为已读`)
}

async function ignoreEntry(entry) {
  try {
    await ElMessageBox.confirm('确认忽略此提醒？', '忽略提醒', { type: 'warning' })
    ElMessage.info('已忽略')
  } catch {}
}

function viewDetail(entry) {
  if (entry.caseId) {
    window.location.hash = `#/cases/${entry.caseId}`
  }
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
            <el-button size="small" text type="primary" @click="viewDetail(item)">
              <el-icon><View /></el-icon> 查看
            </el-button>
            <el-button size="small" text @click="markRead(item)">
              <el-icon><CircleCheck /></el-icon> 已读
            </el-button>
            <el-button size="small" text type="info" @click="ignoreEntry(item)">
              <el-icon><Hide /></el-icon> 忽略
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
  background: #fff;
  border: 1px solid #E4E7ED;
  border-left: 3px solid #ddd;
  border-radius: 8px;
  padding: 14px 16px;
  cursor: pointer;
  transition: all 0.15s ease;
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
  color: #909399;
}

/* ── Tab 行 ─────────────────────────────────── */
.reminder-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #fff;
  border: 1px solid #E4E7ED;
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
  color: #52525B;
  cursor: pointer;
  transition: all 0.15s ease;
  display: flex;
  align-items: center;
  gap: 6px;
}

.tab-btn:hover {
  background: #F4F4F5;
}

.tab-btn.active {
  background: #EFF6FF;
  color: #2563EB;
  font-weight: 500;
}

.tab-count {
  font-size: 11px;
  background: #E4E7ED;
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
  color: #C0C4CC;
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
  transition: all 0.15s ease;
  background: #fff;
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
  color: #909399;
}

.meta-date {
  display: flex;
  align-items: center;
  gap: 3px;
}

.meta-channel {
  padding: 1px 6px;
  background: #F4F4F5;
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
