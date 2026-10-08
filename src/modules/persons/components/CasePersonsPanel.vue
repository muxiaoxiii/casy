<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Link, Edit } from '../../../shared/icons'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'
import EmptyState from '../../../shared/components/EmptyState.vue'
import PersonFormDrawer from './PersonFormDrawer.vue'
import {
  KIND_META,
  KIND_OPTIONS,
  kindMeta,
  summarize,
  type CasePersonDto,
  type PersonDto,
  type PersonKind,
} from '../types'

/**
 * 案件侧实体面板（自包含，由案件详情挂载）
 *
 * 契约：
 *   props:  caseId: string
 *   emits:  change —— 挂载 / 解除 / 编辑实体后触发，供宿主刷新案件概要等
 */
const props = defineProps<{ caseId: string }>()
const emit = defineEmits<{ (e: 'change'): void }>()

const links = ref<CasePersonDto[]>([])
const loading = ref(false)

/** 按类型分组（固定顺序） */
const grouped = computed(() => {
  const map = new Map<PersonKind, CasePersonDto[]>()
  for (const k of KIND_OPTIONS) map.set(k, [])
  for (const l of links.value) {
    const kind = KIND_OPTIONS.includes(l.person.kind) ? l.person.kind : 'contact'
    map.get(kind)!.push(l)
  }
  return KIND_OPTIONS.filter((k) => map.get(k)!.length > 0).map((k) => ({
    kind: k,
    meta: KIND_META[k],
    items: map.get(k)!,
  }))
})

async function load() {
  loading.value = true
  const data = await tauriCall('list_case_persons', { caseId: props.caseId })
  links.value = data ?? []
  loading.value = false
}

onMounted(load)
watch(() => props.caseId, load)

// ---------- 挂载对话框 ----------
const attachVisible = ref(false)
const attachMode = ref<'existing' | 'new'>('existing')
const role = ref('')
const attaching = ref(false)

// 选择已有
const selectedPersonId = ref('')
const candidates = ref<PersonDto[]>([])
const searching = ref(false)

// 快速新建
const newKind = ref<PersonKind>('judge')
const newName = ref('')
const newOrg = ref('')

const attachedIds = computed(() => new Set(links.value.map((l) => l.person.id)))
const attachCandidates = computed(() => candidates.value.filter((p) => !attachedIds.value.has(p.id)))

function openAttach() {
  attachMode.value = 'existing'
  selectedPersonId.value = ''
  role.value = ''
  newKind.value = 'judge'
  newName.value = ''
  newOrg.value = ''
  attachVisible.value = true
  searchPersons('')
}

async function searchPersons(keyword: string) {
  searching.value = true
  const data = await tauriCall('list_persons', {
    kind: null,
    keyword: keyword || null,
  })
  candidates.value = data ?? []
  searching.value = false
}

async function confirmAttach() {
  attaching.value = true
  try {
    let personId = selectedPersonId.value

    if (attachMode.value === 'new') {
      if (!newName.value.trim()) {
        ElMessage.warning('请填写名称')
        return
      }
      const res = await tauriCallSafe('upsert_person', {
        id: null,
        kind: newKind.value,
        name: newName.value.trim(),
        org: newOrg.value.trim() || null,
        phone: null,
        email: null,
        preferences: null,
        notes: null,
      })
      if (!res.ok || !res.data) {
        ElMessage.error(res.error || '创建实体失败')
        return
      }
      personId = res.data
    }

    if (!personId) {
      ElMessage.warning('请选择要挂载的实体')
      return
    }

    const res = await tauriCallSafe('attach_person_to_case', {
      caseId: props.caseId,
      personId,
      role: role.value.trim() || null,
    })
    if (!res.ok) {
      ElMessage.error(res.error || '挂载失败')
      return
    }
    ElMessage.success('已挂载')
    attachVisible.value = false
    await load()
    emit('change')
  } finally {
    attaching.value = false
  }
}

// ---------- 解除挂载 ----------
const detachingId = ref('')

