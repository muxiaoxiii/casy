<script setup>
import { ref, onMounted, computed } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Lock, Edit, Delete, Top, Bottom } from '@element-plus/icons-vue'

const templates = ref([])
const loading = ref(false)
const selectedTemplate = ref(null)
const editingTemplate = ref(null)
const showEditor = ref(false)
const isNewTemplate = ref(false)

// 文件命名设置
const namingSettings = ref({
  folder_naming_date_format: 'YYYY-MM-DD',
  folder_naming_case_no_format: '{case_no}_{short_id}',
  folder_naming_file_format: '{date}_{category}_{case_no}_{hash}.{ext}',
})
const namingSaving = ref(false)

const caseTypeOptions = [
  { value: 'litigation', label: '诉讼案件' },
  { value: 'patent', label: '专利案件' },
  { value: 'trademark', label: '商标案件' },
  { value: 'consultation', label: '咨询/其他' },
]

async function loadTemplates() {
  loading.value = true
  const result = await tauriCallSafe('list_folder_templates')
  loading.value = false
  if (result.ok) {
    templates.value = result.data || []
  }
}

async function loadNamingSettings() {
  const result = await tauriCallSafe('get_folder_naming_settings')
  if (result.ok && result.data) {
    namingSettings.value = {
      folder_naming_date_format: result.data.folder_naming_date_format || 'YYYY-MM-DD',
      folder_naming_case_no_format: result.data.folder_naming_case_no_format || '{case_no}_{short_id}',
      folder_naming_file_format: result.data.folder_naming_file_format || '{date}_{category}_{case_no}_{hash}.{ext}',
    }
  }
}

onMounted(async () => {
  await Promise.all([loadTemplates(), loadNamingSettings()])
})

function selectTemplate(tpl) {
  selectedTemplate.value = tpl
}

function startNewTemplate() {
  editingTemplate.value = {
    id: '',
    name: '',
    caseType: 'litigation',
    isBuiltin: 0,
    directories: [
      { id: '01', name: '材料一', desc: '描述' },
    ],
  }
  isNewTemplate.value = true
  showEditor.value = true
}

function startEditTemplate(tpl) {
  editingTemplate.value = JSON.parse(JSON.stringify(tpl))
  isNewTemplate.value = false
  showEditor.value = true
}

function addDirectory() {
  if (!editingTemplate.value) return
  const dirs = editingTemplate.value.directories
  const nextNum = String(dirs.length + 1).padStart(2, '0')
  dirs.push({ id: nextNum, name: '新目录', desc: '' })
}

function removeDirectory(index) {
  if (!editingTemplate.value) return
  editingTemplate.value.directories.splice(index, 1)
  // 重新编号
  editingTemplate.value.directories.forEach((d, i) => {
    d.id = String(i + 1).padStart(2, '0')
  })
}

function moveDirectory(index, direction) {
  const dirs = editingTemplate.value.directories
  const target = index + direction
  if (target < 0 || target >= dirs.length) return
  const temp = dirs[index]
  dirs[index] = dirs[target]
  dirs[target] = temp
  // 重新编号
  dirs.forEach((d, i) => {
    d.id = String(i + 1).padStart(2, '0')
  })
}

