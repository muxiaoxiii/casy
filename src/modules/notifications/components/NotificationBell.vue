<script setup lang="ts">
/**
 * NotificationBell —— W2 Linear 式安静通知中心（顶栏入口）
 *
 * 设计语义：
 * - Inbox-Zero：通知是"待处理项"而非历史记录，「处理掉」即从列表消失；
 * - 克制的灰蓝角标（不用刺眼红），未读为 0 时不显示角标；
 * - 60s 轮询未读数，面板打开时拉取全量列表；
 * - payloadJson 含 caseId/taskId 时点击跳转对应路由（/cases/:id、/tasks）。
 */
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Bell, AlarmClock, InfoFilled, Check, Delete, CircleCheck } from '../../../shared/icons'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'

// ── 本地类型契约（对齐后端 AppNotification DTO，serde camelCase）──
interface AppNotification {
  id: string
  type: string
  title: string
  body: string | null
  payloadJson: string | null
  createdAt: string | null
  readAt: string | null
}

interface NotificationPayload {
  caseId?: string | null
  taskId?: string | null
  calendarDate?: string | null
  level?: string | null
}

const router = useRouter()

const unreadCount = ref(0)
const notifications = ref<AppNotification[]>([])
const panelOpen = ref(false)
const listLoading = ref(false)
const bellRef = ref<HTMLElement | null>(null)

let pollTimer: ReturnType<typeof setInterval> | null = null

async function refreshUnread() {
  const n = await tauriCall('unread_notification_count', {}, { silent: true })
  if (typeof n === 'number') unreadCount.value = n
}

async function loadList() {
  listLoading.value = true
  const list = await tauriCall('list_notifications', {}, { silent: true })
  if (Array.isArray(list)) notifications.value = list
  listLoading.value = false
}

async function togglePanel() {
  panelOpen.value = !panelOpen.value
  if (panelOpen.value) await loadList()
}

async function dismissOne(n: AppNotification) {
  // 乐观移除（Inbox-Zero：处理即消失）；失败回滚并提示
  const idx = notifications.value.findIndex(x => x.id === n.id)
  notifications.value = notifications.value.filter(x => x.id !== n.id)
  if (!n.readAt) unreadCount.value = Math.max(0, unreadCount.value - 1)
  const res = await tauriCallSafe('dismiss_notification', { id: n.id })
  if (!res.ok) {
    if (idx >= 0) notifications.value.splice(Math.min(idx, notifications.value.length), 0, n)
    if (!n.readAt) unreadCount.value += 1
    ElMessage.error('处理通知失败，请重试')
  }
}

async function dismissAll() {
  const backup = [...notifications.value]
  const backupUnread = unreadCount.value
  notifications.value = []
  unreadCount.value = 0
  const res = await tauriCallSafe('dismiss_all_notifications', {})
  if (!res.ok) {
    notifications.value = backup
    unreadCount.value = backupUnread
    ElMessage.error('清空失败，请重试')
  }
}

function parsePayload(n: AppNotification): NotificationPayload | null {
  if (!n.payloadJson) return null
  try {
    return JSON.parse(n.payloadJson) as NotificationPayload
  } catch {
    return null
  }
}

function hasTarget(n: AppNotification): boolean {
  const p = parsePayload(n)
  return !!(p && (p.caseId || p.taskId || p.calendarDate))
}

async function openNotification(n: AppNotification) {
  if (!n.readAt) {
    unreadCount.value = Math.max(0, unreadCount.value - 1)
    n.readAt = new Date().toISOString()
    void tauriCall('mark_notification_read', { id: n.id }, { silent: true })
  }
  const p = parsePayload(n)
  if (p?.calendarDate) {
    panelOpen.value = false
    router.push({ path: '/calendar', query: { date: p.calendarDate, view: 'day' } })
  } else if (p?.caseId) {
    panelOpen.value = false
    router.push(`/cases/${p.caseId}`)
  } else if (p?.taskId) {
    panelOpen.value = false
    // 携带定位 query（TasksView 消费 ?edit= 打开任务抽屉）
    router.push({ path: '/tasks', query: { edit: p.taskId } })
  }
}

