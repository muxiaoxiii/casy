<script setup>
import { ref, computed, onMounted } from 'vue'
import { useProfileStore } from '../../../stores/profile'
import OnboardingWizard from '../../../shared/components/OnboardingWizard.vue'

// ============================================================
// 律师画像卡片：展示当前画像 + 「重新编辑」打开引导向导
// ============================================================
const profileStore = useProfileStore()
const showWizard = ref(false)

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
        <div class="card-header ui-row">
          <strong>律师画像</strong>
          <el-tag v-if="profileStore.onboardingCompleted" type="success" size="small">已完成</el-tag>
          <el-tag v-else type="info" size="small">未填写</el-tag>
        </div>
      </template>

      <el-form label-width="100px" size="default" class="profile-form">
        <el-form-item label="姓名">
          <span class="profile-value">{{ profileStore.name || '未设置' }}</span>
        </el-form-item>
        <el-form-item label="执业领域">
          <span class="profile-value">{{ practiceAreaText }}</span>
        </el-form-item>
        <el-form-item label="工作时段">
          <span class="profile-value">
            {{ profileStore.work_hours?.start_hour ?? 9 }} 时 至 {{ profileStore.work_hours?.end_hour ?? 18 }} 时
          </span>
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
  gap: 12px;
}

.tip {
  color: var(--gray-400);
  font-size: 13px;
  margin-bottom: 16px;
}

.profile-form {
  max-width: 560px;
}

.profile-value {
  font-size: 13px;
  color: var(--c-text);
}
</style>
