<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Search, Edit, Delete, Phone, Message, User } from '@element-plus/icons-vue'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'
import EmptyState from '../../../shared/components/EmptyState.vue'
import PersonFormDrawer from '../components/PersonFormDrawer.vue'
import PersonDetailDrawer from '../components/PersonDetailDrawer.vue'
import {
  KIND_META,
  KIND_OPTIONS,
  kindMeta,
  formatPersonTime,
  type PersonDto,
  type PersonKind,
} from '../types'

const loading = ref(false)
const persons = ref<PersonDto[]>([])

// 顶部筛选
const activeKind = ref<'' | PersonKind>('')
const keyword = ref('')

let searchTimer: ReturnType<typeof setTimeout> | null = null

async function load() {
  loading.value = true
  const data = await tauriCall('list_persons', {
    kind: activeKind.value || null,
    keyword: keyword.value.trim() || null,
  })
  persons.value = data ?? []
  loading.value = false
}

onMounted(load)

watch(activeKind, load)
watch(keyword, () => {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(load, 300)
})

const totalCount = computed(() => persons.value.length)

// ---------- 新建 / 编辑 ----------
const formVisible = ref(false)
const editingPerson = ref<PersonDto | null>(null)

function openCreate() {
  editingPerson.value = null
  formVisible.value = true
}

function openEdit(p: PersonDto) {
  editingPerson.value = p
  formVisible.value = true
}

async function onSaved() {
  await load()
}

// ---------- 删除 ----------
async function remove(p: PersonDto) {
  try {
    await ElMessageBox.confirm(
      `确定删除「${p.name}」吗？将同时解除其在全部案件中的挂载，此操作不可恢复。`,
      '删除实体',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  const res = await tauriCallSafe('delete_person', { id: p.id })
  if (!res.ok) {
    ElMessage.error(res.error || '删除失败')
    return
  }
  ElMessage.success('已删除')
  if (detailPersonId.value === p.id) detailPersonId.value = ''
  await load()
}

// ---------- 详情抽屉 ----------
const detailVisible = ref(false)
const detailPersonId = ref('')

const detailPerson = computed<PersonDto | null>(
  () => persons.value.find((p) => p.id === detailPersonId.value) ?? null
)

function openDetail(p: PersonDto) {
  detailPersonId.value = p.id
  detailVisible.value = true
}

function onDetailEdit(p: PersonDto) {
  detailVisible.value = false
  openEdit(p)
}

async function onDetailChanged() {
  await load()
}
</script>

<template>
  <div class="persons-page fade-in">
    <!-- 顶部工具栏 -->
    <div class="page-header">
      <div class="page-title-row">
        <h2 class="page-title">实体管理</h2>
        <span class="page-count">{{ totalCount }} 个实体</span>
      </div>
      <el-button type="primary" :icon="Plus" @click="openCreate">新建实体</el-button>
    </div>

    <!-- 类型 Tab -->
    <div class="kind-tabs">
      <button
        :class="['kind-tab', { active: activeKind === '' }]"
        @click="activeKind = ''"
      >
        全部
      </button>
      <button
        v-for="k in KIND_OPTIONS"
        :key="k"
        :class="['kind-tab', { active: activeKind === k }]"
        :style="activeKind === k ? { color: KIND_META[k].color } : {}"
        @click="activeKind = k"
      >
        <el-icon :size="14"><component :is="KIND_META[k].icon" /></el-icon>
        {{ KIND_META[k].label }}
      </button>
    </div>

    <!-- 搜索 -->
    <div class="search-row">
      <el-input
        v-model="keyword"
        :prefix-icon="Search"
        placeholder="搜索名称或机构…"
        clearable
        class="search-input"
      />
    </div>

    <!-- 实体卡片列表 -->
    <div v-loading="loading" class="persons-body">
      <div v-if="persons.length" class="person-grid">
        <div
          v-for="p in persons"
          :key="p.id"
          class="person-card"
          @click="openDetail(p)"
        >
          <div class="card-top">
            <div
              class="person-avatar"
              :style="{ background: kindMeta(p.kind).color + '1A', color: kindMeta(p.kind).color }"
            >
              <el-icon :size="18"><component :is="kindMeta(p.kind).icon" /></el-icon>
            </div>
            <div class="card-actions" @click.stop>
              <el-button size="small" text :icon="Edit" @click="openEdit(p)" />
              <el-button size="small" text type="danger" :icon="Delete" @click="remove(p)" />
            </div>
          </div>

          <div class="person-name">{{ p.name }}</div>
          <div class="person-org">{{ p.org || '—' }}</div>

          <div class="badge-row">
            <span
              class="kind-badge"
              :style="{ background: kindMeta(p.kind).color + '1A', color: kindMeta(p.kind).color }"
            >
              {{ kindMeta(p.kind).label }}
            </span>
            <span class="case-count-badge">关联 {{ p.caseCount ?? 0 }} 案</span>
          </div>

          <div class="contact-row">
            <span v-if="p.phone" class="contact-item">
              <el-icon :size="12"><Phone /></el-icon>{{ p.phone }}
            </span>
            <span v-if="p.email" class="contact-item">
              <el-icon :size="12"><Message /></el-icon>{{ p.email }}
            </span>
          </div>

          <div class="card-footer">更新于 {{ formatPersonTime(p.updatedAt) }}</div>
        </div>
      </div>

      <EmptyState
        v-else-if="!loading"
        type="custom"
        :icon="User"
        :title="keyword || activeKind ? '没有匹配的实体' : '还没有实体'"
        :description="
          keyword || activeKind
            ? '尝试调整搜索关键词或类型筛选'
            : '建立法官、客户、对方律师等对象档案，一次维护、全部案件同步可见'
        "
        :action-text="keyword || activeKind ? '' : '新建实体'"
        :hide-action="Boolean(keyword || activeKind)"
        @action="openCreate"
      />
    </div>

    <!-- 新建 / 编辑抽屉 -->
    <PersonFormDrawer
      v-model="formVisible"
      :person="editingPerson"
      :preset-kind="activeKind"
      @saved="onSaved"
    />

    <!-- 详情抽屉 -->
    <PersonDetailDrawer
      v-model="detailVisible"
      :person="detailPerson"
      @edit="onDetailEdit"
      @deleted="load"
      @changed="onDetailChanged"
    />
  </div>
</template>

<style scoped>
.persons-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}

.page-title-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.page-title {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.page-count {
  font-size: 12px;
  color: var(--c-text-secondary);
}

.kind-tabs {
  display: flex;
  gap: 6px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}

.kind-tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 30px;
  padding: 0 13px;
  border-radius: var(--c-radius-full);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--motion-fast, 0.15s ease);
}