function typeIcon(type: string) {
  if (type === 'reminder' || type === 'rest_day_work') return AlarmClock
  return InfoFilled
}

/** 相对时间：刚刚 / n 分钟前 / n 小时前 / 昨天 / 月-日 */
function relativeTime(iso: string | null): string {
  if (!iso) return ''
  // SQLite datetime('now','localtime') 输出 'YYYY-MM-DD HH:MM:SS'，按本地时间解析
  const normalized = iso.includes('T') ? iso : iso.replace(' ', 'T')
  const t = new Date(normalized)
  if (Number.isNaN(t.getTime())) return iso
  const diff = Date.now() - t.getTime()
  const min = Math.floor(diff / 60000)
  if (min < 1) return '刚刚'
  if (min < 60) return `${min} 分钟前`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr} 小时前`
  if (hr < 48) return '昨天'
  return `${t.getMonth() + 1}月${t.getDate()}日`
}

function onClickOutside(e: MouseEvent) {
  if (panelOpen.value && bellRef.value && !bellRef.value.contains(e.target as Node)) {
    panelOpen.value = false
  }
}

onMounted(() => {
  refreshUnread()
  pollTimer = setInterval(refreshUnread, 60_000)
  document.addEventListener('mousedown', onClickOutside)
})

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer)
  document.removeEventListener('mousedown', onClickOutside)
})
</script>

<template>
  <div ref="bellRef" class="nc-root">
    <!-- 铃铛按钮 + 克制灰蓝角标 -->
    <button
      class="nc-bell-btn"
      :class="{ open: panelOpen }"
      title="通知中心"
      @click="togglePanel"
    >
      <el-icon :size="17"><Bell /></el-icon>
      <span v-if="unreadCount > 0" class="nc-badge">
        {{ unreadCount > 99 ? '99+' : unreadCount }}
      </span>
    </button>

    <!-- 自绘 dropdown 面板（transform/opacity 动效） -->
    <transition name="nc-pop">
      <div v-if="panelOpen" class="nc-panel">
        <div class="nc-header">
          <span class="nc-title">通知中心</span>
          <button
            v-if="notifications.length"
            class="nc-clear-all"
            @click="dismissAll"
          >
            <el-icon :size="12"><Delete /></el-icon>
            全部清空
          </button>
        </div>

        <div v-if="listLoading" class="nc-empty">加载中…</div>

        <div v-else-if="!notifications.length" class="nc-empty">
          <el-icon :size="22" class="nc-empty-icon"><CircleCheck /></el-icon>
          <span>一切就绪，暂无待处理通知</span>
        </div>

        <div v-else class="nc-list">
          <div
            v-for="n in notifications"
            :key="n.id"
            class="nc-item"
            :class="{ unread: !n.readAt, clickable: hasTarget(n) }"
            @click="openNotification(n)"
          >
            <span class="nc-item-icon" :data-type="n.type">
              <el-icon :size="14"><component :is="typeIcon(n.type)" /></el-icon>
            </span>
            <div class="nc-item-body">
              <div class="nc-item-top">
                <span class="nc-item-title">{{ n.title }}</span>
                <span class="nc-item-time">{{ relativeTime(n.createdAt) }}</span>
              </div>
              <p v-if="n.body" class="nc-item-text">{{ n.body }}</p>
            </div>
            <button
              class="nc-item-dismiss"
              title="处理掉"
              @click.stop="dismissOne(n)"
            >
              <el-icon :size="13"><Check /></el-icon>
            </button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.nc-root {
  position: relative;
  flex-shrink: 0;
}

.nc-bell-btn {
  position: relative;
  width: 34px;
  height: 34px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--c-text-regular);
  cursor: pointer;
  display: grid;
  place-items: center;
  transition: background var(--motion-fast) var(--ease-out), color var(--motion-fast) var(--ease-out);
}

.nc-bell-btn:hover,
.nc-bell-btn.open {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

/* 克制的灰蓝角标（非刺眼红） */
.nc-badge {
  position: absolute;
  top: 2px;
  right: 0;
  min-width: 15px;
  height: 15px;
  padding: 0 4px;
  border-radius: 8px;
  background: #5b7a9e;
  color: #fff;
  font-size: 9.5px;
  font-weight: 700;
  font-family: var(--font-mono);
  display: grid;
  place-items: center;
  line-height: 1;
  box-shadow: 0 0 0 1.5px var(--c-bg-topbar);
}

.nc-panel {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  width: 340px;
  max-height: 420px;
  display: flex;
  flex-direction: column;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm), 0 8px 24px rgba(15, 23, 42, 0.12);
  z-index: 100;
  overflow: hidden;
}

.nc-pop-enter-active {
  transition: opacity var(--motion-base) var(--ease-out), transform var(--motion-base) var(--ease-out);
}
.nc-pop-leave-active {
  transition: opacity var(--motion-fast) var(--ease-out), transform var(--motion-fast) var(--ease-out);
}
.nc-pop-enter-from,
.nc-pop-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

.nc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  border-bottom: 1px solid var(--c-border);
  flex-shrink: 0;
}

.nc-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.nc-clear-all {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  color: var(--c-text-secondary);
  font-size: 11.5px;
  cursor: pointer;
  padding: 3px 6px;
  border-radius: var(--c-radius);
  transition: color var(--motion-fast) var(--ease-out), background var(--motion-fast) var(--ease-out);
}

.nc-clear-all:hover {
  color: var(--c-text);
  background: var(--c-bg-hover);
}

.nc-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 36px 0;
  font-size: 12px;
  color: var(--slate-gray-light);
}

.nc-empty-icon {
  color: var(--status-success);
  opacity: 0.7;
}

.nc-list {
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.nc-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--c-border);
  transition: background var(--motion-fast) var(--ease-out);
}

.nc-item:last-child { border-bottom: none; }

.nc-item.clickable { cursor: pointer; }
.nc-item.clickable:hover { background: var(--c-bg-hover); }

.nc-item.unread .nc-item-title { font-weight: 600; }

.nc-item-icon {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border-radius: var(--c-radius-lg);
  display: grid;
  place-items: center;
  margin-top: 1px;
  color: #5b7a9e;
  background: color-mix(in srgb, #5b7a9e 12%, transparent);
}

.nc-item-icon[data-type='reminder'] {
  color: var(--c-warning, #b0823a);
  background: color-mix(in srgb, #b0823a 12%, transparent);
}

.nc-item-body {
  flex: 1;
  min-width: 0;
}

.nc-item-top {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.nc-item-title {
  font-size: 12.5px;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nc-item-time {
  flex-shrink: 0;
  font-size: 10.5px;
  color: var(--slate-gray-light);
}

.nc-item-text {
  margin: 3px 0 0;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--c-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-line;
}

.nc-item-dismiss {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 1px solid var(--c-border);
  background: transparent;
  color: var(--c-text-secondary);
  cursor: pointer;
  display: grid;
  place-items: center;
  margin-top: 2px;
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-out), color var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out);
}

.nc-item:hover .nc-item-dismiss { opacity: 1; }

.nc-item-dismiss:hover {
  color: var(--status-success);
  border-color: var(--status-success);
}

@media (prefers-reduced-motion: reduce) {
  .nc-pop-enter-active,
  .nc-pop-leave-active,
  .nc-bell-btn,
  .nc-item,
  .nc-item-dismiss,
  .nc-clear-all {
    transition: none;
  }
}
</style>
