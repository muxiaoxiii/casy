<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowLeft, Plus, Delete, Document, Link, Collection, Refresh, Edit
} from '@element-plus/icons-vue'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'
import { useCasesStore } from '../../../stores/cases'
import EmptyState from '../../../shared/components/EmptyState.vue'
import type { WhiteboardDto, FactNodeDto, WhiteboardEdgeDto, CaseFileLite } from '../types'

// ============================================================
// 基础
// ============================================================
const route = useRoute()
const router = useRouter()
const casesStore = useCasesStore()

const caseId = computed(() => String(route.params.caseId || ''))
const caseName = ref('')

// ============================================================
// 画布常量（逻辑像素）
// ============================================================
const CANVAS_W = 2400
const CANVAS_H = 1600
const CARD_W = 300
const CARD_MIN_H = 96

// ============================================================
// 白板列表 / 当前白板
// ============================================================
const whiteboards = ref<WhiteboardDto[]>([])
const boardsLoading = ref(false)
const currentBoardId = ref<string>('')

const currentBoard = computed<WhiteboardDto | null>(
  () => whiteboards.value.find((b) => b.id === currentBoardId.value) || null
)

// ============================================================
// 节点
// ============================================================
const nodes = ref<FactNodeDto[]>([])
const nodesLoading = ref(false)
const selectedNodeId = ref<string | null>(null)
const draggingId = ref<string | null>(null)
const expandedIds = ref<Set<string>>(new Set())

// ============================================================
// 连线 (Edges)
// ============================================================
const edges = ref<WhiteboardEdgeDto[]>([])
const edgesLoading = ref(false)

// 拖拽连线临时状态
const edgeDraft = ref<{
  sourceId: string
  startX: number
  startY: number
  endX: number
  endY: number
} | null>(null)

function getEdgePath(sourceNode: FactNodeDto, targetNode: FactNodeDto) {
  const sx = sourceNode.x + 300
  const sy = sourceNode.y + 40
  const tx = targetNode.x
  const ty = targetNode.y + 40
  const dx = Math.abs(tx - sx) * 0.5
  return `M ${sx} ${sy} C ${sx + dx} ${sy}, ${tx - dx} ${ty}, ${tx} ${ty}`
}

function getDraftPath() {
  if (!edgeDraft.value) return ''
  const sx = edgeDraft.value.startX
  const sy = edgeDraft.value.startY
  const tx = edgeDraft.value.endX
  const ty = edgeDraft.value.endY
  const dx = Math.abs(tx - sx) * 0.5
  return `M ${sx} ${sy} C ${sx + dx} ${sy}, ${tx - dx} ${ty}, ${tx} ${ty}`
}

const viewportRef = ref<HTMLElement | null>(null)

const nodeCountLabel = computed(() => {
  const board = currentBoard.value
  return board ? `${nodes.value.length} 个节点` : ''
})

// ============================================================
// 数据加载
// ============================================================
async function loadCase() {
  if (!caseId.value) return
  const result = await casesStore.loadCase(caseId.value)
  if (result.ok && result.data) caseName.value = result.data.caseName || ''
}

async function loadWhiteboards(preferId?: string) {
  if (!caseId.value) return
  boardsLoading.value = true
  const list = await tauriCall('list_whiteboards', { caseId: caseId.value })
  boardsLoading.value = false
  whiteboards.value = Array.isArray(list) ? list : []

  const fromQuery = typeof route.query.board === 'string' ? route.query.board : ''
  const wanted = preferId || fromQuery
  const hit = whiteboards.value.find((b) => b.id === wanted)
  currentBoardId.value = hit ? hit.id : whiteboards.value[0]?.id || ''
  syncBoardQuery()
}

let nodesLoadSeq = 0

