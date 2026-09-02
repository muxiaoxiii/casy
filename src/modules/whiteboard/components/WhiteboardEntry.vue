<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Right, Grid } from '@element-plus/icons-vue'
import { tauriCall } from '../../../core/tauriBridge'
import type { WhiteboardDto } from '../types'

/**
 * WhiteboardEntry —— 事实白板入口小组件
 *
 * 契约：
 *   props: caseId: string（案件 ID，必传）
 *   无 emits；暴露 reload() 供父组件在需要时刷新白板列表
 * 用途：挂载在案件详情页，展示白板摘要（名称 + 节点数）、跳转白板视图、快速新建
 */
const props = defineProps<{ caseId: string }>()

const router = useRouter()
const whiteboards = ref<WhiteboardDto[]>([])
const loading = ref(false)

async function reload() {
  if (!props.caseId) return
  loading.value = true
  const list = await tauriCall<WhiteboardDto[]>('list_whiteboards', { caseId: props.caseId })
  loading.value = false
  whiteboards.value = Array.isArray(list) ? list : []
}

defineExpose({ reload })

function openBoard(board?: WhiteboardDto) {
  router.push({
    path: `/whiteboard/${props.caseId}`,
    query: board ? { board: board.id } : {},
  })
}

async function quickCreate() {
  let name = ''
  try {
    const res = await ElMessageBox.prompt('请输入白板名称', '新建白板', {
      confirmButtonText: '创建',
      cancelButtonText: '取消',
      inputPlaceholder: '例如：争议焦点梳理',
      inputValidator: (v: string) => (v && v.trim() ? true : '名称不能为空'),
    })
    name = res.value.trim()
  } catch {
    return
  }
  const id = await tauriCall<string>('create_whiteboard', { caseId: props.caseId, name })
  if (!id) return
  ElMessage.success('白板已创建')
  await reload()
  openBoard(whiteboards.value.find((b) => b.id === id))
}

onMounted(reload)
watch(() => props.caseId, reload)
</script>

<template>
  <section v-loading="loading" class="whiteboard-entry">
    <header class="whiteboard-entry__header">
      <span class="whiteboard-entry__title">
        <el-icon><Grid /></el-icon>事实白板
      </span>
      <el-button size="small" text :icon="Plus" @click="quickCreate">新建</el-button>
    </header>

    <div v-if="whiteboards.length === 0 && !loading" class="whiteboard-entry__empty">
      <p>还没有事实白板</p>
      <el-button size="small" type="primary" plain :icon="Plus" @click="quickCreate">
        快速新建
      </el-button>
    </div>

    <ul v-else class="whiteboard-entry__list">
      <li v-for="board in whiteboards" :key="board.id">
        <button class="whiteboard-entry__row" type="button" @click="openBoard(board)">
          <span class="whiteboard-entry__name">{{ board.name }}</span>
          <span class="whiteboard-entry__count">{{ board.nodeCount }} 节点</span>
          <el-icon class="whiteboard-entry__arrow"><Right /></el-icon>
        </button>
      </li>
    </ul>

    <footer v-if="whiteboards.length" class="whiteboard-entry__footer">
      <el-button size="small" text @click="openBoard()">
        打开白板<el-icon><Right /></el-icon>
      </el-button>
    </footer>
  </section>
</template>

<style scoped>
.whiteboard-entry {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-card);
  padding: var(--space-3) var(--space-4);
}

.whiteboard-entry__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-2);
}

.whiteboard-entry__title {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--c-text-heading);
}

.whiteboard-entry__empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  color: var(--c-text-secondary);
  font-size: var(--text-base);
  padding: var(--space-2) 0;
}

.whiteboard-entry__empty p {
  margin: 0;
}

.whiteboard-entry__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.whiteboard-entry__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: var(--c-radius);
  background: none;
  font-family: inherit;
  font-size: var(--text-base);
  color: var(--c-text-regular);
  cursor: pointer;
  text-align: left;
  transition: background var(--motion-fast) var(--ease-out);
}

.whiteboard-entry__row:hover {
  background: var(--c-bg-hover);
}

.whiteboard-entry__name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.whiteboard-entry__count {
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: var(--c-text-placeholder);
}

.whiteboard-entry__arrow {
  flex-shrink: 0;
  color: var(--c-text-placeholder);
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-out);
}

.whiteboard-entry__row:hover .whiteboard-entry__arrow {
  opacity: 1;
}

.whiteboard-entry__footer {
  display: flex;
  justify-content: flex-end;
  margin-top: var(--space-1);
}

@media (prefers-reduced-motion: reduce) {
  .whiteboard-entry__row,
  .whiteboard-entry__arrow {
    transition: none;
  }
}
</style>