async function detach(item: CasePersonDto) {
  try {
    await ElMessageBox.confirm(
      `确定将「${item.person.name}」从本案解除挂载吗？实体档案本身会保留。`,
      '解除挂载',
      { type: 'warning', confirmButtonText: '解除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  detachingId.value = item.linkId
  const res = await tauriCallSafe('detach_person_from_case', { linkId: item.linkId })
  detachingId.value = ''
  if (!res.ok) {
    ElMessage.error(res.error || '解除失败')
    return
  }
  ElMessage.success('已解除挂载')
  await load()
  emit('change')
}

// ---------- 编辑实体（单一事实源：改一处，全案件生效） ----------
const editVisible = ref(false)
const editingPerson = ref<PersonDto | null>(null)

function openEdit(person: PersonDto) {
  editingPerson.value = person
  editVisible.value = true
}

async function onSaved() {
  await load()
  emit('change')
}
</script>

<template>
  <div class="case-persons-panel">
    <div class="panel-header">
      <span class="panel-title ui-text ui-text--strong">
        涉案人员
        <span class="panel-count">{{ links.length }}</span>
      </span>
      <el-button size="small" type="primary" plain :icon="Link" @click="openAttach">
        挂载实体
      </el-button>
    </div>

    <div v-loading="loading" class="panel-body">
      <template v-if="grouped.length">
        <div v-for="group in grouped" :key="group.kind" class="kind-group">
          <div class="kind-group-title" :style="{ color: group.meta.color }">
            <el-icon :size="13"><component :is="group.meta.icon" /></el-icon>
            {{ group.meta.label }}
          </div>
          <div
            v-for="item in group.items"
            :key="item.linkId"
            class="person-row"
          >
            <div
              class="person-avatar"
              :style="{ background: group.meta.color + '1A', color: group.meta.color }"
            >
              <el-icon :size="14"><component :is="group.meta.icon" /></el-icon>
            </div>
            <div class="person-info">
              <div class="person-name-row ui-row">
                <span class="person-name">{{ item.person.name }}</span>
                <span v-if="item.role" class="role-tag">{{ item.role }}</span>
              </div>
              <div v-if="item.person.org" class="person-org">{{ item.person.org }}</div>
            </div>
            <el-tooltip
              v-if="item.person.preferences"
              :content="summarize(item.person.preferences, 120)"
              placement="left"
              :show-after="200"
            >
              <span class="pref-dot" :style="{ background: group.meta.color }" />
            </el-tooltip>
            <div class="row-actions">
              <el-button size="small" text :icon="Edit" @click="openEdit(item.person)" />
              <el-button
                size="small"
                text
                type="danger"
                :loading="detachingId === item.linkId"
                @click="detach(item)"
              >
                解除
              </el-button>
            </div>
          </div>
        </div>
      </template>

      <EmptyState
        v-else-if="!loading"
        type="custom"
        :icon="Link"
        title="尚未挂载任何实体"
        description="挂载法官、客户、对方律师等，实体档案全局共享"
        action-text="挂载实体"
        compact
        @action="openAttach"
      />
    </div>

    <!-- 挂载对话框 -->
    <el-dialog v-model="attachVisible" title="挂载实体" width="480px" append-to-body>
      <el-radio-group v-model="attachMode" size="small" style="margin-bottom: 14px">
        <el-radio-button value="existing">选择已有实体</el-radio-button>
        <el-radio-button value="new">快速新建</el-radio-button>
      </el-radio-group>

      <template v-if="attachMode === 'existing'">
        <el-select
          v-model="selectedPersonId"
          filterable
          remote
          clearable
          :remote-method="searchPersons"
          :loading="searching"
          placeholder="搜索姓名或机构…"
          style="width: 100%"
        >
          <el-option
            v-for="p in attachCandidates"
            :key="p.id"
            :value="p.id"
            :label="p.name"
          >
            <span class="option-row ui-row">
              <el-icon :size="13" :color="kindMeta(p.kind).color">
                <component :is="kindMeta(p.kind).icon" />
              </el-icon>
              <span class="option-name">{{ p.name }}</span>
              <span class="option-org">{{ p.org || kindMeta(p.kind).label }}</span>
            </span>
          </el-option>
          <template #empty>
            <div class="option-empty ui-text ui-text--secondary">无可挂载的实体（已挂载的不会重复显示）</div>
          </template>
        </el-select>
      </template>

      <template v-else>
        <el-form label-position="top" size="default">
          <el-form-item label="类型" required>
            <el-select v-model="newKind" style="width: 100%">
              <el-option
                v-for="k in KIND_OPTIONS"
                :key="k"
                :value="k"
                :label="KIND_META[k].label"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="名称" required>
            <el-input v-model="newName" placeholder="姓名 / 机构名称" maxlength="50" />
          </el-form-item>
          <el-form-item label="机构 / 单位">
            <el-input v-model="newOrg" placeholder="选填" maxlength="100" />
          </el-form-item>
        </el-form>
      </template>

      <el-input
        v-model="role"
        placeholder="本案角色（选填），如：主审法官 / 原告代理人"
        maxlength="50"
        style="margin-top: 12px"
      />

      <template #footer>
        <el-button @click="attachVisible = false">取消</el-button>
        <el-button type="primary" :loading="attaching" @click="confirmAttach">挂载</el-button>
      </template>
    </el-dialog>

    <!-- 编辑实体（改 preferences 等，全局同步生效） -->
    <PersonFormDrawer v-model="editVisible" :person="editingPerson" @saved="onSaved" />
  </div>
</template>

<style scoped>
.case-persons-panel {
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--c-border-light);
}

.panel-count {
  font-size: 11px;
  font-weight: 400;
  color: var(--c-text-secondary);
  margin-left: 4px;
}

.panel-body {
  min-height: 80px;
}

.kind-group {
  padding: 8px 0 4px;
}

.kind-group-title {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 600;
  padding: 0 16px 4px;
  letter-spacing: 0.02em;
}

.person-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  transition: background var(--motion-fast, 0.15s ease);
}

.person-row:hover {
  background: var(--c-bg-hover);
}

.person-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.person-info {
  flex: 1;
  min-width: 0;
}

.person-name-row {
  gap: 6px;
}

.person-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.role-tag {
  display: inline-flex;
  align-items: center;
  height: 17px;
  padding: 0 6px;
  border-radius: var(--c-radius-full);
  font-size: 10px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  white-space: nowrap;
}

.person-org {
  font-size: 11px;
  color: var(--c-text-secondary);
  margin-top: 1px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pref-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  cursor: help;
}

.row-actions {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.option-row {
  gap: 7px;
}

.option-name {
  font-weight: 500;
}

.option-org {
  font-size: 11px;
  color: var(--c-text-secondary);
}

.option-empty {
  padding: 12px;
  text-align: center;
}
</style>
