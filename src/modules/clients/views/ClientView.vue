<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Search, ArrowRight, Refresh } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'
import type { Case } from '../../../types'

const router = useRouter()
const loading = ref(false)
const detailsLoading = ref(false)
const error = ref('')
const detailsError = ref('')
const clients = ref<Array<{ client: string; count: number }>>([])
const selectedName = ref('')
const clientCases = ref<Case[]>([])
const search = ref('')
const page = ref(1)
const total = ref(0)
const filteredClients = computed(() => clients.value.filter(c => c.client.toLowerCase().includes(search.value.trim().toLowerCase())))
let requestId = 0

async function loadClients() {
  loading.value = true
  error.value = ''
  const result = await casyContext.cases.stats()
  loading.value = false
  if (!result.ok || !result.data) { error.value = result.error || '客户案件汇总加载失败'; return }
  clients.value = result.data.byClient.filter(c => c.client.trim()).sort((a, b) => b.count - a.count)
  const selected = clients.value.find(c => c.client === selectedName.value) || clients.value[0]
  if (selected) await selectClient(selected.client)
  else { selectedName.value = ''; clientCases.value = []; total.value = 0 }
}
async function selectClient(name: string) {
  selectedName.value = name
  page.value = 1
  await loadCases()
}
async function loadCases() {
  const id = ++requestId
  detailsLoading.value = true
  detailsError.value = ''
  clientCases.value = []
  const result = await casyContext.cases.list({ client: selectedName.value, page: page.value, perPage: 20 })
  if (id !== requestId) return
  detailsLoading.value = false
  if (!result.ok || !result.data) { detailsError.value = result.error || '关联案件加载失败'; return }
  clientCases.value = result.data.items
  total.value = result.data.total
}
const trackLabel = (track: string) => ({ patent_invalidation: '专利无效', civil_tort: '民事诉讼', admin_litigation: '行政诉讼', arbitration: '仲裁', other: '其他' }[track] || track)
onMounted(loadClients)
</script>

<template>
  <div class="client-page">
    <header class="page-header"><div><h1>客户</h1><span>{{ clients.length }} 位关联客户</span></div><el-button :icon="Refresh" :loading="loading" circle title="刷新客户汇总" aria-label="刷新客户汇总" @click="loadClients" /></header>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <div class="client-layout" v-loading="loading">
      <aside class="client-list-panel">
        <el-input v-model="search" :prefix-icon="Search" placeholder="搜索客户" aria-label="搜索客户" clearable />
        <nav aria-label="关联客户" class="client-list">
          <button v-for="client in filteredClients" :key="client.client" class="client-item" :class="{ active: selectedName === client.client }" :aria-pressed="selectedName === client.client" @click="selectClient(client.client)">
            <span class="client-name">{{ client.client }}</span><span class="count">{{ client.count }}</span>
          </button>
        </nav>
        <p v-if="!loading && !filteredClients.length" class="empty">{{ search ? '没有匹配的客户' : '暂无关联客户' }}</p>
      </aside>
      <section class="client-detail" v-loading="detailsLoading">
        <template v-if="selectedName">
          <header class="detail-header"><h2>{{ selectedName }}</h2><span>{{ total }} 件案件</span></header>
          <el-alert v-if="detailsError" :title="detailsError" type="error" :closable="false" />
          <div v-else class="case-list">
            <button v-for="item in clientCases" :key="item.id" class="case-row" @click="router.push({ name: 'case-detail', params: { id: item.id } })">
              <span class="case-copy"><strong>{{ item.caseName }}</strong><span>{{ item.caseNo || '案号未填写' }}</span></span>
              <span class="case-track">{{ trackLabel(item.track) }}</span><span class="case-status" :class="{ closed: item.caseStatus === '已完结' }">{{ item.caseStatus }}</span><el-icon><ArrowRight /></el-icon>
            </button>
          </div>
          <el-pagination v-if="total > 20" v-model:current-page="page" :page-size="20" :total="total" layout="prev, pager, next" @current-change="loadCases" />
        </template>
        <div v-else-if="!loading" class="empty"><p>暂无客户案件</p><el-button text @click="router.push('/cases')">查看案件<el-icon><ArrowRight /></el-icon></el-button></div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.client-page { max-width: 1320px; margin: 0 auto; padding: 28px 32px; height: 100%; display: flex; flex-direction: column; }
.page-header, .detail-header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
.page-header { padding-bottom: 24px; }
.page-header h1 { font-size: 24px; margin: 0 0 4px; }
.page-header span, .detail-header span, .count { font-size: 12px; color: var(--c-text-secondary); font-variant-numeric: tabular-nums; }
.client-layout { display: grid; grid-template-columns: 230px minmax(0, 1fr); flex: 1; min-height: 0; border-top: 1px solid var(--c-border); }
.client-list-panel { padding: 20px 20px 0 0; border-right: 1px solid var(--c-border); overflow-y: auto; }
.client-list { display: flex; flex-direction: column; gap: 4px; margin-top: 16px; }
.client-item { display: flex; justify-content: space-between; align-items: center; gap: 12px; width: 100%; border: 0; background: transparent; padding: 12px; border-radius: 6px; color: var(--c-text); cursor: pointer; font: inherit; text-align: left; }
.client-item:hover { background: var(--c-bg-hover); }
.client-item.active { background: var(--c-bg-selected); color: var(--c-primary); }
.client-name { min-width: 0; overflow-wrap: anywhere; }
.client-detail { padding: 20px 0 0 24px; min-width: 0; overflow-y: auto; }
.detail-header { margin-bottom: 20px; flex-wrap: wrap; }
.detail-header h2 { font-size: 18px; overflow-wrap: anywhere; margin: 0; }
.case-row { display: flex; align-items: center; gap: 16px; width: 100%; border: 0; border-bottom: 1px solid var(--c-border-light); background: transparent; padding: 16px 0; text-align: left; color: var(--c-text); font: inherit; cursor: pointer; }
.case-row:hover { background: var(--c-bg-hover); }
.case-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 4px; overflow-wrap: anywhere; }
.case-copy strong { font-size: 13px; font-weight: 600; }
.case-copy > span, .case-track, .case-status { font-size: 12px; color: var(--c-text-secondary); }
.case-status.closed { color: var(--c-success); }
.empty { padding: 32px 0; font-size: 13px; color: var(--c-text-secondary); }
.el-pagination { margin-top: 20px; }
@media (max-width: 800px) { .client-page { padding: 20px 16px; height: auto; }.client-layout { grid-template-columns: 1fr; }.client-list-panel { border-right: 0; padding: 16px 0; }.client-list { max-height: 180px; overflow: auto; }.client-detail { border-top: 1px solid var(--c-border); padding: 20px 0; }.case-track { display: none; } }
</style>
