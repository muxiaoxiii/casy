<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { CircleCheck, CircleClose, Warning } from '@element-plus/icons-vue'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  recommendation: {
    type: Object,
    default: () => null,
  },
  confirmLevel: {
    type: String,
    default: 'L1',
    validator: (v) => ['L1', 'L2', 'L3'].includes(v),
  },
})

const emit = defineEmits(['update:visible', 'confirm', 'reject'])

const confirmText = ref('')
const itemResults = ref({})

// 对话框可见性
const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit('update:visible', val),
})

// 确认等级说明
const levelInfo = computed(() => {
  const info = {
    L1: {
      label: 'L1 可读确认',
      description: '快速确认，点击即通过',
      color: '#67C23A',
      icon: CircleCheck,
    },
    L2: {
      label: 'L2 逐项确认',
      description: '需要逐项审核并确认',
      color: '#E6A23C',
      icon: Warning,
    },
    L3: {
      label: 'L3 双人复核',
      description: '需要输入确认文字',
      color: '#F56C6C',
      icon: CircleClose,
    },
  }
  return info[props.confirmLevel] || info.L1
})

// 推荐项目列表（L2 用）
const recommendItems = computed(() => {
  if (!props.recommendation?.items) return []
  return props.recommendation.items
})

// L2 是否全部已确认
const allItemsHandled = computed(() => {
  if (props.confirmLevel !== 'L2') return true
  return recommendItems.value.length > 0 &&
    recommendItems.value.every((_, idx) => itemResults.value[idx] !== undefined)
})

// L3 确认文字校验
const confirmTextValid = computed(() => {
  if (props.confirmLevel !== 'L3') return true
  return confirmText.value.trim() === '确认'
})

// 是否可以提交
const canSubmit = computed(() => {
  if (props.confirmLevel === 'L1') return true
  if (props.confirmLevel === 'L2') return allItemsHandled.value
  if (props.confirmLevel === 'L3') return confirmTextValid.value
  return false
})

// 初始化 L2 项目状态
watch(() => props.visible, (val) => {
  if (val) {
    confirmText.value = ''
    itemResults.value = {}
  }
})

function acceptItem(idx) {
  itemResults.value[idx] = 'accepted'
}

function rejectItem(idx) {
  itemResults.value[idx] = 'rejected'
}

function handleConfirm() {
  if (!canSubmit.value) return

  const result = {
    level: props.confirmLevel,
    accepted: true,
  }

  if (props.confirmLevel === 'L2') {
    result.itemResults = { ...itemResults.value }
  }

  emit('confirm', result)
  dialogVisible.value = false
  ElMessage.success('已确认')
}

function handleReject() {
  emit('reject', { level: props.confirmLevel })
  dialogVisible.value = false
  ElMessage.info('已拒绝')
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    title="AI 推荐确认"
    width="520"
    :close-on-click-modal="false"
  >
    <div v-if="recommendation" class="confirm-content">
      <!-- 确认等级提示 -->
      <div class="level-banner" :style="{ borderLeftColor: levelInfo.color }">
        <el-icon :size="16" :color="levelInfo.color">
          <component :is="levelInfo.icon" />
        </el-icon>
        <div>
          <span class="level-label">{{ levelInfo.label }}</span>
          <span class="level-desc">{{ levelInfo.description }}</span>
        </div>
      </div>

      <!-- 推荐内容 -->
      <div class="recommend-section">
        <h4>推荐内容</h4>
        <p class="recommend-text">{{ recommendation.content }}</p>
      </div>

      <!-- 推荐理由 -->
      <div v-if="recommendation.reason" class="reason-section">
        <h4>推荐理由</h4>
        <p class="reason-text">{{ recommendation.reason }}</p>
      </div>

      <!-- Effective Policy 说明 -->
      <div v-if="recommendation.effectivePolicy" class="policy-section">
        <el-alert
          type="info"
          :closable="false"
          show-icon
        >
          <template #title>
            <span>Effective Policy: {{ recommendation.effectivePolicy }}</span>
          </template>
          <template #default>
            <span class="policy-desc">
              确认等级由系统安全下限、场景风险、模型质量和用户设置共同决定
            </span>
          </template>
        </el-alert>
      </div>

      <!-- L2: 逐项确认 -->
      <div v-if="confirmLevel === 'L2' && recommendItems.length > 0" class="items-section">
        <h4>逐项确认</h4>
        <div class="item-list">
          <div
            v-for="(item, idx) in recommendItems"
            :key="idx"
            class="confirm-item"
            :class="{
              accepted: itemResults[idx] === 'accepted',
              rejected: itemResults[idx] === 'rejected',
            }"
          >
            <div class="item-content">
              <span class="item-index">{{ idx + 1 }}.</span>
              <span>{{ item }}</span>
            </div>
            <div class="item-actions">
              <el-button
                size="small"
                :type="itemResults[idx] === 'accepted' ? 'success' : 'default'"
                @click="acceptItem(idx)"
              >
                接受
              </el-button>
              <el-button
                size="small"
                :type="itemResults[idx] === 'rejected' ? 'danger' : 'default'"
                @click="rejectItem(idx)"
              >
                拒绝
              </el-button>
            </div>
          </div>
        </div>
      </div>

      <!-- L3: 输入确认文字 -->
      <div v-if="confirmLevel === 'L3'" class="text-confirm-section">
        <h4>请输入"确认"以继续</h4>
        <el-input
          v-model="confirmText"
          placeholder="输入 确认"
          size="default"
        />
        <p class="confirm-hint">此操作需要双人复核，请谨慎确认</p>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-button @click="handleReject">拒绝</el-button>
        <el-button type="primary" :disabled="!canSubmit" @click="handleConfirm">
          确认
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.confirm-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.level-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  background: #F5F7FA;
  border-left: 3px solid;
  border-radius: 4px;
}

.level-label {
  font-weight: 600;
  font-size: 13px;
  color: #303133;
  margin-right: 8px;
}

.level-desc {
  font-size: 12px;
  color: #909399;
}

.recommend-section h4,
.reason-section h4,
.items-section h4,
.text-confirm-section h4 {
  margin: 0 0 8px 0;
  font-size: 13px;
  font-weight: 600;
  color: #303133;
}

.recommend-text {
  margin: 0;
  font-size: 14px;
  color: #303133;
  line-height: 1.6;
  padding: 10px 12px;
  background: #F0F9FF;
  border-radius: 4px;
}

.reason-text {
  margin: 0;
  font-size: 13px;
  color: #606266;
  line-height: 1.6;
}

.policy-section :deep(.el-alert) {
  padding: 8px 12px;
}

.policy-desc {
  font-size: 12px;
  color: #909399;
}

.item-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.confirm-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  background: #FAFAFA;
  border: 1px solid #E4E7ED;
  border-radius: 6px;
  transition: all 0.2s;
}

.confirm-item.accepted {
  background: #F0F9FF;
  border-color: #67C23A;
}

.confirm-item.rejected {
  background: #FEF0F0;
  border-color: #F56C6C;
}

.item-content {
  display: flex;
  gap: 8px;
  font-size: 13px;
  color: #303133;
  flex: 1;
}

.item-index {
  color: #909399;
  flex-shrink: 0;
}

.item-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.text-confirm-section .el-input {
  margin-bottom: 8px;
}

.confirm-hint {
  margin: 0;
  font-size: 12px;
  color: #E6A23C;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