async function loadNodes() {
  if (!currentBoardId.value) {
    nodes.value = []
    return
  }
  // 竞态防护：快速切换白板时旧响应不得覆盖新数据
  const reqId = ++nodesLoadSeq
  const boardId = currentBoardId.value
  nodesLoading.value = true
  const list = await tauriCall('list_fact_nodes', { whiteboardId: boardId })
  if (reqId !== nodesLoadSeq) return
  nodesLoading.value = false
  nodes.value = Array.isArray(list) ? list : []
  selectedNodeId.value = null
  expandedIds.value = new Set()

  // 同时加载连线
  edgesLoading.value = true
  const edgeList = await tauriCall('list_whiteboard_edges', { whiteboardId: boardId })
  if (reqId !== nodesLoadSeq) return
  edgesLoading.value = false
  edges.value = Array.isArray(edgeList) ? edgeList : []
}

function syncBoardQuery() {
  if (!currentBoardId.value) return
  if (route.query.board === currentBoardId.value) return
  router.replace({ query: { ...route.query, board: currentBoardId.value } })
}

function onBoardChange(id: string) {
  currentBoardId.value = id
  syncBoardQuery()
}

watch(currentBoardId, () => {
  void loadNodes()
})

// ============================================================
// 白板增删
// ============================================================
async function createBoard() {
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
  const id = await tauriCall('create_whiteboard', { caseId: caseId.value, name })
  if (!id) return
  ElMessage.success('白板已创建')
  await loadWhiteboards(id)
}

async function renameBoard() {
  const board = currentBoard.value
  if (!board) return
  let name = ''
  try {
    const res = await ElMessageBox.prompt('请输入新的白板名称', '重命名白板', {
      confirmButtonText: '重命名',
      cancelButtonText: '取消',
      inputValue: board.name,
      inputValidator: (v: string) => (v && v.trim() ? true : '名称不能为空'),
    })
    name = res.value.trim()
  } catch {
    return
  }
  if (name === board.name) return
  const res = await tauriCallSafe('rename_whiteboard', { id: board.id, name })
  if (!res.ok) {
    ElMessage.error(res.error || '重命名失败')
    return
  }
  ElMessage.success('已重命名')
  await loadWhiteboards(board.id)
}

async function deleteBoard() {
  const board = currentBoard.value
  if (!board) return
  try {
    await ElMessageBox.confirm(
      `确定删除白板「${board.name}」？其下 ${board.nodeCount} 个事实节点将一并删除，且不可恢复。`,
      '删除白板',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消', confirmButtonClass: 'el-button--danger' }
    )
  } catch {
    return
  }
  const res = await tauriCallSafe('delete_whiteboard', { id: board.id })
  if (!res.ok) {
    ElMessage.error(res.error || '删除失败')
    return
  }
  ElMessage.success('白板已删除')
  await loadWhiteboards()
}

// ============================================================
// 卡片拖拽（自绘 pointer 拖拽 + 乐观更新）
// ============================================================
interface DragState {
  id: string
  startX: number
  startY: number
  origX: number
  origY: number
  moved: boolean
}
let drag: DragState | null = null

function clamp(v: number, min: number, max: number): number {
  return Math.min(Math.max(v, min), Math.max(min, max))
}

function onCardPointerDown(node: FactNodeDto, e: PointerEvent) {
  if (e.button !== 0) return
  const target = e.target as HTMLElement | null
  if (target && target.closest('button, a, input, textarea, .fact-card__no-drag')) return
  e.preventDefault()
  selectNode(node.id)
  drag = { id: node.id, startX: e.clientX, startY: e.clientY, origX: node.x, origY: node.y, moved: false }
  draggingId.value = node.id
  window.addEventListener('pointermove', onPointerMove)
  window.addEventListener('pointerup', onPointerUp, { once: true })
}

function onPointerMove(e: PointerEvent) {
  if (!drag) return
  const dx = e.clientX - drag.startX
  const dy = e.clientY - drag.startY
  if (!drag.moved && Math.hypot(dx, dy) < 3) return
  drag.moved = true
  const node = nodes.value.find((n) => n.id === drag!.id)
  if (!node) return
  node.x = clamp(drag.origX + dx, 0, CANVAS_W - CARD_W)
  node.y = clamp(drag.origY + dy, 0, CANVAS_H - CARD_MIN_H)
}

