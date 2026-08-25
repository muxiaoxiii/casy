<script setup lang="ts">
/**
 * AreasDialog —— GTD 领域管理界面（A1-2）
 *
 * 职责：领域 CRUD 的纯 UI 编排，全部数据操作经 casyContext.tasks（单一通路）。
 * 设计要点：
 * - 后端 delete_area 有任务数保护（有任务即拒绝），错误文案直接透出
 * - update_area 的 description/icon 为直接赋值 → 编辑整组提交（见 services/tasks.ts 注释）
 * - 变更后 emit('changed') 由父级刷新领域缓存
 */
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Plus, Edit, Delete, Collection, Check, Close } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'

interface AreaRow {
  id: string
  name: string
  description: string | null
  icon: string | null
  sortOrder?: number
}

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'changed'): void
}>()

const visible = ref(props.modelValue)
watch(() => props.modelValue, v => (visible.value = v))
watch(visible, v => emit('update:modelValue', v))

const areas = ref<AreaRow[]>([])
const loading = ref(false)
const newName = ref('')
const newDescription = ref('')
const creating = ref(false)

// 行内编辑态
const editingId = ref<string | null>(null)
const editName = ref('')
const editDescription = ref('')
const saving = ref(false)

async function load() {
  loading.value = true
  const result = await casyContext.tasks.areas()
  loading.value = false
  if (result.ok) areas.value = (result.data as AreaRow[]) ?? []
  else ElMessage.error(result.error || '加载领域失败')
}

watch(visible, v => {
  if (v) void load()
})

async function createArea() {
  const name = newName.value.trim()
  if (!name) {
    ElMessage.warning('请输入领域名称')
    return
  }
  creating.value = true
  const result = await casyContext.tasks.createArea({
    name,
    description: newDescription.value.trim() || null,
  })
  creating.value = false
  if (result.ok) {
    ElMessage.success(`已创建领域「${name}」`)
    newName.value = ''
    newDescription.value = ''
    await load()
    emit('changed')
  } else {
    ElMessage.error(result.error || '创建失败')
  }
}

function startEdit(area: AreaRow) {
  editingId.value = area.id
  editName.value = area.name
  editDescription.value = area.description ?? ''
}

function cancelEdit() {
  editingId.value = null
}

async function saveEdit() {
  if (!editingId.value) return
  const name = editName.value.trim()
  if (!name) {
    ElMessage.warning('名称不能为空')
    return
  }
  saving.value = true
  // 整组提交：后端 description/icon 非空即覆盖
  const result = await casyContext.tasks.updateArea(editingId.value, {
    name,
    description: editDescription.value.trim() || null,
  })
  saving.value = false
  if (result.ok) {
    editingId.value = null
    ElMessage.success('已保存')
    await load()
    emit('changed')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function removeArea(area: AreaRow) {
  const result = await casyContext.tasks.removeArea(area.id)
  if (result.ok) {
    ElMessage.success(`已删除领域「${area.name}」`)
    await load()
    emit('changed')
  } else {
    // 后端保护文案："该领域下有 N 个任务，无法删除"
    ElMessage.warning(result.error || '删除失败')
  }
}
</script>

<template>
  <el-dialog
    v-model="visible"
    title="管理领域"
    width="520px"
    :close-on-click-modal="false"
  >
    <!-- 新建 -->
    <div class="area-create">
      <el-input
        v-model="newName"
        placeholder="新领域名称（如：客户开发、专业进修）"
        size="default"
        @keyup.enter="createArea"
      />
      <el-input
        v-model="newDescription"
        placeholder="描述（可选）"
        size="default"
        @keyup.enter="createArea"
      />
      <el-button type="primary" :loading="creating" @click="createArea">
        <el-icon><Plus /></el-icon>
        新建
      </el-button>
    </div>

    <!-- 列表 -->
    <div v-loading="loading" class="area-list">
      <div v-for="area in areas" :key="area.id" class="area-row">
        <template v-if="editingId === area.id">
          <el-input v-model="editName" size="small" @keyup.enter="saveEdit" />
          <el-input
            v-model="editDescription"
            size="small"
            placeholder="描述"
            @keyup.enter="saveEdit"
          />
          <div class="row-actions">
            <el-button size="small" type="primary" :loading="saving" @click="saveEdit">
              <el-icon><Check /></el-icon>
            </el-button>
            <el-button size="small" @click="cancelEdit">
              <el-icon><Close /></el-icon>
            </el-button>
          </div>
        </template>
        <template v-else>
          <el-icon class="area-icon"><Collection /></el-icon>
          <div class="area-info">
            <span class="area-name">{{ area.name }}</span>
            <span v-if="area.description" class="area-desc">{{ area.description }}</span>
          </div>
          <div class="row-actions">
            <el-button size="small" text @click="startEdit(area)">
              <el-icon><Edit /></el-icon>
            </el-button>
            <el-button size="small" text type="danger" @click="removeArea(area)">
              <el-icon><Delete /></el-icon>
            </el-button>
          </div>
        </template>
      </div>
      <div v-if="!loading && areas.length === 0" class="area-empty">
        还没有领域。领域用于划分人生的责任范围（如「执业发展」「家庭」），任务与案件都可归属其中。
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.area-create {
  display: flex;
  gap: 8px;
  margin-bottom: 16px;
}
.area-create .el-input:first-child {
  flex: 0 0 200px;
}
.area-create .el-input:nth-child(2) {
  flex: 1;
}

.area-list {
  min-height: 120px;
  max-height: 400px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.area-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  transition: background var(--motion-fast) var(--ease-out);
}
.area-row:hover {
  background: var(--c-bg-hover, var(--gray-50));
}
.area-row .el-input {
  flex: 1;
}

.area-icon {
  color: var(--c-success, #67C23A);
  flex-shrink: 0;
}
.area-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.area-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--c-text-primary, var(--c-text));
}
.area-desc {
  font-size: 12px;
  color: var(--c-text-secondary, var(--c-text-secondary));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-out);
}
.area-row:hover .row-actions {
  opacity: 1;
}
.area-empty {
  padding: 24px 12px;
  text-align: center;
  font-size: 13px;
  color: var(--c-text-secondary, var(--c-text-secondary));
  line-height: 1.6;
}
</style>
