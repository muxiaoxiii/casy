<script setup>
import { computed } from 'vue'
import {
  Briefcase,
  Finished,
  Document,
  Collection,
  Calendar,
  Box,
  Folder,
  Search,
} from '@element-plus/icons-vue'

const props = defineProps({
  /** 场景类型：cases / tasks / docs / knowledge / calendar / inbox / files / search / custom */
  type: { type: String, default: 'custom' },
  /** 自定义图标（仅 custom 类型生效） */
  icon: { type: [Object, String], default: null },
  /** 自定义标题 */
  title: { type: String, default: '' },
  /** 自定义描述 */
  description: { type: String, default: '' },
  /** 自定义按钮文案 */
  actionText: { type: String, default: '' },
  /** 是否紧凑模式 */
  compact: { type: Boolean, default: false },
})

const emit = defineEmits(['action'])

// 预设场景配置
const presets = {
  cases: {
    icon: Briefcase,
    title: '还没有案件',
    description: '创建你的第一个案件，开始管理专利事务',
    actionText: '创建案件',
  },
  tasks: {
    icon: Finished,
    title: '任务列表为空',
    description: '添加待办事项，让工作有条不紊',
    actionText: '添加任务',
  },
  docs: {
    icon: Document,
    title: '暂无文书',
    description: '使用 AI 或模板生成你的第一份文书',
    actionText: '生成文书',
  },
  knowledge: {
    icon: Collection,
    title: '知识库为空',
    description: '积累经验和参考，让 AI 越来越懂你',
    actionText: '添加知识',
  },
  calendar: {
    icon: Calendar,
    title: '日历空空如也',
    description: '添加日程和截止日期，不错过任何节点',
    actionText: '添加日程',
  },
  inbox: {
    icon: Box,
    title: '收件箱是空的',
    description: '快速捕获的想法和待处理事项会出现在这里',
    actionText: '捕获想法',
  },
  files: {
    icon: Folder,
    title: '暂无文件',
    description: '上传相关文件，集中管理案件资料',
    actionText: '上传文件',
  },
  search: {
    icon: Search,
    title: '没有找到结果',
    description: '尝试调整搜索关键词或筛选条件',
    actionText: '',
  },
}

const config = computed(() => {
  if (props.type === 'custom') {
    return {
      icon: props.icon || Box,
      title: props.title || '暂无数据',
      description: props.description || '',
      actionText: props.actionText || '',
    }
  }
  return presets[props.type] || presets.cases
})

const displayIcon = computed(() => config.value.icon)
const displayTitle = computed(() => props.title || config.value.title)
const displayDesc = computed(() => props.description || config.value.description)
const displayAction = computed(() => props.actionText || config.value.actionText)

function handleAction() {
  emit('action')
}
</script>

<template>
  <div class="empty-state" :class="{ 'empty-state--compact': compact }">
    <div class="empty-icon">
      <el-icon :size="compact ? 32 : 48">
        <component :is="displayIcon" />
      </el-icon>
    </div>

    <div class="empty-content">
      <h3 class="empty-title">{{ displayTitle }}</h3>
      <p v-if="displayDesc" class="empty-desc">{{ displayDesc }}</p>
    </div>

    <el-button
      v-if="displayAction"
      type="primary"
      :size="compact ? 'small' : 'default'"
      @click="handleAction"
    >
      {{ displayAction }}
    </el-button>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  text-align: center;
}

.empty-state--compact {
  padding: 24px 16px;
}

.empty-icon {
  color: #D1D5DB;
  margin-bottom: 16px;
}

.empty-state--compact .empty-icon {
  margin-bottom: 10px;
}

.empty-content {
  margin-bottom: 20px;
}

.empty-state--compact .empty-content {
  margin-bottom: 12px;
}

.empty-title {
  font-size: 15px;
  font-weight: 600;
  color: #6B7280;
  margin: 0 0 6px 0;
  line-height: 1.4;
}

.empty-state--compact .empty-title {
  font-size: 13px;
}

.empty-desc {
  font-size: 13px;
  color: #9CA3AF;
  margin: 0;
  line-height: 1.5;
  max-width: 280px;
}

.empty-state--compact .empty-desc {
  font-size: 12px;
  max-width: 240px;
}
</style>