async function onPointerUp() {
  window.removeEventListener('pointermove', onPointerMove)
  const finished = drag
  drag = null
  draggingId.value = null
  if (!finished || !finished.moved) return
  const node = nodes.value.find((n) => n.id === finished.id)
  if (!node) return
  const nextX = node.x
  const nextY = node.y
  const res = await tauriCallSafe('update_fact_node', { id: finished.id, x: nextX, y: nextY })
  if (!res.ok) {
    node.x = finished.origX
    node.y = finished.origY
    ElMessage.error('位置保存失败，已回退')
  }
}

// ============================================================
// 连线拖拽交互
// ============================================================
function onConnectionPointerDown(node: FactNodeDto, e: PointerEvent) {
  e.preventDefault()
  e.stopPropagation()
  const viewport = viewportRef.value
  if (!viewport) return

  const rect = viewport.querySelector('.canvas-surface')!.getBoundingClientRect()
  const startX = e.clientX - rect.left
  const startY = e.clientY - rect.top

  edgeDraft.value = {
    sourceId: node.id,
    startX,
    startY,
    endX: startX,
    endY: startY,
  }

  const onEdgeMove = (ev: PointerEvent) => {
    if (!edgeDraft.value) return
    edgeDraft.value.endX = ev.clientX - rect.left
    edgeDraft.value.endY = ev.clientY - rect.top
  }

  const onEdgeUp = async (ev: PointerEvent) => {
    window.removeEventListener('pointermove', onEdgeMove)
    window.removeEventListener('pointerup', onEdgeUp)
    
    if (!edgeDraft.value) return
    const draft = edgeDraft.value
    edgeDraft.value = null

    const targetEl = document.elementFromPoint(ev.clientX, ev.clientY)
    const cardEl = targetEl?.closest('.fact-card')
    if (!cardEl) return
    
    const targetId = (cardEl as HTMLElement).dataset.nodeId
    if (!targetId || targetId === draft.sourceId) return

    // 防重连
    if (edges.value.some(e => e.sourceNodeId === draft.sourceId && e.targetNodeId === targetId)) {
      ElMessage.warning('已经连接过该节点')
      return
    }

    const res = await tauriCallSafe('create_whiteboard_edge', {
      whiteboardId: currentBoardId.value,
      sourceNodeId: draft.sourceId,
      targetNodeId: targetId
    })
    
    if (res.ok && res.data) {
      edges.value.push({
        id: res.data as string,
        whiteboardId: currentBoardId.value,
        sourceNodeId: draft.sourceId,
        targetNodeId: targetId,
        createdAt: new Date().toISOString()
      })
    } else {
      ElMessage.error(res.error || '连线创建失败')
    }
  }

  window.addEventListener('pointermove', onEdgeMove)
  window.addEventListener('pointerup', onEdgeUp)
}

async function deleteEdge(id: string) {
  const res = await tauriCallSafe('delete_whiteboard_edge', { id })
  if (res.ok) {
    edges.value = edges.value.filter(e => e.id !== id)
  } else {
    ElMessage.error(res.error || '连线删除失败')
  }
}

// ============================================================
// 节点选择 / 删除 / 展开
// ============================================================
function selectNode(id: string) {
  selectedNodeId.value = id
}

function onCanvasPointerDown(e: PointerEvent) {
  // 点击非卡片区域即取消选中（.canvas-surface 铺满视口，不能用 target===currentTarget）
  if (!(e.target as HTMLElement | null)?.closest?.('.fact-card')) selectedNodeId.value = null
}

async function deleteNode(node: FactNodeDto) {
  try {
    await ElMessageBox.confirm(
      '确定删除这个事实节点？删除后不可恢复。',
      '删除事实节点',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消', confirmButtonClass: 'el-button--danger' }
    )
  } catch {
    return
  }
  const res = await tauriCallSafe('delete_fact_node', { id: node.id })
  if (!res.ok) {
    ElMessage.error(res.error || '删除失败')
    return
  }
  nodes.value = nodes.value.filter((n) => n.id !== node.id)
  if (selectedNodeId.value === node.id) selectedNodeId.value = null
  if (currentBoard.value) currentBoard.value.nodeCount = nodes.value.length
  ElMessage.success('节点已删除')
}

