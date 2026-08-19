<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { tauriCallSafe } from '../../../core/tauriBridge'

const router = useRouter()
const loading = ref(false)
const degraded = ref(false)
const nodes = ref([])
const edges = ref([])
const selectedNode = ref(null)

const colors = {
  knowledge: '#6C6A9C',
  case: '#3E5C9A',
  task: '#4C8067',
}

const labels = {
  knowledge: '知识',
  case: '案件',
  task: '任务',
}

const svgWidth = ref(800)
const svgHeight = 520

const nodeById = computed(() => {
  const map = {}
  for (const nd of nodes.value) map[nd.id] = nd
  return map
})

// 边端点解析为节点对象（两端都需在节点集内才画）
const edgeViews = computed(() =>
  edges.value
    .map(e => ({ source: nodeById.value[e.source], target: nodeById.value[e.target], type: e.type }))
    .filter(e => e.source && e.target)
)

function nodeRadius(nd) {
  return nd.type === 'case' ? 20 : nd.type === 'knowledge' ? 14 : 12
}

function shortName(nd) {
  const name = nd.name || ''
  return name.length > 10 ? name.slice(0, 10) + '…' : name
}

// ============================================================
// 手写力导向布局：斥力 + 边弹簧 + 向心重力，迭代 160 次
// ============================================================
function layoutGraph(nodeList, edgeList, width, height) {
  const n = nodeList.length
  if (!n) return
  const idx = new Map(nodeList.map((nd, i) => [nd.id, i]))

  // 初始：均匀环形
  const r0 = Math.min(width, height) * 0.35
  nodeList.forEach((nd, i) => {
    const a = (i / n) * Math.PI * 2
    nd.x = width / 2 + Math.cos(a) * r0
    nd.y = height / 2 + Math.sin(a) * r0
  })

  const ITER = 160
  for (let it = 0; it < ITER; it++) {
    const cooling = 1 - it / ITER

    // 斥力（全对）
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        let dx = nodeList[i].x - nodeList[j].x
        let dy = nodeList[i].y - nodeList[j].y
        let d2 = dx * dx + dy * dy
        if (d2 < 1) { dx = Math.random() - 0.5; dy = Math.random() - 0.5; d2 = 1 }
        const d = Math.sqrt(d2)
        const f = Math.min(2400 / d2, 12) * cooling
        const fx = (dx / d) * f
        const fy = (dy / d) * f
        nodeList[i].x += fx; nodeList[i].y += fy
        nodeList[j].x -= fx; nodeList[j].y -= fy
      }
    }

    // 边弹簧（目标距离 120）
    for (const e of edgeList) {
      const a = nodeList[idx.get(e.source)]
      const b = nodeList[idx.get(e.target)]
      if (!a || !b) continue
      const dx = b.x - a.x
      const dy = b.y - a.y
      const d = Math.max(1, Math.hypot(dx, dy))
      const f = (d - 120) * 0.02 * cooling
      const fx = (dx / d) * f
      const fy = (dy / d) * f
      a.x += fx; a.y += fy
      b.x -= fx; b.y -= fy
    }

    // 向心重力
    for (const nd of nodeList) {
      nd.x += (width / 2 - nd.x) * 0.01
      nd.y += (height / 2 - nd.y) * 0.01
    }
  }

  // 收进画布
  for (const nd of nodeList) {
    nd.x = Math.max(50, Math.min(width - 50, nd.x))
    nd.y = Math.max(40, Math.min(height - 40, nd.y))
  }
}

async function loadGraph() {
  loading.value = true
  degraded.value = false
  const result = await tauriCallSafe('get_knowledge_graph', { limit: 100 })
  loading.value = false
  if (result.ok && result.data) {
    nodes.value = (result.data.nodes || []).map(nd => ({ ...nd }))
    edges.value = result.data.edges || []
    layoutGraph(nodes.value, edges.value, svgWidth.value, svgHeight)
  } else {
    nodes.value = []
    edges.value = []
    degraded.value = true
  }
}

onMounted(() => {
  const container = document.getElementById('graph-container')
  if (container?.clientWidth) svgWidth.value = container.clientWidth
  loadGraph()
})

// ============================================================
// 拖拽（SVG pointer 事件）
// ============================================================
const dragState = ref(null) // { node, startX, startY, moved }

function startDrag(event, nd) {
  event.preventDefault()
  dragState.value = { node: nd, startX: event.clientX, startY: event.clientY, moved: false }
}

function onDrag(event) {
  const drag = dragState.value
  if (!drag) return
  const svg = document.getElementById('graph-svg')
  if (!svg) return
  const rect = svg.getBoundingClientRect()
  const x = event.clientX - rect.left
  const y = event.clientY - rect.top
  if (Math.abs(event.clientX - drag.startX) + Math.abs(event.clientY - drag.startY) > 4) {
    drag.moved = true
  }
  drag.node.x = Math.max(20, Math.min(rect.width - 20, x))
  drag.node.y = Math.max(20, Math.min(rect.height - 20, y))
}

function endDrag() {
  dragState.value = null
}

function onNodeClick(nd) {
  // 拖拽后不触发点击
  if (dragState.value?.moved) return
  selectedNode.value = selectedNode.value?.id === nd.id ? null : nd
}

