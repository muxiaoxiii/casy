<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'

const props = defineProps<{
  modelValue: boolean
  currentCaseId: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'relation-added'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val)
})

const submitting = ref(false)
const allCases = ref<any[]>([])

const form = ref({
  targetCaseId: '',
  relationType: 'same_patent',
  label: ''
})

const relationOptions = [
  { value: 'same_patent', label: '同专利' },
  { value: 'same_party', label: '同当事人' },
  { value: 'appeal_of', label: '审级关联' },
  { value: 'cross_reference', label: '交叉引用' }
]

// 过滤掉当前案件，只显示其他案件供关联
const selectableCases = computed(() => {
  return allCases.value.filter(c => c.id !== props.currentCaseId)
})

onMounted(async () => {
  const result = await casyContext.cases.list({})
  if (result.ok && result.data && result.data.items) {
    allCases.value = result.data.items
  }
})

function handleClose() {
  form.value = {
    targetCaseId: '',
    relationType: 'same_patent',
    label: ''
  }
  visible.value = false
}

async function handleSubmit() {
  if (submitting.value) return
  if (!form.value.targetCaseId) {
    ElMessage.warning('请选择要关联的案件')
    return
  }

  submitting.value = true
  try {
    const res = await casyContext.cases.addRelation(
      props.currentCaseId,
      form.value.targetCaseId,
      form.value.relationType,
      form.value.label
    )
    if (res.ok) {
      ElMessage.success('关联案件成功')
      emit('relation-added')
      handleClose()
    } else {
      ElMessage.error(res.error || '添加失败')
    }
  } catch (err: any) {
    ElMessage.error(err.message || String(err))
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <el-dialog
    v-model="visible"
    title="添加关联案件"
    width="500px"
    @close="handleClose"
  >
    <el-form label-width="120px" @submit.prevent="handleSubmit">
      <el-form-item label="目标案件" required>
        <el-select
          v-model="form.targetCaseId"
          filterable
          placeholder="搜索或选择案件"
          style="width: 100%"
        >
          <el-option
            v-for="c in selectableCases"
            :key="c.id"
            :label="c.caseName || c.caseNo || '未命名案件'"
            :value="c.id"
          >
            <span style="float: left">{{ c.caseName || '未知案名' }}</span>
            <span style="float: right; color: var(--el-text-color-secondary); font-size: 12px">
              {{ c.caseNo || '' }}
            </span>
          </el-option>
        </el-select>
      </el-form-item>

      <el-form-item label="关联类型" required>
        <el-select v-model="form.relationType" style="width: 100%">
          <el-option
            v-for="opt in relationOptions"
            :key="opt.value"
            :label="opt.label"
            :value="opt.value"
          />
        </el-select>
      </el-form-item>

      <el-form-item label="备注说明 (可选)">
        <el-input v-model="form.label" placeholder="如：一审 -> 二审" />
      </el-form-item>

    </el-form>

    <template #footer>
      <span class="dialog-footer">
        <el-button @click="handleClose">取消</el-button>
        <el-button type="primary" @click="handleSubmit" :loading="submitting">
          确定添加
        </el-button>
      </span>
    </template>
  </el-dialog>
</template>

<style scoped>
.form-tip {
  font-size: 12px;
  line-height: 1.4;
  margin-top: 6px;
  color: var(--el-text-color-secondary);
}
.warning-tip {
  color: var(--el-color-warning);
  background-color: var(--el-color-warning-light-9);
  padding: 8px;
  border-radius: 4px;
}
</style>