function toggleExpand(id: string) {
  const next = new Set(expandedIds.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expandedIds.value = next
}

// ============================================================
// 跳转出处
// ============================================================
function gotoSource(node: FactNodeDto) {
  router.push({
    path: `/files/${caseId.value}`,
    // 与 CaseFilesView 的 query 消费约定对齐（select + anchor），并带上页码
    query: node.fileId
      ? { select: node.fileId, ...(node.page ? { anchor: `page:${node.page}` } : {}) }
      : {},
  })
}

// ============================================================
// 添加事实节点对话框
// ============================================================
const addDialogVisible = ref(false)
const addSubmitting = ref(false)
const caseFiles = ref<CaseFileLite[]>([])
const filesLoading = ref(false)
const addForm = ref({
  fileId: '' as string,
  page: null as number | null,
  excerpt: '',
  note: '',
})

async function openAddDialog() {
  if (!currentBoardId.value) {
    ElMessage.warning('请先创建或选择一个白板')
    return
  }
  addForm.value = { fileId: '', page: null, excerpt: '', note: '' }
  addDialogVisible.value = true
  filesLoading.value = true
  const list = await tauriCall('list_case_files', { caseId: caseId.value, category: null })
  filesLoading.value = false
  caseFiles.value = Array.isArray(list) ? list : []
}

async function submitAdd() {
  const excerpt = addForm.value.excerpt.trim()
  if (!excerpt) {
    ElMessage.warning('请填写摘录内容')
    return
  }
  addSubmitting.value = true
  const pos = nextNodePosition()
  const id = await tauriCall('create_fact_node', {
    whiteboardId: currentBoardId.value,
    fileId: addForm.value.fileId || null,
    page: addForm.value.page ?? null,
    excerpt,
    note: addForm.value.note.trim() || null,
    x: pos.x,
    y: pos.y,
  })
  addSubmitting.value = false
  if (!id) return
  addDialogVisible.value = false
  ElMessage.success('事实节点已添加')
  await loadNodes()
  if (currentBoard.value) currentBoard.value.nodeCount = nodes.value.length
  selectedNodeId.value = id
}

/** 落在画布可视中心附近，并按已有节点数错位，避免完全重叠 */
function nextNodePosition(): { x: number; y: number } {
  const viewport = viewportRef.value
  const cx = viewport ? viewport.scrollLeft + viewport.clientWidth / 2 : CANVAS_W / 2
  const cy = viewport ? viewport.scrollTop + viewport.clientHeight / 2 : CANVAS_H / 2
  const stagger = (nodes.value.length % 5) * 28
  return {
    x: clamp(Math.round(cx - CARD_W / 2 + stagger), 0, CANVAS_W - CARD_W),
    y: clamp(Math.round(cy - 120 + stagger), 0, CANVAS_H - CARD_MIN_H),
  }
}

// ============================================================
// 工具
// ============================================================
function formatTime(value: string | null): string {
  if (!value) return ''
  return value.slice(0, 16)
}

function fileSizeLabel(bytes: number | null): string {
  if (!bytes) return ''
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

async function refresh() {
  await loadWhiteboards(currentBoardId.value || undefined)
  await loadNodes()
}

onMounted(async () => {
  await Promise.all([loadCase(), loadWhiteboards()])
})

onBeforeUnmount(() => {
  window.removeEventListener('pointermove', onPointerMove)
  window.removeEventListener('pointerup', onPointerUp)
})
</script>

<template>
  <div class="whiteboard-view">
    <!-- 顶栏 -->
    <header class="whiteboard-topbar">
      <el-button text :icon="ArrowLeft" class="topbar-back" @click="router.push(`/cases/${caseId}`)">
        {{ caseName || '案件详情' }}
      </el-button>
      <el-divider direction="vertical" />

      <template v-if="whiteboards.length">
        <el-select
          :model-value="currentBoardId"
          class="board-select"
          placeholder="选择白板"
          @change="onBoardChange"
        >
          <el-option
            v-for="board in whiteboards"
            :key="board.id"
            :label="board.name"
            :value="board.id"
          />
        </el-select>
        <el-button :icon="Plus" @click="createBoard">新建白板</el-button>
        <el-button :icon="Edit" text :disabled="!currentBoard" @click="renameBoard">
          重命名
        </el-button>
        <el-button :icon="Delete" text type="danger" :disabled="!currentBoard" @click="deleteBoard">
          删除
        </el-button>
      </template>

      <div class="topbar-spacer" />

      <span v-if="currentBoard" class="node-count">
        <el-icon><Collection /></el-icon>{{ nodeCountLabel }}
      </span>
      <el-button :icon="Refresh" text @click="refresh" />
      <el-button type="primary" :icon="Plus" :disabled="!currentBoard" @click="openAddDialog">
        添加事实节点
      </el-button>
    </header>

    <!-- 无白板空态 -->
    <div v-if="!boardsLoading && whiteboards.length === 0" class="whiteboard-empty">
      <EmptyState
        title="还没有事实白板"
        description="为案件创建一块白板，把卷宗中的关键事实摘录为可自由排布的节点。"
        action-text="新建白板"
        @action="createBoard"
      />
    </div>

    <!-- 画布 -->
    <div v-else ref="viewportRef" class="canvas-viewport" @pointerdown="onCanvasPointerDown">
      <div class="canvas-surface" :style="{ width: CANVAS_W + 'px', height: CANVAS_H + 'px' }">
        <!-- SVG 连线层 -->
        <svg class="canvas-edges" :width="CANVAS_W" :height="CANVAS_H">
          <g v-for="edge in edges" :key="edge.id">
            <template v-if="nodes.find(n => n.id === edge.sourceNodeId) && nodes.find(n => n.id === edge.targetNodeId)">
              <path
                :d="getEdgePath(nodes.find(n => n.id === edge.sourceNodeId)!, nodes.find(n => n.id === edge.targetNodeId)!)"
                class="edge-path"
              />
              <path
                :d="getEdgePath(nodes.find(n => n.id === edge.sourceNodeId)!, nodes.find(n => n.id === edge.targetNodeId)!)"
                class="edge-path-hitbox"
                @pointerdown.stop="deleteEdge(edge.id)"
                title="删除连线"
              />
            </template>
          </g>
          <path
            v-if="edgeDraft"
            :d="getDraftPath()"
            class="edge-path is-draft"
          />
        </svg>

        <!-- 无节点提示 -->
        <div v-if="!nodesLoading && nodes.length === 0" class="canvas-hint">
          <el-icon :size="28"><Document /></el-icon>
          <p>点击右上角「添加事实节点」，从卷宗摘录第一条事实。</p>
          <p class="canvas-hint__sub">节点可自由拖拽排布，出处徽标可跳转卷宗文件。</p>
        </div>

        <!-- 事实节点卡片 -->
        <article
          v-for="node in nodes"
          :key="node.id"
          class="fact-card"
          :data-node-id="node.id"
          :class="{
            'is-selected': selectedNodeId === node.id,
            'is-dragging': draggingId === node.id,
          }"
          :style="{ transform: `translate(${node.x}px, ${node.y}px)` }"
          @pointerdown="onCardPointerDown(node, $event)"
        >
          <div class="fact-card__connection-anchor" @pointerdown.stop="onConnectionPointerDown(node, $event)" title="拖拽连线"></div>
          <header class="fact-card__header fact-card__no-drag">
            <button
              v-if="node.fileName"
              class="fact-card__source"
              type="button"
              :title="`查看出处：${node.fileName}`"
              @click.stop="gotoSource(node)"
            >
              <el-icon><Link /></el-icon>
              <span class="fact-card__source-name">{{ node.fileName }}</span>
              <span v-if="node.page != null" class="fact-card__source-page">P{{ node.page }}</span>
            </button>
            <span v-else class="fact-card__source fact-card__source--manual">
              <el-icon><Document /></el-icon>
              <span class="fact-card__source-name">手动摘录</span>
              <span v-if="node.page != null" class="fact-card__source-page">P{{ node.page }}</span>
            </span>
            <el-button
              class="fact-card__delete"
              text
              type="danger"
              size="small"
              :icon="Delete"
              @click.stop="deleteNode(node)"
            />
          </header>

          <p class="fact-card__excerpt" :class="{ 'is-expanded': expandedIds.has(node.id) }">
            {{ node.excerpt }}
          </p>
          <button
            v-if="node.excerpt.length > 120"
            class="fact-card__expand fact-card__no-drag"
            type="button"
            @click.stop="toggleExpand(node.id)"
          >
            {{ expandedIds.has(node.id) ? '收起' : '展开' }}
          </button>

          <p v-if="node.note" class="fact-card__note fact-card__no-drag">{{ node.note }}</p>

          <footer class="fact-card__footer">
            <time>{{ formatTime(node.createdAt) }}</time>
          </footer>
        </article>

        <div v-if="nodesLoading" class="canvas-loading">加载中…</div>
      </div>
    </div>

    <!-- 添加事实节点对话框 -->
    <el-dialog
      v-model="addDialogVisible"
      title="添加事实节点"
      width="560px"
      :close-on-click-modal="false"
    >
      <el-form label-position="top">
        <el-form-item label="出处文件（可选）">
          <el-select
            v-model="addForm.fileId"
            filterable
            clearable
            placeholder="从本案卷宗选择文件，留空则为手动摘录"
            style="width: 100%"
            :loading="filesLoading"
          >
            <el-option
              v-for="file in caseFiles"
              :key="file.id"
              :value="file.id"
              :label="file.fileName"
            >
              <span>{{ file.fileName }}</span>
              <span class="file-option-size">{{ fileSizeLabel(file.fileSize) }}</span>
            </el-option>
          </el-select>
        </el-form-item>
        <el-form-item label="页码（可选）">
          <el-input-number v-model="addForm.page" :min="1" :max="99999" placeholder="页码" />
        </el-form-item>
        <el-form-item label="摘录原文" required>
          <el-input
            v-model="addForm.excerpt"
            type="textarea"
            :rows="5"
            maxlength="2000"
            show-word-limit
            placeholder="粘贴或输入卷宗中的关键事实原文（必填）"
          />
        </el-form-item>
        <el-form-item label="批注（可选）">
          <el-input
            v-model="addForm.note"
            type="textarea"
            :rows="2"
            maxlength="500"
            placeholder="你的分析、疑点或待办"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="addDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="addSubmitting" @click="submitAdd">创建节点</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.whiteboard-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--c-bg-page);
  color: var(--c-text);
}

