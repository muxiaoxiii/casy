<script setup lang="ts">
/**
 * RenameWorkbench —— 智能重命名工作台 v1（index-v2 规则版）
 *
 * - 规则提取：日期(YYYYMMDD/MM-DD/中文日期) + 文档类型关键词 + 副本噪声清理
 * - 行：原文件名 → 建议新名（提取 chip 可视化依据），可单行编辑覆盖
 * - 应用：单条 / 全部批量（后端磁盘+DB 同步，冲突自动去重）
 */
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowRight, Check, Close } from '../../../shared/icons'
import { casyContext } from '../../../core/plugin/context'
import EmptyState from '../../../shared/components/EmptyState.vue'

const props = defineProps<{ caseId: string; files: any[]; dirName: string }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'applied'): void }>()

const TYPE_KEYWORDS = [
  '请求书', '答辩状', '答辩', '证据', '判决', '决定', '庭审', '口审',
  '起诉状', '代理词', '意见陈述', '清单', '报告', '函件', '函', '委托书',
  '营业执照', '身份证', '扫描件', '会议纪要', '复盘',
]

interface Row {
  id: string
  orig: string
  suggested: string
  chips: string[]
  edited: string | null
  done: boolean
}

const rows = ref<Row[]>([])
const applying = ref(false)

/** 规则引擎：从原名提取 → 建议新名 */
function suggest(orig: string): { name: string; chips: string[] } {
  const chips: string[] = []
  const stem = orig.replace(/\.[^.]+$/, '')

  // 1) 日期：20260824 / 2026-08-24 / 2026.8.24 / 中文 8月24日
  let date = ''
  const mDate =
    stem.match(/(20\d{2})[-._年]?(0?[1-9]|1[0-2])[-._月]?(0?[1-9]|[12]\d|3[01])/) ||
    stem.match(/(0?[1-9]|1[0-2])月(0?[1-9]|[12]\d|3[01])日/)
  if (mDate) {
    const y = mDate[1].length === 4 ? mDate[1] : String(new Date().getFullYear())
    const offset = mDate[1].length === 4 ? 1 : 0
    date = `${y}${mDate[1 + offset].padStart(2, '0')}${mDate[2 + offset].padStart(2, '0')}`
    chips.push(date)
  }

  // 2) 文档类型关键词
  let type = ''
  for (const kw of TYPE_KEYWORDS) {
    if (stem.includes(kw)) {
      type = kw
      chips.push(type)
      break
    }
  }

  // 3) 噪声清理：副本后缀 / 扫描件_ / 微信图片_ / 空括号
  let clean = stem
    .replace(/[(（]\d+[)）]\s*$/g, '')
    .replace(/^(扫描件|微信图片|Screenshot)_?[\d-_]*/i, '')
    .replace(/[_\s-]+$/g, '')
    .trim()

  // 新名组装：日期_类型_清洗后主体（去重段）
  let body = clean
  if (mDate) body = body.replace(mDate[0], '').replace(/^日/, '')
  if (type) body = body.replace(type, '')
  body = body.replace(/^[_\s-]+|[_\s-]+$/g, '').replace(/_{2,}/g, '_')
  const name = [date, type, body || undefined].filter(Boolean).join('_')

  return { name, chips }
}

onMounted(() => {
  rows.value = props.files.map(f => {
    const s = suggest(f.fileName)
    return { id: f.id, orig: f.fileName, suggested: s.name, chips: s.chips, edited: null, done: false }
  })
})

const pendingRows = computed(() => rows.value.filter(r => !r.done && currentName(r) && currentName(r) !== r.orig))
const doneCount = computed(() => rows.value.filter(r => r.done).length)

function currentName(r: Row): string {
  return (r.edited ?? r.suggested).trim()
}

function editRow(r: Row) {
  // 进入手动编辑：以当前建议为底稿
  r.edited = currentName(r)
}

async function applyRow(r: Row) {
  if (applying.value) return
  const newName = currentName(r)
  if (!newName || newName === r.orig) return
  applying.value = true
  try {
    const result = await casyContext.files.applyRenames(props.caseId, [{ id: r.id, newName }])
    if (!result.ok) return ElMessage.error(result.error || '重命名失败')
    acceptOutcomes(result.data || [])
    emit('applied')
  } finally {
    applying.value = false
  }
}

function acceptOutcomes(outcomes: { id: string; newName: string; warning?: string | null }[]) {
  for (const outcome of outcomes) {
    const row = rows.value.find(r => r.id === outcome.id)
    if (!row) continue
    row.suggested = outcome.newName
    row.edited = null
    row.done = true
    if (outcome.warning) ElMessage.warning(outcome.warning)
  }
}

async function applyAll() {
  if (applying.value) return
  const targets = pendingRows.value
  if (!targets.length) {
    ElMessage.info('没有需要重命名的文件')
    return
  }
  applying.value = true
  const renames = targets.map(r => ({ id: r.id, newName: currentName(r) }))
  try {
    const result = await casyContext.files.applyRenames(props.caseId, renames)
    if (result.ok) {
      const n = result.data?.length ?? 0
      ElMessage.success(`已批量重命名 ${n} 个文件`)
      acceptOutcomes(result.data || [])
      emit('applied')
    } else {
      ElMessage.error(result.error || '批量应用失败')
    }
  } finally {
    applying.value = false
  }
}
</script>

