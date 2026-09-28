<script setup>
import { ref, reactive, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useProfileStore } from '../../stores/profile'

const props = defineProps({ modelValue: { type: Boolean, default: false } })
const emit = defineEmits(['update:modelValue', 'saved', 'dismiss'])
const profileStore = useProfileStore()
const visible = computed({ get: () => props.modelValue, set: v => emit('update:modelValue', v) })
const saving = ref(false)
const form = reactive({ name: '', practice_areas: [], start_hour: 9, end_hour: 18 })
const baseline = ref('')
const dirty = computed(() => visible.value && JSON.stringify(form) !== baseline.value)
const practiceOptions = ['专利诉讼', '专利无效', '行政诉讼', '顾问', '其他']
watch(() => props.modelValue, v => {
  if (!v) return
  form.name = profileStore.name || ''
  form.practice_areas = [...(profileStore.practice_areas || [])]
  form.start_hour = profileStore.work_hours?.start_hour ?? 9
  form.end_hour = profileStore.work_hours?.end_hour ?? 18
  baseline.value = JSON.stringify(form)
}, { immediate: true })
async function later() {
  if (saving.value) return
  if (dirty.value) {
    try { await ElMessageBox.confirm('个人设置有未保存的修改，确认放弃？', '保留修改？', { confirmButtonText: '放弃修改', cancelButtonText: '继续编辑', type: 'warning' }) } catch { return }
  }
  visible.value = false
  emit('dismiss')
}
async function finish() {
  if (saving.value) return
  if (form.start_hour >= form.end_hour) return ElMessage.warning('结束时间须晚于开始时间')
  saving.value = true
  try {
    const result = await profileStore.save({
      name: form.name.trim(),
      practice_areas: [...form.practice_areas],
      common_case_types: [...(profileStore.common_case_types || [])],
      reminder_channels: [...(profileStore.reminder_channels || [])],
      work_hours: { start_hour: form.start_hour, end_hour: form.end_hour },
      onboarding_completed: true,
    })
    if (!result.ok) return ElMessage.error(result.error || '保存失败')
    visible.value = false
    emit('saved')
  } catch (cause) { ElMessage.error(String(cause)) }
  finally { saving.value = false }
}
function beforeUnload(event) { if (dirty.value || saving.value) { event.preventDefault(); event.returnValue = '' } }
onMounted(() => window.addEventListener('beforeunload', beforeUnload))
onBeforeUnmount(() => window.removeEventListener('beforeunload', beforeUnload))
</script>

<template>
  <el-dialog v-model="visible" title="个人设置" width="min(480px, calc(100vw - 32px))" :close-on-click-modal="false" :show-close="false" :close-on-press-escape="false" class="onboarding-dialog">
    <el-form :disabled="saving" label-position="top" @submit.prevent="finish">
      <el-form-item label="称呼（选填）"><el-input v-model="form.name" autocomplete="name" maxlength="60" /></el-form-item>
      <el-form-item label="执业领域（选填）">
        <el-checkbox-group v-model="form.practice_areas"><el-checkbox v-for="area in practiceOptions" :key="area" :value="area">{{ area }}</el-checkbox></el-checkbox-group>
      </el-form-item>
      <el-form-item label="工作时段">
        <div class="hours"><el-input-number v-model="form.start_hour" :min="0" :max="23" aria-label="工作开始时间" /><span>至</span><el-input-number v-model="form.end_hour" :min="1" :max="23" aria-label="工作结束时间" /></div>
      </el-form-item>
    </el-form>
    <template #footer><el-button :disabled="saving" @click="later">暂时跳过</el-button><el-button type="primary" :loading="saving" @click="finish">保存</el-button></template>
  </el-dialog>
</template>

<style scoped>
.hours{display:flex;align-items:center;gap:12px;flex-wrap:wrap}
.hours :deep(.el-input-number){width:140px}
</style>
