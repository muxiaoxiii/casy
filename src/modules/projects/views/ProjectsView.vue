<template>
  <div class="projects-page">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <h3>项目</h3>
        <span class="shortcut-hint">非诉、顾问、研究与其他长期事项</span>
      </div>
      <div class="toolbar-right">
        <el-input
          v-model="searchQuery"
          placeholder="搜索项目…"
          clearable
          size="small"
          style="width: 200px"
          @input="load"
        />
      </div>
    </div>

    <div v-loading="loading" class="projects-body">
      <el-alert v-if="loadError" :title="loadError" type="error" :closable="false"><el-button text @click="load">重试</el-button></el-alert>
      <!-- 非案件项目 -->
      <section class="proj-section">
        <div class="section-head">
          <span class="section-title">非案件项目</span>
          <el-button size="small" type="primary" text @click="startCreate">
            <el-icon><Plus /></el-icon> 新建
          </el-button>
        </div>

        <!-- 新建行 -->
        <div v-if="creating" class="proj-create">
          <el-input
            ref="createInputRef"
            v-model="createForm.name"
            placeholder="项目名称（如：常年顾问、尽调、专利组合）"
            @keyup.enter="saveCreate"
          />
          <el-button :loading="saving" type="primary" @click="saveCreate">创建</el-button>
          <el-button :disabled="saving" @click="creating = false">取消</el-button>
        </div>

        <div v-if="personalProjects.length" class="proj-list">
          <div v-for="p in personalProjects" :key="p.id" class="proj-row" data-kind="personal" @contextmenu="!editingId && showContextMenu($event, p.name, [{ label: '编辑项目', disabled: mutating, run: () => startEdit(p) }, { label: '删除项目', danger: true, disabled: mutating, run: () => removeProject(p) }])">
            <template v-if="editingId === p.id">
              <el-input v-model="editName" size="small" @keyup.enter="saveEdit(p)" />
              <div class="row-actions">
                <el-button size="small" type="primary" text :disabled="mutating" @click="saveEdit(p)">保存</el-button>
                <el-button size="small" text @click="editingId = null">取消</el-button>
              </div>
            </template>
            <template v-else>
              <span class="proj-dot personal" />
              <span class="proj-name">{{ p.name }}</span>
              <span v-if="p.description" class="proj-desc">{{ p.description }}</span>
              <span class="proj-status">{{ statusLabel(p.status) }}</span>
              <div class="row-actions">
                <el-button size="small" text :disabled="mutating" :aria-label="`编辑项目 ${p.name}`" @click="startEdit(p)"><el-icon><Edit /></el-icon></el-button>
                <el-button size="small" text type="danger" :disabled="mutating" :aria-label="`删除项目 ${p.name}`" @click="removeProject(p)">
                  <el-icon><Delete /></el-icon>
                </el-button>
              </div>
            </template>
          </div>
        </div>
        <EmptyState
          v-else-if="!loading && !creating && !loadError"
          type="custom"
          :title="searchQuery ? '没有匹配的项目' : '还没有非案件项目'"
          description="用于承载非诉、顾问、研究或个人长期事项；诉讼和争议案件仍在案件模块管理。"
          :action-text="searchQuery ? '清除搜索' : '新建项目'"
          @action="searchQuery ? (searchQuery = '', load()) : startCreate()"
        />
      </section>
    </div>
  </div>
  <ContextMenu v-bind="contextMenu" @close="contextMenu.open = false" />
</template>

<script setup>
import ContextMenu from "../../../shared/components/ContextMenu.vue"
import { useContextActions } from "../../../shared/composables/useContextActions"
const { contextMenu, showContextMenu } = useContextActions()

import { ref, computed, onMounted, nextTick } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Edit, Delete } from '../../../shared/icons'
import { casyContext } from '../../../core/plugin/context'
import EmptyState from '../../../shared/components/EmptyState.vue'

const projects = ref([])
const loading = ref(false)
const loadError = ref('')
const mutating = ref(false)
let loadRevision = 0
const searchQuery = ref('')

const creating = ref(false)
const createForm = ref({ name: '', description: '' })
const createInputRef = ref(null)
const saving = ref(false)

const editingId = ref(null)
const editName = ref('')

const personalProjects = computed(() => projects.value.filter(p => p.kind === 'personal'))