<template>
  <div class="rename-view open">
    <div class="rename-head">
      <span class="rh-title">智能重命名 · {{ dirName }}</span>
      <span class="rh-rule">规则：日期 + 文档类型 + 清洗主体</span>
      <el-button class="back" size="small" text @click="emit('close')">返回列表</el-button>
    </div>

    <div class="rename-rows">
      <div
        v-for="r in rows"
        :key="r.id"
        class="rename-row"
        :class="{ done: r.done, skip: r.suggested === r.orig && !r.done }"
      >
        <div class="rr-orig">
          <div class="k">原名</div>
          <div class="v">{{ r.orig }}</div>
        </div>
        <div class="rr-arrow"><el-icon><ArrowRight /></el-icon></div>
        <div class="rr-new">
          <div class="k">
            新名
            <span v-for="(c, ci) in r.chips" :key="ci" class="extract-chip">{{ c }}</span>
          </div>
          <template v-if="!r.done">
            <input v-model="r.edited" class="rn-input" :placeholder="r.suggested" @focus="editRow(r)" />
            <div class="extract">
              <span class="extract-chip">{{ currentName(r) }}</span>
              <button class="mini-btn" title="采用建议" @click.stop="r.edited = null; editRow(r)">
                <el-icon><Check /></el-icon>
              </button>
              <button class="mini-btn" title="放弃编辑" @click.stop="r.edited = null">
                <el-icon><Close /></el-icon>
              </button>
            </div>
          </template>
          <div v-else class="v done-text">{{ r.suggested }}</div>
        </div>
        <div class="rr-ops">
          <el-button
            v-if="!r.done"
            size="small"
            type="primary"
            plain
            :disabled="applying || !currentName(r) || currentName(r) === r.orig"
            @click="applyRow(r)"
          >
            应用
          </el-button>
          <span v-else class="rr-badge ai">✓ 已完成</span>
        </div>
      </div>

      <EmptyState
        v-if="!rows.length"
        type="custom"
        title="此目录没有可重命名的登记文件"
        description="切回文件列表确认目录选择，或先导入文件。"
        compact
      />
    </div>

    <div class="rename-footer">
      <span class="rf-count">待应用 {{ pendingRows.length }} 条 · 已完成 {{ doneCount }} 条</span>
      <el-button type="primary" :loading="applying" @click="applyAll">全部应用</el-button>
    </div>
  </div>
</template>

<style scoped>
.rename-view { padding: 2px; }
.rename-head {
  display: flex; align-items: center; gap: 10px;
  margin-bottom: 12px; flex-wrap: wrap;
}
.rh-title { font-size: 13px; font-weight: 600; color: var(--c-text); }
.rh-rule {
  font-size: 10.5px; color: var(--c-text-secondary);
  border: 1px solid var(--c-border); border-radius: 4px;
  padding: 2px 8px; font-family: var(--font-mono); background: var(--gray-50);
}
.back { margin-left: auto; }

.rename-row {
  display: flex; align-items: center; gap: 12px;
  padding: 12px; margin-bottom: 8px;
  border: 1px solid var(--c-border); border-radius: var(--c-radius-lg);
  background: var(--c-surface);
  transition: border-color var(--motion-fast) var(--ease-out), background var(--motion-fast) var(--ease-out);
}
.rename-row.done {
  border-color: var(--c-success);
  background: var(--c-success-light);
}
.rename-row.skip { opacity: 0.55; }

.rr-orig { flex: 1; min-width: 0; }
.rr-orig .k { font-size: 10px; color: var(--c-text-secondary); margin-bottom: 3px; }
.rr-orig .v {
  font-size: 12px; color: var(--c-text-regular); font-family: var(--font-mono);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.rr-arrow {
  width: 26px; height: 26px; border-radius: 50%;
  background: var(--c-primary-light); color: var(--c-primary);
  display: flex; align-items: center; justify-content: center; flex-shrink: 0;
}
.rr-new { flex: 1.2; min-width: 0; }
.rr-new .k {
  font-size: 10px; color: var(--c-text-secondary); margin-bottom: 3px;
  display: flex; align-items: center; gap: 5px;
}
.extract-chip {
  font-size: 9.5px; height: 16px; padding: 0 6px; border-radius: 999px;
  background: var(--c-primary-light); color: var(--c-primary);
  display: inline-flex; align-items: center; font-family: var(--font-mono);
}
.rn-input {
  width: 100%;
  font-size: 12.5px; font-weight: 600; color: var(--c-primary);
  font-family: var(--font-mono);
  border: 1px solid var(--c-border); border-radius: var(--c-radius-btn);
  padding: 4px 8px; outline: none;
}
.rn-input:focus { border-color: var(--c-primary); }
.extract { display: flex; gap: 4px; margin-top: 5px; flex-wrap: wrap; align-items: center; }
.mini-btn {
  border: none; background: transparent; cursor: pointer;
  color: var(--c-text-secondary); padding: 2px;
  display: inline-flex; align-items: center;
}
.mini-btn:hover { color: var(--c-text); }
.v { font-size: 12px; color: var(--c-text-regular); font-family: var(--font-mono); }
.done-text { color: var(--c-success); font-weight: 600; }
.rr-ops { flex-shrink: 0; }
.rr-badge {
  font-size: 9.5px; height: 16px; padding: 0 6px; border-radius: 999px;
  display: inline-flex; align-items: center; font-weight: 600;
}
.rr-badge.ai { background: var(--c-primary); color: #fff; }

.rename-footer {
  display: flex; align-items: center; gap: 10px;
  padding-top: 12px; border-top: 1px solid var(--c-border); margin-top: 4px;
}
.rf-count { font-size: 12px; color: var(--c-text-secondary); margin-right: auto; }
</style>
