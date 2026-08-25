<template>
  <div class="projects-page">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <h3>项目</h3>
        <span class="shortcut-hint">个人项目直管 · 法律项目由案件管理镜像</span>
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
      <!-- 个人项目 -->
      <section class="proj-section">
        <div class="section-head">
          <span class="section-title">个人项目</span>
          <el-button size="small" type="primary" text @click="startCreate">
            <el-icon><Plus /></el-icon> 新建
          </el-button>
        </div>

        <!-- 新建行 -->
        <div v-if="creating" class="proj-create">
          <el-input
            ref="createInputRef"
            v-model="createForm.name"
            placeholder="项目名称（如：装修、考证、副业）"
            @keyup.enter="saveCreate"
          />
          <el-button :loading="saving" type="primary" @click="saveCreate">创建</el-button>
          <el-button @click="creating = false">取消</el-button>
        </div>

        <div v-if="personalProjects.length" class="proj-list">
          <div v-for="p in personalProjects" :key="p.id" class="proj-row" data-kind="personal">
            <template v-if="editingId === p.id">
              <el-input v-model="editName" size="small" @keyup.enter="saveEdit(p)" />
              <div class="row-actions">
                <el-button size="small" type="primary" text @click="saveEdit(p)">存</el-button>
                <el-button size="small" text @click="editingId = null">取消</el-button>
              </div>
            </template>
            <template v-else>
              <span class="proj-dot personal" />
              <span class="proj-name">{{ p.name }}</span>
              <span v-if="p.description" class="proj-desc">{{ p.description }}</span>
              <span class="proj-status">{{ statusLabel(p.status) }}</span>
              <div class="row-actions">
                <el-button size="small" text @click="startEdit(p)"><el-icon><Edit /></el-icon></el-button>
                <el-button size="small" text type="danger" @click="removeProject(p)">
                  <el-icon><Delete /></el-icon>
                </el-button>
              </div>
            </template>
          </div>
        </div>
        <EmptyState
          v-else-if="!loading && !creating"
          type="custom"
          icon="📁"
          title="还没有个人项目"
          description="项目是跨越较长时间的目标容器（区别于一次性任务）——任务可归属其中获得上下文。"
          action-text="新建第一个项目"
          @action="startCreate"
        />
      </section>

      <!-- 法律项目（只读透出） -->
      <section class="proj-section">
        <div class="section-head">
          <span class="section-title">法律项目</span>
          <span class="section-note">由案件管理镜像维护，此处只读</span>
        </div>
        <div v-if="legalProjects.length" class="proj-list">
          <div v-for="p in legalProjects" :key="p.id" class="proj-row" data-kind="legal">
            <span class="proj-dot legal" />
            <span class="proj-name">{{ p.name }}</span>
            <span v-if="p.description" class="proj-desc">{{ p.description }}</span>
            <span class="proj-status">{{ statusLabel(p.status) }}</span>
            <div class="row-actions">
              <el-button size="small" text type="primary" @click="goCase(p.id)">打开案件</el-button>
            </div>
          </div>
        </div>
        <EmptyState
          v-else
          type="custom"
          title="暂无法律项目"
          description="在案件管理中创建的案件会自动出现在这里。"
          compact
        />
      </section>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Plus, Edit, Delete } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'
import EmptyState from '../../../shared/components/EmptyState.vue'

const router = useRouter()

const projects = ref([])
const loading = ref(false)
const searchQuery = ref('')

const creating = ref(false)
const createForm = ref({ name: '', description: '' })
const createInputRef = ref(null)
const saving = ref(false)

const editingId = ref(null)
const editName = ref('')

const personalProjects = computed(() => projects.value.filter(p => p.kind === 'personal'))
const legalProjects = computed(() => projects.value.filter(p => p.kind === 'legal'))

function statusLabel(s) {
  const map = { active: '进行中', paused: '暂停', done: '已完成', archived: '已归档' }
  return map[s] || s || ''
}

async function load() {
  loading.value = true
  const result = await casyContext.projects.list(searchQuery.value.trim() || undefined)
  loading.value = false
  if (result.ok) projects.value = result.data || []
  else ElMessage.error(result.error || '加载项目失败')
}

function startCreate() {
  creating.value = true
  setTimeout(() => createInputRef.value?.focus?.(), 30)
}

async function saveCreate() {
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
  const name = editName.value.trim()
  if (!name) {
    ElMessage.warning('名称不能为空')
    return
  }
  const result = await casyContext.projects.updatePersonal(p.id, { name })
  if (result.ok) {
    editingId.value = null
    await load()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function removeProject(p) {
  const result = await casyContext.projects.remove(p.id)
  if (result.ok) {
    ElMessage.success(`已删除「${p.name}」`)
    await load()
  } else {
    // 含"仍有未完成任务"的保护文案透出
    ElMessage.warning(result.error || '删除失败')
  }
}

function goCase(caseId) {
  void router.push('/cases/' + caseId)
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
  background: #fff;
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
.proj-row:hover { background: var(--gray-50); }
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
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-out);
}

.proj-empty {
  padding: 22px 14px;
  border: 1px dashed var(--c-border);
  border-radius: 8px;
  font-size: 13px;
  color: var(--c-text-secondary);
  line-height: 1.7;
  background: var(--gray-50);
}
</style>
