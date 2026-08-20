<template>
  <div class="perspective-manager">
    <!-- 创建/编辑透视对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="editingPerspective ? '编辑透视' : '创建透视'"
      width="500px"
      append-to-body
    >
      <el-form :model="formData" label-width="80px">
        <el-form-item label="名称" required>
          <el-input v-model="formData.name" placeholder="如：本周紧急任务" />
        </el-form-item>
        
        <el-form-item label="图标">
          <el-input v-model="formData.icon" placeholder="emoji 图标" style="width: 80px" />
        </el-form-item>
        
        <el-form-item label="颜色">
          <el-color-picker v-model="formData.color" />
        </el-form-item>

        <el-divider content-position="left">过滤条件</el-divider>

        <el-form-item label="任务类型">
          <el-select v-model="formData.filters.taskType" clearable placeholder="不限">
            <el-option label="行动" value="action" />
            <el-option label="等待" value="waiting" />
            <el-option label="委派" value="delegated" />
            <el-option label="某天" value="someday" />
          </el-select>
        </el-form-item>

        <el-form-item label="优先级">
          <el-select v-model="formData.filters.priority" clearable placeholder="不限">
            <el-option label="重要紧急" value="urgent_important" />
            <el-option label="重要不紧急" value="important" />
            <el-option label="紧急不重要" value="urgent" />
            <el-option label="普通" value="normal" />
          </el-select>
        </el-form-item>

        <el-form-item label="上下文">
          <el-select v-model="formData.filters.context" clearable placeholder="不限">
            <el-option label="@办公室" value="office" />
            <el-option label="@电话" value="phone" />
            <el-option label="@法院" value="court" />
            <el-option label="@电脑" value="computer" />
            <el-option label="@外出" value="outside" />
          </el-select>
        </el-form-item>

        <el-form-item label="日期范围">
          <el-select v-model="formData.filters.dateRange" clearable placeholder="不限">
            <el-option label="已逾期" value="overdue" />
            <el-option label="今天" value="today" />
            <el-option label="本周" value="week" />
            <el-option label="本月" value="month" />
          </el-select>
        </el-form-item>

        <el-form-item label="旗标">
          <el-checkbox v-model="formData.filters.flagged">仅显示旗标任务</el-checkbox>
        </el-form-item>

        <el-divider content-position="left">排序</el-divider>

        <el-form-item label="排序字段">
          <el-select v-model="formData.sortBy" clearable placeholder="默认">
            <el-option label="截止日期" value="dueDate" />
            <el-option label="优先级" value="priority" />
            <el-option label="创建时间" value="createdAt" />
            <el-option label="今日排序" value="todayIndex" />
          </el-select>
        </el-form-item>

        <el-form-item v-if="formData.sortBy" label="排序方向">
          <el-radio-group v-model="formData.sortOrder">
            <el-radio value="asc">升序</el-radio>
            <el-radio value="desc">降序</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="handleSave">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, reactive, watch } from 'vue'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  perspective: {
    type: Object,
    default: null,
  },
})

const emit = defineEmits(['update:visible', 'save'])

const dialogVisible = ref(false)
const editingPerspective = ref(null)

const defaultFormData = {
  name: '',
  icon: '📋',
  color: '#409eff',
  filters: {
    taskType: null,
    priority: null,
    context: null,
    caseId: null,
    areaId: null,
    flagged: null,
    completed: false,
    dateRange: null,
  },
  sortBy: null,
  sortOrder: 'asc',
}

const formData = reactive({ ...defaultFormData })

watch(() => props.visible, (val) => {
  dialogVisible.value = val
})

watch(dialogVisible, (val) => {
  emit('update:visible', val)
})

watch(() => props.perspective, (val) => {
  if (val) {
    editingPerspective.value = val
    Object.assign(formData, {
      name: val.name,
      icon: val.icon || '📋',
      color: val.color || '#409eff',
      filters: { ...defaultFormData.filters, ...val.filters },
      sortBy: val.sortBy || null,
      sortOrder: val.sortOrder || 'asc',
    })
  } else {
    editingPerspective.value = null
    Object.assign(formData, defaultFormData)
  }
}, { immediate: true })

function handleSave() {
  if (!formData.name.trim()) {
    return
  }
  emit('save', { ...formData })
  dialogVisible.value = false
}
</script>

<style scoped>
.perspective-manager {
  display: contents;
}
</style>