async function saveTemplate() {
  if (!editingTemplate.value) return
  if (!editingTemplate.value.name.trim()) {
    ElMessage.warning('请输入模板名称')
    return
  }
  const result = await tauriCallSafe('save_folder_template', {
    data: editingTemplate.value,
  })
  if (result.ok) {
    ElMessage.success(isNewTemplate.value ? '模板已创建' : '模板已保存')
    showEditor.value = false
    await loadTemplates()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function deleteTemplate(tpl) {
  if (tpl.isBuiltin) {
    ElMessage.warning('不能删除内置模板')
    return
  }
  try {
    await ElMessageBox.confirm(
      `确定删除模板「${tpl.name}」？`,
      '删除确认',
      { type: 'warning' }
    )
  } catch {
    return
  }
  const result = await tauriCallSafe('delete_folder_template', {
    templateId: tpl.id,
  })
  if (result.ok) {
    ElMessage.success('模板已删除')
    if (selectedTemplate.value?.id === tpl.id) {
      selectedTemplate.value = null
    }
    await loadTemplates()
  } else {
    ElMessage.error(result.error || '删除失败')
  }
}

async function saveNamingSettings() {
  namingSaving.value = true
  const result = await tauriCallSafe('save_folder_naming_settings', {
    data: namingSettings.value,
  })
  namingSaving.value = false
  if (result.ok) {
    ElMessage.success('命名设置已保存')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

const builtinTemplates = computed(() => templates.value.filter(t => t.isBuiltin))
const customTemplates = computed(() => templates.value.filter(t => !t.isBuiltin))
</script>

<template>
  <div class="folder-template-settings">
    <h3>文件夹模板管理</h3>
    <p class="desc">配置案件文件夹的子目录结构模板。内置模板不可编辑，可创建自定义模板。</p>

    <el-row :gutter="20">
      <!-- 模板列表 -->
      <el-col :span="10">
        <div class="template-list">
          <div class="list-header">
            <span>模板列表</span>
            <el-button type="primary" size="small" @click="startNewTemplate">
              <el-icon><Plus /></el-icon> 新建
            </el-button>
          </div>

          <div class="list-section">
            <div class="section-label">内置模板</div>
            <div
              v-for="tpl in builtinTemplates"
              :key="tpl.id"
              class="template-item"
              :class="{ active: selectedTemplate?.id === tpl.id }"
              @click="selectTemplate(tpl)"
            >
              <el-icon><Lock /></el-icon>
              <span class="tpl-name">{{ tpl.name }}</span>
              <el-tag size="small" type="info">{{ tpl.directories?.length || 0 }} 目录</el-tag>
            </div>
          </div>

          <div class="list-section" v-if="customTemplates.length">
            <div class="section-label">自定义模板</div>
            <div
              v-for="tpl in customTemplates"
              :key="tpl.id"
              class="template-item"
              :class="{ active: selectedTemplate?.id === tpl.id }"
              @click="selectTemplate(tpl)"
            >
              <span class="tpl-name">{{ tpl.name }}</span>
              <el-tag size="small" type="success">{{ tpl.directories?.length || 0 }} 目录</el-tag>
              <div class="tpl-actions">
                <el-button link size="small" @click.stop="startEditTemplate(tpl)">
                  <el-icon><Edit /></el-icon>
                </el-button>
                <el-button link size="small" type="danger" @click.stop="deleteTemplate(tpl)">
                  <el-icon><Delete /></el-icon>
                </el-button>
              </div>
            </div>
          </div>
        </div>
      </el-col>

      <!-- 预览 / 编辑 -->
      <el-col :span="14">
        <!-- 编辑器 -->
        <div v-if="showEditor && editingTemplate" class="template-editor">
          <div class="editor-header">
            <h4>{{ isNewTemplate ? '新建模板' : '编辑模板' }}</h4>
            <div>
              <el-button size="small" @click="showEditor = false">取消</el-button>
              <el-button type="primary" size="small" @click="saveTemplate">保存</el-button>
            </div>
          </div>

          <el-form label-width="80px" size="small">
            <el-form-item label="名称">
              <el-input v-model="editingTemplate.name" placeholder="模板名称" />
            </el-form-item>
            <el-form-item label="类型">
              <el-select v-model="editingTemplate.caseType">
                <el-option
                  v-for="opt in caseTypeOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                />
              </el-select>
            </el-form-item>
          </el-form>

          <div class="dir-editor">
            <div class="dir-header">
              <span>目录列表</span>
              <el-button size="small" @click="addDirectory">添加目录</el-button>
            </div>
            <div
              v-for="(dir, index) in editingTemplate.directories"
              :key="index"
              class="dir-row"
            >
              <span class="dir-id">{{ dir.id }}</span>
              <el-input
                v-model="dir.name"
                size="small"
                class="dir-name-input"
                placeholder="目录名"
              />
              <el-input
                v-model="dir.desc"
                size="small"
                class="dir-desc-input"
                placeholder="描述（可选）"
              />
              <div class="dir-btns">
                <el-button link size="small" :disabled="index === 0" @click="moveDirectory(index, -1)">
                  <el-icon><Top /></el-icon>
                </el-button>
                <el-button link size="small" :disabled="index === editingTemplate.directories.length - 1" @click="moveDirectory(index, 1)">
                  <el-icon><Bottom /></el-icon>
                </el-button>
                <el-button link size="small" type="danger" @click="removeDirectory(index)">
                  <el-icon><Delete /></el-icon>
                </el-button>
              </div>
            </div>
          </div>
        </div>

        <!-- 预览 -->
        <div v-else-if="selectedTemplate" class="template-preview">
          <div class="preview-header">
            <h4>{{ selectedTemplate.name }}</h4>
            <el-tag v-if="selectedTemplate.isBuiltin" size="small" type="info">内置</el-tag>
            <el-button
              v-else
              size="small"
              type="primary"
              @click="startEditTemplate(selectedTemplate)"
            >编辑</el-button>
          </div>
          <p class="preview-type">类型: {{ caseTypeOptions.find(o => o.value === selectedTemplate.caseType)?.label || selectedTemplate.caseType }}</p>
          <div class="dir-list">
            <div
              v-for="dir in selectedTemplate.directories"
              :key="dir.id"
              class="dir-item"
            >
              <span class="dir-id">{{ dir.id }}</span>
              <span class="dir-name">{{ dir.name }}</span>
              <span class="dir-desc" v-if="dir.desc">{{ dir.desc }}</span>
            </div>
          </div>
        </div>

        <!-- 空状态 -->
        <div v-else class="empty-preview">
          <el-empty description="选择模板查看或点击新建" :image-size="80" />
        </div>
      </el-col>
    </el-row>

    <!-- 文件命名设置 -->
    <el-divider />
    <h3>文件命名规则</h3>
    <p class="desc">配置文件归档时的命名格式。支持变量: {date}, {category}, {case_no}, {hash}, {ext}</p>

    <el-form label-width="140px" size="small" class="naming-form">
      <el-form-item label="日期格式">
        <el-input v-model="namingSettings.folder_naming_date_format" placeholder="YYYY-MM-DD" />
      </el-form-item>
      <el-form-item label="案号格式">
        <el-input v-model="namingSettings.folder_naming_case_no_format" placeholder="{case_no}_{short_id}" />
      </el-form-item>
      <el-form-item label="文件名格式">
        <el-input v-model="namingSettings.folder_naming_file_format" placeholder="{date}_{category}_{case_no}_{hash}.{ext}" />
      </el-form-item>
      <el-form-item>
        <el-button type="primary" :loading="namingSaving" @click="saveNamingSettings">
          保存命名设置
        </el-button>
      </el-form-item>
    </el-form>
  </div>
</template>

<style scoped>
.folder-template-settings {
  max-width: 900px;
}

.folder-template-settings h3 {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 600;
}

.desc {
  color: #909399;
  font-size: 13px;
  margin: 0 0 16px;
}

.template-list {
  border: 1px solid #e4e7ed;
  border-radius: 8px;
  overflow: hidden;
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  background: #f5f7fa;
  font-weight: 600;
  font-size: 14px;
}

.section-label {
  padding: 6px 12px;
  font-size: 12px;
  color: #909399;
  background: #fafafa;
  border-top: 1px solid #ebeef5;
}

.template-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: pointer;
  transition: background 0.15s;
  border-top: 1px solid #f0f0f0;
}

.template-item:hover {
  background: #f5f7fa;
}

.template-item.active {
  background: #ecf5ff;
}

.tpl-name {
  flex: 1;
  font-size: 13px;
}

.tpl-actions {
  display: flex;
  gap: 2px;
}

.template-preview,
.template-editor {
  border: 1px solid #e4e7ed;
  border-radius: 8px;
  padding: 16px;
}

.preview-header,
.editor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.preview-header h4,
.editor-header h4 {
  margin: 0;
  font-size: 15px;
}

.preview-type {
  color: #909399;
  font-size: 13px;
  margin: 0 0 12px;
}

.dir-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.dir-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  background: #f9f9f9;
  border-radius: 4px;
  font-size: 13px;
}

.dir-id {
  color: #909399;
  font-family: monospace;
  min-width: 24px;
}

.dir-name {
  font-weight: 500;
}

.dir-desc {
  color: #909399;
  font-size: 12px;
}

.dir-editor {
  margin-top: 12px;
}

.dir-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  font-size: 13px;
  font-weight: 600;
}

.dir-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
}

.dir-name-input {
  width: 150px;
}

.dir-desc-input {
  flex: 1;
}

.dir-btns {
  display: flex;
  gap: 0;
}

.empty-preview {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  border: 1px solid #e4e7ed;
  border-radius: 8px;
}

.naming-form {
  max-width: 500px;
}
</style>