function statusLabel(s) {
  const map = { active: '进行中', paused: '暂停', done: '已完成', archived: '已归档' }
  return map[s] || s || ''
}

async function load() {
  const request = ++loadRevision
  loading.value = true
  loadError.value = ''
  const result = await casyContext.projects.list(searchQuery.value.trim() || undefined)
  if (request !== loadRevision) return
  loading.value = false
  if (result.ok) projects.value = result.data || []
  else loadError.value = result.error || '加载项目失败'
}

async function startCreate() {
  creating.value = true
  await nextTick()
  createInputRef.value?.focus?.()
}

async function saveCreate() {
  if (saving.value) return
  const name = createForm.value.name.trim()
  if (!name) {
    ElMessage.warning('请输入项目名称')
    return
  }
  saving.value = true
  const result = await casyContext.projects.createPersonal({
    name,
    description: createForm.value.description.trim() || null,
  })
  saving.value = false
  if (result.ok) {
    ElMessage.success(`已创建「${name}」`)
    createForm.value.name = ''
    creating.value = false
    await load()
  } else {
    ElMessage.error(result.error || '创建失败')
  }
}

function startEdit(p) {
  editingId.value = p.id
  editName.value = p.name
}

async function saveEdit(p) {
  if (mutating.value) return
  const name = editName.value.trim()
  if (!name) {
    ElMessage.warning('名称不能为空')
    return
  }
  mutating.value = true
  const result = await casyContext.projects.updatePersonal(p.id, { name })
  mutating.value = false
  if (result.ok) {
    editingId.value = null
    await load()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function removeProject(p) {
  if (mutating.value) return
  try { await ElMessageBox.confirm(`删除项目「${p.name}」？有未完成任务的项目不能删除。`, '删除项目', { type: 'warning', confirmButtonText: '删除项目', cancelButtonText: '取消' }) } catch { return }
  if (mutating.value) return
  mutating.value = true
  const result = await casyContext.projects.remove(p.id)
  mutating.value = false
  if (result.ok) {
    ElMessage.success(`已删除「${p.name}」`)
    await load()
  } else {
    // 含"仍有未完成任务"的保护文案透出
    ElMessage.warning(result.error || '删除失败')
  }
}

onMounted(() => {
  load()
})
</script>

<style scoped>
.projects-page {
  padding: 20px 24px;
  max-width: 860px;
  margin: 0 auto;
}
.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 18px;
}
.toolbar-left h3 {
  margin: 0;
  font-size: 17px;
}
.shortcut-hint {
  font-size: 12px;
  color: var(--c-text-secondary);
  margin-left: 10px;
}

.proj-section { margin-bottom: 28px; }
.section-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 8px;
}
.section-title { font-size: 14px; font-weight: 600; color: var(--c-text); }
.section-note { font-size: 12px; color: var(--c-text-secondary); }

.proj-create {
  display: flex;
  gap: 8px;
  padding: 8px 0;
}

.proj-list {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  overflow: hidden;
}
.proj-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--c-border-lighter);
  transition: background var(--motion-fast) var(--ease-out);
}
.proj-row:last-child { border-bottom: none; }
.proj-row:hover { background: var(--c-bg-subtle); }
.proj-row:hover .row-actions { opacity: 1; }

.proj-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.proj-dot.personal { background: var(--c-success); }
.proj-dot.legal { background: var(--c-info); }
.proj-name { font-size: 14px; color: var(--c-text); }
.proj-desc {
  font-size: 12px;
  color: var(--c-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.proj-status {
  margin-left: auto;
  font-size: 11px;
  color: var(--c-text-secondary);
}
.row-actions {
  display: flex;
  gap: 2px;
  opacity: 1;
  transition: opacity var(--motion-fast) var(--ease-out);
}

.proj-empty {
  padding: 22px 14px;
  border: 1px dashed var(--c-border);
  border-radius: 8px;
  font-size: 13px;
  color: var(--c-text-secondary);
  line-height: 1.7;
  background: var(--c-bg-subtle);
}
@media(max-width:600px){.toolbar{align-items:flex-start;flex-direction:column;gap:12px}.proj-row{flex-wrap:wrap}.proj-create{flex-wrap:wrap}.shortcut-hint{margin-left:0}.toolbar-right,.toolbar-right .el-input{width:100%!important}}
</style>
