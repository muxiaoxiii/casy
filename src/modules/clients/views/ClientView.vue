<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { User } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'
import EmptyState from '../../../shared/components/EmptyState.vue'

const router = useRouter()
const loading = ref(false)
const clients = ref([])
const selectedClient = ref(null)
const clientCases = ref([])
const clientTasks = ref([])

const searchQuery = ref('')

const filteredClients = computed(() => {
  if (!searchQuery.value) return clients.value
  const q = searchQuery.value.toLowerCase()
  return clients.value.filter(c => c.name.toLowerCase().includes(q))
})

const clientStats = computed(() => {
  if (!selectedClient.value) return null
  const cases = clientCases.value
  return {
    total: cases.length,
    active: cases.filter(c => c.caseStatus !== '已完结').length,
    closed: cases.filter(c => c.caseStatus === '已完结').length,
    tasks: clientTasks.value.filter(t => !t.completed).length,
    overdue: clientTasks.value.filter(t => {
      const due = t.dueDate || t.deadline
      return due && due < new Date().toISOString().split('T')[0] && !t.completed
    }).length,
  }
})

onMounted(async () => {
  loading.value = true
  await loadClients()
  loading.value = false
})

async function loadClients() {
  const result = await casyContext.cases.list({})
  if (result.ok && result.data) {
    // 按客户聚合
    const clientMap = {}
    for (const c of (result.data.items || result.data)) {
      const name = c.clientName || c.client_name || '未知客户'
      if (!clientMap[name]) {
        clientMap[name] = { name, cases: [], caseCount: 0 }
      }
      clientMap[name].cases.push(c)
      clientMap[name].caseCount++
    }
    clients.value = Object.values(clientMap).sort((a, b) => b.caseCount - a.caseCount)
  }
}

async function selectClient(client) {
  selectedClient.value = client
  clientCases.value = client.cases

  // 一次拉取全部任务，前端按案件过滤（避免逐案件 N+1 调用）
  const result = await casyContext.tasks.list({})
  if (result.ok && result.data) {
    const caseIds = new Set(client.cases.map(c => c.id))
    clientTasks.value = result.data.filter(t => caseIds.has(t.caseId))
  } else {
    clientTasks.value = []
  }
}

function goToCase(caseId) {
  router.push({ name: 'case-detail', params: { id: caseId } })
}

function getTrackColor(track) {
  const map = {
    patent_invalidation: '#6C6A9C',
    civil_tort: '#3E5C9A',
    admin_litigation: '#B0823A',
    other: '#9BA2AF',
  }
  return map[track] || '#9BA2AF'
}

function getTrackLabel(track) {
  const map = {
    patent_invalidation: '专利无效',
    civil_tort: '民事侵权',
    admin_litigation: '行政诉讼',
    other: '其他',
  }
  return map[track] || '其他'
}

function getStatusColor(status) {
  if (status === '已完结') return '#4C8067'
  if (status === '等待中') return '#B0823A'
  return '#3E5C9A'
}
</script>

