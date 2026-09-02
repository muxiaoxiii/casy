<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Phone, Message, Edit, Delete, Right } from '@element-plus/icons-vue'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'
import EmptyState from '../../../shared/components/EmptyState.vue'
import { Briefcase } from '@element-plus/icons-vue'
import {
  kindMeta,
  formatPersonTime,
  type PersonDto,
  type PersonCaseDto,
} from '../types'

const props = defineProps<{
  modelValue: boolean
  person: PersonDto | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'edit', person: PersonDto): void
  /** 删除成功 */
  (e: 'deleted', id: string): void
  /** 挂载关系变化（父级需刷新列表以更新 caseCount） */
  (e: 'changed'): void
}>()

const router = useRouter()
const cases = ref<PersonCaseDto[]>([])
const loadingCases = ref(false)
const detachingId = ref('')

watch(
  () => [props.modelValue, props.person?.id] as const,
  async ([open, id]) => {
    if (!open || !id) return
    loadingCases.value = true
    const data = await tauriCall<PersonCaseDto[]>('list_person_cases', { personId: id })
    cases.value = data ?? []
    loadingCases.value = false
  }
)

function close() {
  emit('update:modelValue', false)
}

function goToCase(caseId: string) {
  close()
  router.push({ name: 'case-detail', params: { id: caseId } })
}