/* ── 顶栏 ─────────────────────────────── */
.whiteboard-topbar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-topbar);
  backdrop-filter: blur(8px);
  flex-shrink: 0;
}

.topbar-back {
  font-weight: 600;
  color: var(--c-text-heading);
}

.board-select {
  width: 220px;
}

.topbar-spacer {
  flex: 1;
}

.node-count {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--text-sm);
  color: var(--c-text-secondary);
  margin-right: var(--space-2);
}

.whiteboard-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ── 画布 ─────────────────────────────── */
.canvas-viewport {
  flex: 1;
  overflow: auto;
  position: relative;
  min-height: 0;
}

.canvas-surface {
  position: relative;
  background-image: radial-gradient(var(--c-border-strong) 1px, transparent 1px);
  background-size: 24px 24px;
  background-color: var(--c-bg-page);
}

.canvas-hint {
  position: absolute;
  top: 120px;
  left: 50%;
  transform: translateX(-50%);
  text-align: center;
  color: var(--c-text-placeholder);
  font-size: var(--text-base);
  pointer-events: none;
}

.canvas-hint__sub {
  font-size: var(--text-sm);
  color: var(--c-text-placeholder);
  opacity: 0.8;
}

.canvas-loading {
  position: absolute;
  top: 80px;
  left: 50%;
  transform: translateX(-50%);
  color: var(--c-text-secondary);
  font-size: var(--text-sm);
}

