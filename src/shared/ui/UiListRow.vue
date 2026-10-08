<script setup lang="ts">
/** 可点击列表行原语：统一 hover/focus/active、焦点环与键盘可达（替代 8 文件重复的“可点条目”签名） */
withDefaults(defineProps<{
  active?: boolean
  disabled?: boolean
}>(), { active: false, disabled: false })

const emit = defineEmits<{ click: [MouseEvent | KeyboardEvent] }>()
</script>
<template>
  <div
    class="ui-list-row"
    :class="{ 'is-active': active }"
    :aria-disabled="disabled || undefined"
    role="button"
    tabindex="0"
    @click="!disabled && emit('click', $event)"
    @keydown.enter.prevent="!disabled && emit('click', $event)"
    @keydown.space.prevent="!disabled && emit('click', $event)"
  >
    <slot />
  </div>
</template>
<style scoped>
.ui-list-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--c-radius-md, 4px);
  cursor: pointer;
  color: var(--c-text);
  font-size: 13px;
  background: transparent;
  border: 1px solid transparent;
  min-width: 0;
}
.ui-list-row:hover { background: var(--c-bg-hover, rgba(0, 0, 0, 0.04)); }
.ui-list-row:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 1px; }
.ui-list-row:active { background: var(--c-bg-active, rgba(0, 0, 0, 0.08)); }
.ui-list-row.is-active { background: var(--c-primary-light); border-color: var(--c-primary); }
.ui-list-row[aria-disabled='true'] { cursor: not-allowed; opacity: 0.6; }
</style>