async function detach(c: PersonCaseDto) {
  try {
    await ElMessageBox.confirm(
      `确定将「${props.person?.name}」从案件「${c.caseName}」中解除挂载吗？`,
      '解除挂载',
      { type: 'warning', confirmButtonText: '解除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  detachingId.value = c.linkId
  const res = await tauriCallSafe<void>('detach_person_from_case', { linkId: c.linkId })
  detachingId.value = ''
  if (!res.ok) {
    ElMessage.error(res.error || '解除失败')
    return
  }
  cases.value = cases.value.filter((x) => x.linkId !== c.linkId)
  ElMessage.success('已解除挂载')
  emit('changed')
}

async function remove() {
  const p = props.person
  if (!p) return
  try {
    await ElMessageBox.confirm(
      `确定删除「${p.name}」吗？将同时解除其在全部案件中的挂载，此操作不可恢复。`,
      '删除实体',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  const res = await tauriCallSafe<void>('delete_person', { id: p.id })
  if (!res.ok) {
    ElMessage.error(res.error || '删除失败')
    return
  }
  ElMessage.success('已删除')
  emit('deleted', p.id)
  close()
}
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    size="480px"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <template #header>
      <span class="drawer-title">实体档案</span>
    </template>

    <template v-if="person">
      <!-- 概要 -->
      <div class="profile-head">
        <div
          class="profile-avatar"
          :style="{ background: kindMeta(person.kind).color + '1A', color: kindMeta(person.kind).color }"
        >
          <el-icon :size="24"><component :is="kindMeta(person.kind).icon" /></el-icon>
        </div>
        <div class="profile-main">
          <div class="profile-name-row">
            <span class="profile-name">{{ person.name }}</span>
            <span
              class="kind-badge"
              :style="{ background: kindMeta(person.kind).color + '1A', color: kindMeta(person.kind).color }"
            >
              {{ kindMeta(person.kind).label }}
            </span>
          </div>
          <div v-if="person.org" class="profile-org">{{ person.org }}</div>
        </div>
      </div>

      <!-- 联系方式 -->
      <div class="info-grid">
        <div class="info-item">
          <el-icon class="info-icon"><Phone /></el-icon>
          <span>{{ person.phone || '未填写电话' }}</span>
        </div>
        <div class="info-item">
          <el-icon class="info-icon"><Message /></el-icon>
          <span>{{ person.email || '未填写邮箱' }}</span>
        </div>
      </div>

      <!-- 偏好 -->
      <div v-if="person.preferences" class="section">
        <div class="section-title">偏好 / 习惯</div>
        <div class="section-body preferences">{{ person.preferences }}</div>
      </div>

      <!-- 备注 -->
      <div v-if="person.notes" class="section">
        <div class="section-title">备注</div>
        <div class="section-body">{{ person.notes }}</div>
      </div>

      <!-- 关联案件 -->
      <div class="section">
        <div class="section-title">
          关联案件
          <span class="section-count">{{ cases.length }}</span>
        </div>
        <div v-loading="loadingCases">
          <div v-if="cases.length" class="case-list">
            <div v-for="c in cases" :key="c.linkId" class="case-row" @click="goToCase(c.caseId)">
              <div class="case-info">
                <div class="case-name">{{ c.caseName }}</div>
                <div class="case-no">{{ c.caseNo || '—' }}</div>
              </div>
              <span v-if="c.role" class="role-tag">{{ c.role }}</span>
              <el-button
                size="small"
                text
                type="danger"
                :loading="detachingId === c.linkId"
                @click.stop="detach(c)"
              >
                解除
              </el-button>
              <el-icon class="case-arrow"><Right /></el-icon>
            </div>
          </div>
          <EmptyState
            v-else-if="!loadingCases"
            type="custom"
            :icon="Briefcase"
            title="暂未关联案件"
            description="可在案件详情页挂载该实体"
            hide-action
            compact
          />
        </div>
      </div>

      <div class="meta-line">
        创建于 {{ formatPersonTime(person.createdAt) }} · 更新于 {{ formatPersonTime(person.updatedAt) }}
      </div>
    </template>

    <template #footer>
      <el-button type="danger" plain :icon="Delete" @click="remove">删除</el-button>
      <el-button type="primary" :icon="Edit" @click="person && emit('edit', person)">编辑</el-button>
    </template>
  </el-drawer>
</template>

<style scoped>
.drawer-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.profile-head {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 16px;
}

.profile-avatar {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.profile-main {
  flex: 1;
  min-width: 0;
}

.profile-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.profile-name {
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.kind-badge {
  display: inline-flex;
  align-items: center;
  height: 20px;
  padding: 0 8px;
  border-radius: var(--c-radius-full);
  font-size: 11px;
  font-weight: 500;
  white-space: nowrap;
}

.profile-org {
  font-size: 13px;
  color: var(--c-text-secondary);
  margin-top: 3px;
}

.info-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-subtle);
  margin-bottom: 16px;
}

.info-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--c-text-regular);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.info-icon {
  color: var(--c-text-placeholder);
  flex-shrink: 0;
}

.section {
  margin-bottom: 16px;
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-heading);
  margin-bottom: 8px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.section-count {
  font-size: 11px;
  font-weight: 400;
  color: var(--c-text-secondary);
}

.section-body {
  font-size: 13px;
  color: var(--c-text-regular);
  line-height: 1.6;
  padding: 10px 12px;
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius);
  background: var(--c-bg-card);
  white-space: pre-wrap;
  word-break: break-word;
}

.section-body.preferences {
  background: var(--c-info-light);
  border-color: transparent;
}

.case-list {
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius-lg);
  overflow: hidden;
}

.case-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--c-border-light);
  cursor: pointer;
  transition: background var(--motion-fast, 0.15s ease);
}

.case-row:last-child {
  border-bottom: none;
}

.case-row:hover {
  background: var(--c-bg-hover);
}

.case-info {
  flex: 1;
  min-width: 0;
}

.case-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.case-no {
  font-size: 11px;
  color: var(--c-text-secondary);
  margin-top: 2px;
}

.role-tag {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 7px;
  border-radius: var(--c-radius-full);
  font-size: 10px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  white-space: nowrap;
}

.case-arrow {
  color: var(--c-text-placeholder);
  flex-shrink: 0;
}

.meta-line {
  font-size: 11px;
  color: var(--c-text-placeholder);
  margin-top: 8px;
}
</style>
