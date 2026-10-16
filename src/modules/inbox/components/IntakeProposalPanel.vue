<script setup>
/**
 * IntakeProposalPanel —— 待确认建议（UPDATE_PLAN 0.2.0 Phase 1 前端）
 *
 * 铁律的可视化：抽取出的开庭事实**不自动建日程**，全部在这里等用户裁决；
 * 每条建议都带页码出处（"依据：传票第 N 页"），这是自动建档敢让用户信的前提。
 *
 * C1 冲突（同案已有开庭且日期不同）渲染三选一：
 *   两个开庭都建立 / 改期（旧排期标记改期并留痕）/ 都不是开庭（类型误判）。
 * 无案件时先让用户选归卷案件（uncertain_case）。
 */
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import { Calendar, Warning, Check, Close } from '../../../shared/icons'

const proposals = ref([])
const cases = ref([])
const loading = ref(false)
const busy = ref(false)
const rejectReason = ref('')
const rejectingId = ref('')
const assignCaseId = ref('')

const pending = computed(() => proposals.value.filter((p) => p.status === 'pending'))

function docTypeLabel(type) {
  return ({ summons: '传票', hearing_notice: '口审通知书', judgment: '判决书', ruling: '裁定书' }[type]) || type || '文书'
}

function evidenceLine(p) {
  const ev = p.evidence && p.evidence[0] ? p.evidence[0] : {}
  const parts = [docTypeLabel(ev.docType)]
  if (ev.caseNo) parts.push(ev.caseNo)
  if (ev.court) parts.push(ev.court)
  return parts.filter(Boolean).join(' · ')
}

function pageText(p) {
  const ev = p.evidence && p.evidence[0] ? p.evidence[0] : {}
  return ev.page ? `第 ${ev.page} 页` : '页码未定位'
}

function dateText(p) {
  return String(p.payload?.hearingDate || '').slice(0, 16)
}

async function load() {
  loading.value = true
  const result = await casyContext.files.proposals('pending')
  loading.value = false
  if (result.ok && Array.isArray(result.data)) proposals.value = result.data
}

async function loadCases() {
  const result = await casyContext.cases.listAll()
  if (result.ok && Array.isArray(result.data)) cases.value = result.data
}

async function confirm(p, option, extra = {}) {
  if (busy.value) return
  busy.value = true
  const resolution = { option, ...extra }
  const result = await casyContext.files.confirmProposal(p.id, resolution)
  busy.value = false
  if (!result.ok) { ElMessage.error(result.error || '确认失败'); return }
  ElMessage.success(option === 'reschedule' ? '已登记改期，旧排期已标记' : '已建立开庭排期')
  await load()
}

/** C1：改期——需要用户确认取代哪条旧排期，并留依据 */
async function reschedule(p) {
  const existing = p.conflict?.existing || []
  if (!existing.length) { ElMessage.warning('没有可取代的排期'); return }
  try {
    const { value } = await ElMessageBox.prompt(
      `改期依据（如：传票改期通知）：`,
      '标记改期',
      { confirmButtonText: '确认改期', cancelButtonText: '取消', inputPlaceholder: '改期依据' },
    )
    await confirm(p, 'reschedule', {
      supersedeHearingId: existing[0].id,
      reason: String(value || '').trim() || '传票改期',
    })
  } catch { /* 用户取消 */ }
}

/** uncertain_case：先选归卷案件 */
async function assignCase(p) {
  const id = assignCaseId.value
  if (!id) { ElMessage.warning('请先选择归卷案件'); return }
  await confirm(p, 'assign_case', { caseId: id })
}

async function reject(p) {
  const reason = String(rejectReason.value || '').trim()
  if (!reason) { ElMessage.warning('请填写不采用的原因'); return }
  const result = await casyContext.files.rejectProposal(p.id, reason)
  if (!result.ok) { ElMessage.error(result.error || '记录失败'); return }
  rejectingId.value = ''
  rejectReason.value = ''
  ElMessage.success('已记录，后续推荐会参考这个判断')
  await load()
}

onMounted(() => { void load(); void loadCases() })
</script>