/* ── 事实节点卡片 ─────────────────────── */
.fact-card {
  position: absolute;
  top: 0;
  left: 0;
  width: 300px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-md);
  cursor: grab;
  user-select: none;
  transition: box-shadow var(--motion-base) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    transform var(--motion-slow) var(--ease-out);
}

.fact-card.is-dragging {
  cursor: grabbing;
  transition: none; /* 拖拽中禁用过渡，跟手 */
  box-shadow: var(--shadow-xl);
  z-index: 10;
}

.fact-card.is-selected {
  border-color: var(--c-primary);
  box-shadow: 0 0 0 2px var(--c-primary-lighter), var(--shadow-lg);
}

.fact-card__header {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 22px;
}

.fact-card__source {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 230px;
  padding: 2px 8px;
  border: none;
  border-radius: var(--c-radius-full);
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-size: var(--text-xs);
  line-height: 18px;
  cursor: pointer;
  font-family: inherit;
  transition: background var(--motion-fast) var(--ease-out);
}

.fact-card__source:hover {
  background: var(--c-primary-lighter);
}

.fact-card__source--manual {
  background: var(--c-bg-subtle);
  color: var(--c-text-secondary);
  cursor: default;
}

.fact-card__source-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fact-card__source-page {
  flex-shrink: 0;
  font-weight: 600;
}

