<script setup>
import { computed } from 'vue'

const props = defineProps({
  /** 显示行数 */
  rows: { type: Number, default: 3 },
  /** 是否显示头像占位 */
  avatar: { type: Boolean, default: false },
  /** 是否显示标题占位 */
  title: { type: Boolean, default: true },
  /** 最后一行宽度百分比 */
  lastLineWidth: { type: Number, default: 60 },
})

const contentRows = computed(() => {
  const arr = []
  for (let i = 0; i < props.rows; i++) {
    const isLast = i === props.rows - 1
    arr.push({
      width: isLast ? `${props.lastLineWidth}%` : '100%',
    })
  }
  return arr
})
</script>

<template>
  <div class="skeleton-card">
    <div v-if="avatar || title" class="skeleton-header ui-row">
      <div v-if="avatar" class="skeleton-avatar skeleton-bone"></div>
      <div v-if="title" class="skeleton-title skeleton-bone"></div>
    </div>
    <div class="skeleton-body ui-col">
      <div
        v-for="(row, index) in contentRows"
        :key="index"
        class="skeleton-line skeleton-bone"
        :style="{ width: row.width }"
      ></div>
    </div>
  </div>
</template>

<style scoped>
.skeleton-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  padding: 16px;
}

.skeleton-header {
  /* 布局走共享 .ui-row；此处只保留本文件特有的 12px 间距与 14px 下间距 */
  gap: 12px;
  margin-bottom: 14px;
}

.skeleton-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  flex-shrink: 0;
}

.skeleton-title {
  height: 16px;
  width: 45%;
}

.skeleton-body {
  /* 布局走共享 .ui-col；此处只保留本文件特有的 10px 间距 */
  gap: 10px;
}

.skeleton-line {
  height: 12px;
}

/* 骨架屏闪烁动画 */
.skeleton-bone {
  background: linear-gradient(
    90deg,
    var(--c-bg-page) 25%,
    var(--c-border) 37%,
    var(--c-bg-page) 63%
  );
  background-size: 400% 100%;
  animation: skeleton-pulse 1.4s ease infinite;
  border-radius: 4px;
}

@keyframes skeleton-pulse {
  0% {
    background-position: 100% 50%;
  }
  100% {
    background-position: 0 50%;
  }
}
@media (prefers-reduced-motion: reduce) { .skeleton-bone { animation: none; } }
</style>