<template>
  <section v-if="pending.length" class="proposal-panel" aria-label="待确认建议">
    <header class="proposal-head ui-row ui-row--between">
      <div class="ui-row" style="gap: 8px">
        <el-icon :size="16" color="var(--c-warning)"><WarningFilled /></el-icon>
        <strong>待确认建议 · {{ pending.length }}</strong>
        <span class="proposal-sub">抽取出的开庭事实不会自动建日程，请你确认</span>
      </div>
      <el-button text size="small" :loading="loading" @click="load">刷新</el-button>
    </header>

    <article v-for="p in pending" :key="p.id" class="proposal-card">
      <div class="proposal-card-head ui-row ui-row--between ui-row--wrap">
        <span class="proposal-action">建议建立开庭：<strong>{{ dateText(p) }}</strong></span>
        <span class="proposal-conf">{{ Math.round((p.confidence || 0) * 100) }}%</span>
      </div>
      <p class="proposal-evidence">依据：{{ evidenceLine(p) }} · {{ pageText(p) }}</p>

      <!-- C1：同案开庭日期不一致 -->
      <div v-if="p.conflict && p.conflict.kind === 'same_case_hearing_date'" class="proposal-conflict">
        <p class="conflict-title">该案件已有开庭排期，日期不同：</p>
        <ul class="conflict-existing">
          <li v-for="item in p.conflict.existing" :key="item.id">
            {{ String(item.date || '').slice(0, 16) }}<template v-if="item.name"> · {{ item.name }}</template><template v-if="item.venue"> · {{ item.venue }}</template>
          </li>
        </ul>
        <p class="conflict-question">是真的两个开庭，还是改期了？</p>
        <div class="conflict-actions ui-row ui-row--wrap">
          <el-button size="small" type="primary" :loading="busy" @click="confirm(p, 'two_hearings')">两个开庭都建立</el-button>
          <el-button size="small" :loading="busy" @click="reschedule(p)">改期（留痕）</el-button>
          <el-button size="small" text :loading="busy" @click="confirm(p, 'not_hearing')">都不是开庭</el-button>
        </div>
      </div>

      <!-- C4：归属未定 -->
      <div v-else-if="p.conflict && p.conflict.kind === 'uncertain_case'" class="proposal-conflict">
        <p class="conflict-title">未能匹配到案件，请选择归卷案件：</p>
        <div class="conflict-actions ui-row ui-row--wrap">
          <el-select v-model="assignCaseId" size="small" filterable clearable placeholder="选择案件" style="width: 220px">
            <el-option v-for="c in cases" :key="c.id" :value="c.id" :label="c.caseNo ? `${c.caseName} · ${c.caseNo}` : c.caseName" />
          </el-select>
          <el-button size="small" type="primary" :loading="busy" @click="assignCase(p)">确认并建立</el-button>
        </div>
      </div>

      <!-- 无冲突：直接确认 -->
      <div v-else class="conflict-actions ui-row ui-row--wrap">
        <el-button size="small" type="primary" :loading="busy" @click="confirm(p, 'confirm')">建立开庭</el-button>
      </div>

      <footer class="proposal-foot ui-row ui-row--between">
        <el-button text size="small" @click="rejectingId = rejectingId === p.id ? '' : p.id">不采用</el-button>
      </footer>
      <div v-if="rejectingId === p.id" class="proposal-reject ui-row">
        <el-input v-model="rejectReason" size="small" placeholder="为什么不适用？（如：日期看错了）" />
        <el-button size="small" :loading="busy" @click="reject(p)">记录</el-button>
      </div>
    </article>
  </section>
</template>

<style scoped>
.proposal-panel { margin-bottom: 16px; border: 1px solid var(--c-warning); border-radius: 8px; background: var(--c-bg-card); overflow: hidden; }
.proposal-head { padding: 10px 14px; background: var(--c-warning-light); }
.proposal-sub { font-size: 12px; color: var(--c-text-secondary); }
.proposal-card { padding: 12px 14px; border-top: 1px solid var(--c-border-light); }
.proposal-card-head { gap: 8px; }
.proposal-action { font-size: 13px; }
.proposal-conf { font-size: 11px; color: var(--c-text-secondary); }
.proposal-evidence { margin: 6px 0 10px; font-size: 12px; color: var(--c-text-secondary); line-height: 1.6; }
.proposal-conflict { padding: 10px 12px; border-radius: 6px; background: var(--c-bg-subtle); }
.conflict-title { margin: 0 0 6px; font-size: 12px; font-weight: 600; }
.conflict-existing { margin: 0 0 8px; padding-left: 18px; font-size: 12px; color: var(--c-text-regular); line-height: 1.8; }
.conflict-question { margin: 0 0 8px; font-size: 12px; color: var(--c-warning); font-weight: 600; }
.conflict-actions { gap: 8px; }
.proposal-foot { margin-top: 8px; }
.proposal-reject { gap: 8px; margin-top: 8px; }
</style>
