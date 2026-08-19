<script setup>
import { ref, reactive, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useProfileStore } from '../../stores/profile'

// ============================================================
// 首次使用引导 / 律师画像编辑
// 步骤：① 姓名 + 执业领域 ② 常用案件类型 + 工作时段 ③ 提醒通道
// ============================================================
const props = defineProps({
  modelValue: { type: Boolean, default: false },
})
const emit = defineEmits(['update:modelValue', 'saved', 'dismiss'])

const profileStore = useProfileStore()

const visible = computed({
  get: () => props.modelValue,
  set: (v) => emit('update:modelValue', v),
})

const step = ref(0)
const saving = ref(false)

const form = reactive({
  name: '',
  practice_areas: [],
  common_case_types: [],
  start_hour: 9,
  end_hour: 18,
  reminder_channels: [],
})

const practiceOptions = ['专利诉讼', '专利无效', '行政诉讼', '顾问', '其他']

const caseTypeOptions = [
  { value: 'computational', label: '计算型' },
  { value: 'exploratory', label: '探索型' },
  { value: 'growth', label: '成长型' },
]

const channelOptions = [
  { value: 'local', label: '本地通知' },
  { value: 'system', label: '系统通知' },
  { value: 'calendar', label: '日历同步' },
]

// 打开时用当前画像预填
watch(() => props.modelValue, (v) => {
  if (!v) return
  step.value = 0
  form.name = profileStore.name || ''
  form.practice_areas = [...(profileStore.practice_areas || [])]
  form.common_case_types = [...(profileStore.common_case_types || [])]
  form.start_hour = profileStore.work_hours?.start_hour ?? 9
  form.end_hour = profileStore.work_hours?.end_hour ?? 18
  form.reminder_channels = [...(profileStore.reminder_channels || [])]
})

function nextStep() {
  if (step.value === 0 && !form.name.trim()) {
    ElMessage.warning('请填写姓名')
    return
  }
  if (step.value < 2) step.value += 1
}

function prevStep() {
  if (step.value > 0) step.value -= 1
}

function later() {
  visible.value = false
  emit('dismiss')
}

async function finish() {
  saving.value = true
  const profile = {
    name: form.name.trim(),
    practice_areas: [...form.practice_areas],
    common_case_types: [...form.common_case_types],
    work_hours: { start_hour: form.start_hour, end_hour: form.end_hour },
    reminder_channels: [...form.reminder_channels],
    onboarding_completed: true,
  }
  const result = await profileStore.save(profile)
  saving.value = false
  if (result.ok) {
    ElMessage.success('律师画像已保存')
    visible.value = false
    emit('saved')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}
</script>

<template>
  <el-dialog
    v-model="visible"
    width="520px"
    :close-on-click-modal="false"
    :show-close="false"
    class="onboarding-dialog"
  >
    <template #header>
      <div class="wizard-header">
        <h3 class="wizard-title">律师画像</h3>
        <p class="wizard-subtitle">用于个性化提醒与首页问候，可随时在设置中修改</p>
      </div>
    </template>

    <el-steps :active="step" align-center class="wizard-steps">
      <el-step title="基本信息" />
      <el-step title="工作偏好" />
      <el-step title="提醒通道" />
    </el-steps>

    <!-- 步骤 ①：姓名 / 执业领域 -->
    <div v-show="step === 0" class="wizard-body">
      <el-form label-width="90px" size="default">
        <el-form-item label="姓名">
          <el-input v-model="form.name" placeholder="如何称呼你（如：王）" style="width: 220px" />
        </el-form-item>
        <el-form-item label="执业领域">
          <el-checkbox-group v-model="form.practice_areas">
            <el-checkbox v-for="opt in practiceOptions" :key="opt" :value="opt">{{ opt }}</el-checkbox>
          </el-checkbox-group>
        </el-form-item>
      </el-form>
    </div>

    <!-- 步骤 ②：常用案件类型 / 工作时段 -->
    <div v-show="step === 1" class="wizard-body">
      <el-form label-width="110px" size="default">
        <el-form-item label="常用案件类型">
          <el-checkbox-group v-model="form.common_case_types">
            <el-checkbox v-for="opt in caseTypeOptions" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </el-checkbox>
          </el-checkbox-group>
        </el-form-item>
        <el-form-item label="工作时段">
          <el-input-number v-model="form.start_hour" :min="0" :max="23" size="small" />
          <span class="hours-sep">时 至</span>
          <el-input-number v-model="form.end_hour" :min="0" :max="23" size="small" />
          <span class="hours-sep">时</span>
        </el-form-item>
      </el-form>
    </div>

    <!-- 步骤 ③：提醒通道 -->
    <div v-show="step === 2" class="wizard-body">
      <el-form label-width="90px" size="default">
        <el-form-item label="提醒通道">
          <el-checkbox-group v-model="form.reminder_channels">
            <el-checkbox v-for="opt in channelOptions" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </el-checkbox>
          </el-checkbox-group>
        </el-form-item>
      </el-form>
      <p class="wizard-hint">日历同步需先在「设置 → SMTP / MCP」中配置 CalDAV。</p>
    </div>

    <template #footer>
      <div class="wizard-footer">
        <el-button text class="later-btn" @click="later">稍后再填</el-button>
        <div class="wizard-actions">
          <el-button v-if="step > 0" @click="prevStep">上一步</el-button>
          <el-button v-if="step < 2" type="primary" @click="nextStep">下一步</el-button>
          <el-button v-else type="primary" :loading="saving" @click="finish">完成</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.wizard-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #1F2430;
}

.wizard-subtitle {
  margin: 4px 0 0;
  font-size: 12px;
  color: #909399;
}

.wizard-steps {
  margin-bottom: 20px;
}

.wizard-body {
  min-height: 140px;
}

.hours-sep {
  margin: 0 8px;
  font-size: 13px;
  color: #606266;
}

.wizard-hint {
  margin: 8px 0 0;
  font-size: 12px;
  color: #909399;
}

.wizard-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.wizard-actions {
  display: flex;
  gap: 8px;
}
</style>