.fact-card__delete {
  margin-left: auto;
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-out);
}

.fact-card:hover .fact-card__delete,
.fact-card.is-selected .fact-card__delete {
  opacity: 1;
}

.fact-card__excerpt {
  margin: 0;
  font-size: var(--text-base);
  line-height: 1.6;
  color: var(--c-text-regular);
  white-space: pre-wrap;
  word-break: break-word;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
  overflow: hidden;
}

.fact-card__excerpt.is-expanded {
  display: block;
  overflow: visible;
}

.fact-card__expand {
  align-self: flex-start;
  border: none;
  background: none;
  padding: 0;
  color: var(--c-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  font-family: inherit;
}

.fact-card__note {
  margin: 0;
  padding: 6px 8px;
  border-left: 2px solid var(--c-warning);
  background: var(--c-warning-light);
  border-radius: 0 var(--c-radius) var(--c-radius) 0;
  font-size: var(--text-sm);
  color: var(--c-text-regular);
  white-space: pre-wrap;
  word-break: break-word;
}

.fact-card__footer {
  display: flex;
  justify-content: flex-end;
  font-size: var(--text-xs);
  color: var(--c-text-placeholder);
}

.canvas-edges {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
  z-index: 1;
}

.edge-path {
  fill: none;
  stroke: var(--c-border-strong);
  stroke-width: 2px;
  stroke-dasharray: 4, 4;
  transition: stroke var(--motion-fast);
}

.edge-path.is-draft {
  stroke: var(--c-primary);
}

.edge-path-hitbox {
  fill: none;
  stroke: transparent;
  stroke-width: 12px;
  pointer-events: auto;
  cursor: pointer;
}

.edge-path-hitbox:hover + .edge-path,
.edge-path-hitbox:hover ~ .edge-path,
.edge-path-hitbox:hover {
}
g:hover .edge-path {
  stroke: var(--c-danger);
  stroke-dasharray: none;
}

.fact-card__connection-anchor {
  position: absolute;
  right: -8px;
  top: 40px;
  width: 16px;
  height: 16px;
  background: var(--c-bg-card);
  border: 2px solid var(--c-primary);
  border-radius: 50%;
  cursor: crosshair;
  opacity: 0;
  transition: opacity var(--motion-fast);
  z-index: 2;
}

.fact-card:hover .fact-card__connection-anchor {
  opacity: 1;
}

.fact-card__connection-anchor:hover {
  transform: scale(1.2);
  background: var(--c-primary-light);
}

.file-option-size {
  float: right;
  color: var(--c-text-placeholder);
  font-size: var(--text-xs);
}

@media (prefers-reduced-motion: reduce) {
  .fact-card,
  .fact-card__delete,
  .fact-card__source {
    transition: none;
  }
}
</style>