.kind-tab:hover {
  border-color: var(--c-border-strong);
  background: var(--c-bg-hover);
}

.kind-tab.active {
  border-color: currentColor;
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}

.search-row {
  margin-bottom: 14px;
}

.search-input {
  max-width: 320px;
}

.persons-body {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.person-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 12px;
  padding-bottom: 16px;
}

.person-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 14px;
  cursor: pointer;
  transition: border-color var(--motion-fast, 0.15s ease), box-shadow var(--motion-fast, 0.15s ease);
}

.person-card:hover {
  border-color: var(--c-border-strong);
  box-shadow: 0 2px 8px rgba(25, 28, 30, 0.06);
}

.card-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 10px;
}

.person-avatar {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.card-actions {
  display: flex;
  opacity: 0;
  transition: opacity var(--motion-fast, 0.15s ease);
}

.person-card:hover .card-actions {
  opacity: 1;
}

.person-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--c-text-heading);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.person-org {
  font-size: 12px;
  color: var(--c-text-secondary);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.badge-row {
  display: flex;
  gap: 6px;
  margin-top: 10px;
}

.kind-badge {
  display: inline-flex;
  align-items: center;
  height: 19px;
  padding: 0 8px;
  border-radius: var(--c-radius-full);
  font-size: 10px;
  font-weight: 500;
}

.case-count-badge {
  display: inline-flex;
  align-items: center;
  height: 19px;
  padding: 0 8px;
  border-radius: var(--c-radius-full);
  font-size: 10px;
  background: var(--c-bg-subtle);
  color: var(--c-text-secondary);
}

.contact-row {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin-top: 10px;
  min-height: 18px;
}

.contact-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--c-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-footer {
  font-size: 11px;
  color: var(--c-text-placeholder);
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px solid var(--c-border-lighter);
}
</style>