function goToNode(node) {
  const rawId = node.id.replace(/^(k-|c-|t-)/, '')
  if (node.type === 'case') {
    router.push({ name: 'case-detail', params: { id: rawId } })
  } else if (node.type === 'knowledge') {
    router.push({ name: 'knowledge', query: { select: rawId } })
  } else if (node.type === 'task') {
    router.push({ name: 'tasks', query: { edit: rawId } })
  }
}
</script>

<template>
  <div class="knowledge-graph fade-in">
    <div class="graph-header">
      <h3>知识图谱</h3>
      <span class="graph-sub">知识 ↔ 案件 ↔ 任务关系网络（真实关联，来自后端）</span>
      <el-button size="small" text @click="loadGraph">刷新</el-button>
    </div>

    <div class="graph-legend">
      <span v-for="(color, type) in colors" :key="type" class="legend-item">
        <span class="legend-dot" :style="{ background: color }" />
        {{ labels[type] }}
      </span>
      <span class="legend-hint">节点可拖拽；点击节点查看详情</span>
    </div>

    <div v-if="degraded" class="degrade-banner">
      图谱数据不可用，请稍后重试
    </div>

    <div id="graph-container" class="graph-container" v-loading="loading">
      <svg
        v-if="nodes.length"
        id="graph-svg"
        :width="svgWidth"
        :height="svgHeight"
        @pointermove="onDrag"
        @pointerup="endDrag"
        @pointerleave="endDrag"
      >
        <!-- 边 -->
        <line
          v-for="(e, i) in edgeViews"
          :key="i"
          :x1="e.source.x"
          :y1="e.source.y"
          :x2="e.target.x"
          :y2="e.target.y"
          stroke="#CBD2DB"
          stroke-width="1.5"
          stroke-opacity="0.6"
        >
          <title>{{ e.type }}</title>
        </line>

        <!-- 节点 -->
        <g
          v-for="nd in nodes"
          :key="nd.id"
          :transform="`translate(${nd.x},${nd.y})`"
          class="graph-node"
          @pointerdown="startDrag($event, nd)"
          @click="onNodeClick(nd)"
        >
          <circle
            :r="nodeRadius(nd)"
            :fill="colors[nd.type] || '#9BA2AF'"
            fill-opacity="0.15"
            :stroke="colors[nd.type] || '#9BA2AF'"
            :stroke-width="selectedNode?.id === nd.id ? 3 : 2"
          />
          <text
            text-anchor="middle"
            :dy="nodeRadius(nd) + 12"
            class="node-label"
          >{{ shortName(nd) }}</text>
          <title>{{ nd.name }}</title>
        </g>
      </svg>
      <div v-else-if="!loading && !degraded" class="graph-empty">
        暂无图谱数据——知识条目关联案件或任务后会出现在这里
      </div>
    </div>

    <!-- 选中节点详情 -->
    <div v-if="selectedNode" class="selected-node-card">
      <div class="node-header">
        <span class="node-dot" :style="{ background: colors[selectedNode.type] || '#9BA2AF' }" />
        <span class="node-type">{{ labels[selectedNode.type] || selectedNode.type }}</span>
        <button class="close-btn" @click="selectedNode = null">✕</button>
      </div>
      <div class="node-name">{{ selectedNode.name }}</div>
      <div v-if="selectedNode.category" class="node-meta">分类: {{ selectedNode.category }}</div>
      <button
        class="btn btn-primary btn-sm"
        @click="goToNode(selectedNode)"
      >
        查看{{ labels[selectedNode.type] || '详情' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.knowledge-graph {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.graph-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.graph-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #1F2430;
}

.graph-sub {
  font-size: 12px;
  color: #9BA2AF;
}

.graph-legend {
  display: flex;
  gap: 16px;
  margin-bottom: 12px;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #4B5160;
}

.legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.legend-hint {
  margin-left: auto;
  font-size: 11px;
  color: #9BA2AF;
}

.degrade-banner {
  padding: 8px 12px;
  margin-bottom: 12px;
  background: #F7F1E3;
  border: 1px solid #E4D3A8;
  border-radius: 6px;
  font-size: 12px;
  color: #7A5B24;
}

.graph-container {
  flex: 1;
  min-height: 400px;
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  overflow: hidden;
}

.graph-node {
  cursor: grab;
}

.graph-node:active {
  cursor: grabbing;
}

.node-label {
  font-size: 10px;
  fill: #4B5160;
  pointer-events: none;
  user-select: none;
}

.graph-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  min-height: 400px;
  color: #9BA2AF;
  font-size: 13px;
}

.selected-node-card {
  margin-top: 12px;
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 12px 16px;
}

.node-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.node-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.node-type {
  font-size: 11px;
  color: #9BA2AF;
  text-transform: uppercase;
}

.close-btn {
  margin-left: auto;
  background: none;
  border: none;
  color: #9BA2AF;
  cursor: pointer;
  font-size: 14px;
}

.node-name {
  font-size: 14px;
  font-weight: 600;
  color: #1F2430;
  margin-bottom: 4px;
}

.node-meta {
  font-size: 12px;
  color: #9BA2AF;
  margin-bottom: 4px;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid transparent;
  margin-top: 8px;
}

.btn-primary {
  background: #3E5C9A;
  color: white;
}
</style>
