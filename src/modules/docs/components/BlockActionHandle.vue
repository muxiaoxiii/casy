<script setup lang="ts">
import { ref } from 'vue'
import { Plus, Delete, CopyDocument, Switch, Edit, List, Finished, Opportunity } from '../../../shared/icons'

const props = defineProps<{
  editor: any
  top: number
  visible: boolean
}>()

const emit = defineEmits<{
  (e: 'open-slash'): void
}>()

const menuVisible = ref(false)

function onPlusClick() {
  props.editor.chain().focus().createParagraphNear().run()
  emit('open-slash')
}

function turnInto(type: string, level?: number) {
  if (type === 'heading') {
    props.editor.chain().focus().toggleHeading({ level: level as any }).run()
  } else if (type === 'paragraph') {
    props.editor.chain().focus().setParagraph().run()
  } else if (type === 'bulletList') {
    props.editor.chain().focus().toggleBulletList().run()
  } else if (type === 'taskList') {
    props.editor.chain().focus().toggleTaskList().run()
  } else if (type === 'callout') {
    props.editor.chain().focus().toggleBlockquote().run()
  }
  menuVisible.value = false
}

function duplicateBlock() {
  const { state } = props.editor
  const { $from } = state.selection
  const currentBlock = $from.node($from.depth)
  if (currentBlock) {
    props.editor.chain().focus().insertContentAt($from.after(), currentBlock.toJSON()).run()
  }
  menuVisible.value = false
}

function deleteBlock() {
  props.editor.chain().focus().deleteNode('paragraph').run()
  menuVisible.value = false
}
</script>

<template>
  <div
    v-show="visible"
    class="notion-block-handle"
    :style="{ top: top + 'px' }"
    @click.stop
  >
    <!-- ➕ 添加块按钮 -->
    <button class="handle-btn plus-btn" title="在下方新增块 (快捷唤出 /)" @click="onPlusClick">
      <el-icon :size="13"><Plus /></el-icon>
    </button>

    <!-- ⠿ 块操作手柄 -->
    <el-popover
      v-model:visible="menuVisible"
      placement="bottom-start"
      :width="200"
      trigger="click"
      popper-class="notion-handle-popover"
    >
      <template #reference>
        <button class="handle-btn drag-btn" title="点击查看块操作与转换">
          <span class="dots-grid">
            <span class="dot" />
            <span class="dot" />
            <span class="dot" />
            <span class="dot" />
            <span class="dot" />
            <span class="dot" />
          </span>
        </button>
      </template>

      <div class="block-action-pop-content">
        <div class="pop-section-label">转换块类型 (Turn Into)</div>
        <button class="pop-action-item" @click="turnInto('paragraph')">
          <el-icon><Edit /></el-icon> 转换为 正文
        </button>
        <button class="pop-action-item" @click="turnInto('heading', 1)">
          <el-icon><Edit /></el-icon> 转换为 一级标题
        </button>
        <button class="pop-action-item" @click="turnInto('heading', 2)">
          <el-icon><Edit /></el-icon> 转换为 二级标题
        </button>
        <button class="pop-action-item" @click="turnInto('bulletList')">
          <el-icon><List /></el-icon> 转换为 无序列表
        </button>
        <button class="pop-action-item" @click="turnInto('taskList')">
          <el-icon><Finished /></el-icon> 转换为 待办清单
        </button>
        <button class="pop-action-item" @click="turnInto('callout')">
          <el-icon><Opportunity /></el-icon> 转换为 提示/引用框
        </button>

        <div class="pop-divider" />

        <button class="pop-action-item" @click="duplicateBlock">
          <el-icon><CopyDocument /></el-icon> 复制该块
        </button>
        <button class="pop-action-item text-danger" @click="deleteBlock">
          <el-icon><Delete /></el-icon> 删除该块
        </button>
      </div>
    </el-popover>
  </div>
</template>

<style scoped>
.notion-block-handle {
  position: absolute;
  left: 12px;
  display: flex;
  align-items: center;
  gap: 2px;
  transform: translateY(-2px);
  z-index: 20;
  user-select: none;
  opacity: 0.85;
  transition: opacity var(--motion-fast);
}

.notion-block-handle:hover {
  opacity: 1;
}

.handle-btn {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: var(--slate-gray-light);
  display: grid;
  place-items: center;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.handle-btn:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.dots-grid {
  display: grid;
  grid-template-columns: repeat(2, 3px);
  grid-template-rows: repeat(3, 3px);
  gap: 2px;
}

.dot {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: currentColor;
}

.block-action-pop-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px 0;
}

.pop-section-label {
  font-size: 10px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 4px 8px 2px;
}

.pop-action-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  border: none;
  background: transparent;
  font-size: 12.5px;
  color: var(--c-text-regular);
  cursor: pointer;
  transition: all var(--motion-fast);
  text-align: left;
  width: 100%;
}

.pop-action-item:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.pop-action-item.text-danger {
  color: var(--status-risk);
}

.pop-action-item.text-danger:hover {
  background: var(--bg-risk-weak);
}

.pop-divider {
  height: 1px;
  background: var(--c-border-light);
  margin: 4px 0;
}
</style>