<template>
  <div class="client-page fade-in">
    <div class="client-layout">
      <!-- 左栏：客户列表 -->
      <div class="client-list-panel">
        <div class="client-list-header">
          <h3>客户管理</h3>
          <span class="client-count">{{ clients.length }} 个客户</span>
        </div>

        <div class="client-search">
          <input v-model="searchQuery" placeholder="搜索客户…" />
        </div>

        <div class="client-list" v-loading="loading">
          <div
            v-for="client in filteredClients"
            :key="client.name"
            :class="['client-item', { active: selectedClient?.name === client.name }]"
            @click="selectClient(client)"
          >
            <div class="client-avatar">{{ client.name[0] }}</div>
            <div class="client-info">
              <div class="client-name">{{ client.name }}</div>
              <div class="client-meta">{{ client.caseCount }} 个案件</div>
            </div>
          </div>
        </div>
      </div>

      <!-- 右栏：客户详情 -->
      <div class="client-detail-panel">
        <template v-if="selectedClient && clientStats">
          <!-- 客户概要 -->
          <div class="card" style="margin-bottom: 16px">
            <div style="display: flex; align-items: center; gap: 16px">
              <div class="client-avatar-lg">{{ selectedClient.name[0] }}</div>
              <div style="flex: 1">
                <h2 style="margin: 0; font-size: 20px; font-weight: 700; color: #1F2430">{{ selectedClient.name }}</h2>
                <div style="font-size: 13px; color: #9BA2AF; margin-top: 4px">
                  {{ clientStats.total }} 个案件 · {{ clientStats.active }} 进行中 · {{ clientStats.closed }} 已结案
                </div>
              </div>
            </div>
          </div>

          <!-- 统计卡片 -->
          <div class="stat-cards" style="margin-bottom: 16px">
            <div class="stat-card">
              <div class="stat-value">{{ clientStats.total }}</div>
              <div class="stat-label">总案件</div>
            </div>
            <div class="stat-card">
              <div class="stat-value" style="color: #3E5C9A">{{ clientStats.active }}</div>
              <div class="stat-label">进行中</div>
            </div>
            <div class="stat-card">
              <div class="stat-value" style="color: #4C8067">{{ clientStats.closed }}</div>
              <div class="stat-label">已结案</div>
            </div>
            <div class="stat-card">
              <div class="stat-value" style="color: #B4554F">{{ clientStats.overdue }}</div>
              <div class="stat-label">逾期任务</div>
            </div>
          </div>

          <!-- 案件列表 -->
          <div class="card">
            <div class="card-header">
              <span>名下案件</span>
              <span class="sub">{{ clientStats.active }} 进行中</span>
            </div>
            <div>
              <div
                v-for="c in clientCases"
                :key="c.id"
                class="case-row"
                @click="goToCase(c.id)"
              >
                <div style="flex: 1; min-width: 0">
                  <div style="font-size: 13px; font-weight: 500; color: #1F2430">{{ c.caseName }}</div>
                  <div style="font-size: 11px; color: #9BA2AF; margin-top: 2px">{{ c.caseNo || '—' }}</div>
                </div>
                <span class="tag" :style="{ background: getTrackColor(c.track) + '20', color: getTrackColor(c.track) }">
                  {{ getTrackLabel(c.track) }}
                </span>
                <span class="tag" :style="{ background: getStatusColor(c.caseStatus) + '20', color: getStatusColor(c.caseStatus) }">
                  {{ c.caseStatus }}
                </span>
              </div>
            </div>
          </div>

          <!-- 关联任务 -->
          <div class="card" style="margin-top: 16px">
            <div class="card-header">
              <span>关联任务</span>
              <span class="sub">{{ clientStats.tasks }} 未完成</span>
            </div>
            <div>
              <div
                v-for="task in clientTasks.filter(t => !t.completed).slice(0, 10)"
                :key="task.id"
                class="case-row"
              >
                <span class="check" />
                <div style="flex: 1; min-width: 0">
                  <div style="font-size: 13px; color: #1F2430">{{ task.taskName }}</div>
                  <div style="font-size: 11px; color: #9BA2AF; margin-top: 2px">
                    {{ task.caseId }} · {{ task.dueDate || task.deadline || '无截止' }}
                  </div>
                </div>
              </div>
              <div v-if="clientTasks.filter(t => !t.completed).length === 0" class="empty-state" style="padding: 24px">
                无关联任务
              </div>
            </div>
          </div>
        </template>

        <div v-else class="empty-state" style="height: 100%; display: flex; align-items: center; justify-content: center">
          <EmptyState
            type="custom"
            :icon="User"
            title="选择客户查看详情"
            description="查看名下所有案件、任务和文书"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.client-page {
  height: 100%;
}

.client-layout {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 16px;
  height: 100%;
}

.client-list-panel {
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.client-list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid #EEF0F3;
}

.client-list-header h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: #1F2430;
}

.client-count {
  font-size: 11px;
  color: #9BA2AF;
}

.client-search {
  padding: 8px 12px;
  border-bottom: 1px solid #EEF0F3;
}

.client-search input {
  width: 100%;
  border: 1px solid #E0E3E9;
  border-radius: 6px;
  padding: 6px 10px;
  font-size: 13px;
  outline: none;
  font-family: inherit;
}

.client-search input:focus {
  border-color: #3E5C9A;
}

.client-list {
  flex: 1;
  overflow-y: auto;
}

.client-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  cursor: pointer;
  transition: background 0.15s;
  border-left: 3px solid transparent;
}

.client-item:hover {
  background: #F6F7F9;
}

.client-item.active {
  background: #EDF1F8;
  border-left-color: #3E5C9A;
}

.client-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: #EDF1F8;
  color: #3E5C9A;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 600;
  flex-shrink: 0;
}

.client-name {
  font-size: 13px;
  font-weight: 500;
  color: #1F2430;
}

.client-meta {
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 1px;
}

.client-detail-panel {
  overflow-y: auto;
}

.client-avatar-lg {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: #EDF1F8;
  color: #3E5C9A;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  font-weight: 600;
  flex-shrink: 0;
}

.stat-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}

.stat-card {
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 14px 16px;
  text-align: center;
}

.stat-value {
  font-size: 24px;
  font-weight: 700;
  color: #1F2430;
  line-height: 1;
}

.stat-label {
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 4px;
}

.case-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  border-bottom: 1px solid #EEF0F3;
  cursor: pointer;
  transition: background 0.15s;
}

.case-row:last-child { border-bottom: none; }
.case-row:hover { background: #F6F7F9; }

.check {
  width: 15px;
  height: 15px;
  border: 1.5px solid #CDD2DB;
  border-radius: 4px;
  flex-shrink: 0;
}

.tag {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 7px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 500;
  white-space: nowrap;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 700;
  color: #1F2430;
  padding: 14px 16px;
  border-bottom: 1px solid #EEF0F3;
}

.card-header .sub {
  font-weight: 400;
  color: #9BA2AF;
  font-size: 11px;
  margin-left: auto;
}

.empty-state {
  color: #9BA2AF;
  font-size: 13px;
}
</style>
