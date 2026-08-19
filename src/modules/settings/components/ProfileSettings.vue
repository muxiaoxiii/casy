<script setup>
import { ref, computed, onMounted } from 'vue'
import { useProfileStore } from '../../../stores/profile'
import OnboardingWizard from '../../../shared/components/OnboardingWizard.vue'

// ============================================================
// 律师画像卡片：展示当前画像 + 「重新编辑」打开引导向导
// ============================================================
const profileStore = useProfileStore()
const showWizard = ref(false)

const CASE_TYPE_LABELS = {
  computational: '计算型',
  exploratory: '探索型',
  growth: '成长型',
}

const CHANNEL_LABELS = {
  local: '本地通知',
  system: '系统通知',
  calendar: '日历同步',
}

const caseTypeText = computed(() => {
  const types = profileStore.common_case_types || []
  return types.length ? types.map(t => CASE_TYPE_LABELS[t] || t).join('、') : '未设置'
})

const channelText = computed(() => {
  const channels = profileStore.reminder_channels || []
  return channels.length ? channels.map(c => CHANNEL_LABELS[c] || c).join('、') : '未设置'
})

const practiceAreaText = computed(() => {
  const areas = profileStore.practice_areas || []
  return areas.length ? areas.join('、') : '未设置'
})

onMounted(() => {
  if (!profileStore.loaded) profileStore.load()
})
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>律师画像</strong>
          <el-tag v-if="profileStore.onboardingCompleted" type="success" size="small">已完成</el-tag>
          <el-tag v-else type="info" size="small">未填写</el-tag>
        </div>
      </template>

      <p class="tip">画像用于首页问候、提醒时段与通道偏好，不会影响案件数据。</p>

      <el-form label-width="110px" size="default" class="profile-form">
        <el-form-item label="姓名">
          <span class="profile-value">{{ profileStore.name || '未设置' }}</span>
        </el-form-item>
        <el-form-item label="执业领域">
          <span class="profile-value">{{ practiceAreaText }}</span>
        </el-form-item>
        <el-form-item label="常用案件类型">
          <span class="profile-value">{{ caseTypeText }}</span>
        </el-form-item>
        <el-form-item label="工作时段">
          <span class="profile-value">
            {{ profileStore.work_hours?.start_hour ?? 9 }} 时 至 {{ profileStore.work_hours?.end_hour ?? 18 }} 时
          </span>
        </el-form-item>
        <el-form-item label="提醒通道">
          <span class="profile-value">{{ channelText }}</span>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="showWizard = true">
            {{ profileStore.onboardingCompleted ? '重新编辑' : '立即填写' }}
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <OnboardingWizard v-model="showWizard" />
  </div>
</template>

<style scoped>
.tab-content {
  padding: 0 16px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.tip {
  color: #909399;
  font-size: 13px;
  margin-bottom: 16px;
}

.profile-form {
  max-width: 560px;
}

.profile-value {
  font-size: 13px;
  color: #1F2430;
}
</style>
