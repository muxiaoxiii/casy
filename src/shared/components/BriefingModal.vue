<script setup lang="ts">
import { computed, nextTick, ref, type PropType } from 'vue'
import { Close, CopyDocument, Download, Refresh } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import html2canvas from 'html2canvas'
import waxSealOxblood from '../../assets/briefing/wax-seal-oxblood.png'
import waxSealAntiqueGold from '../../assets/briefing/wax-seal-antique-gold.png'
import paperFiberWarm from '../../assets/briefing/paper-fiber-warm.png'
import {
  useBriefingModel,
  type BriefingType,
  type FocusItem,
  type RedlineItem,
  type HearingItem,
  type ReportMetrics,
} from './briefing/useBriefingModel'
import { renderBriefingMarkdown } from './briefing/briefingMarkdown'

const props = defineProps({
  visible: { type: Boolean, default: false },
  type: { type: String as PropType<BriefingType>, default: 'daily' },
  styleVariant: { type: String, default: '' },
  title: { type: String, default: '' },
  dateText: { type: String, default: '' },
  content: { type: String, default: '' },
  dateRange: { type: String, default: '' },
  loading: { type: Boolean, default: false },
  preview: { type: Boolean, default: false },
  nextAction: { type: Object as PropType<FocusItem | null>, default: null },
  redlines: { type: Array as PropType<RedlineItem[]>, default: () => [] },
  hearings: { type: Array as PropType<HearingItem[]>, default: () => [] },
  metrics: {
    type: Object as PropType<ReportMetrics>,
    default: () => ({ committedHours: '0.0', freeSpaceHours: '8.5h', waitingCount: 0, completedCount: 0, totalCases: 0 }),
  },
})

const emit = defineEmits(['update:visible', 'regenerate'])
const reportSheetRef = ref<HTMLElement | null>(null)
const exporting = ref(false)
const generatedAt = new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit' }).format(new Date())

const {
  activeStyle,
  activeMeta,
  reportTypeLabel,
  reportTypeCn,
  effectiveTitle,
  effectiveDate,
  reportCode,
  displayNextAction,
  displayRedlines,
  displayHearings,
  displayContent,
  displayMetrics,
  metricCards,
} = useBriefingModel(props)

const sealSrc = computed(() => (activeMeta.value.seal === 'gold' ? waxSealAntiqueGold : waxSealOxblood))
const reportStyleVars = computed(() => ({ '--paper-texture': `url(${paperFiberWarm})` }))

function close() {
  emit('update:visible', false)
}

function onRegenerate() {
  emit('regenerate')
}

function itemKey(item: RedlineItem | HearingItem, index: number) {
  return item.id ?? `${index}-${item.title || ('time' in item ? item.time : '')}`
}

async function copyContent() {
  const redlineText = displayRedlines.value.length
    ? displayRedlines.value.map((item) => `• ${item.title || '未命名事项'}（${item.timeText || '时间未标注'}）`).join('\n')
    : '当前没有可展示的期限数据'
  const hearingText = displayHearings.value.length
    ? displayHearings.value.map((item) => `• ${item.time || '时间未标注'} ${item.title || '未命名排期'} ${item.court ? `｜${item.court}` : ''}`).join('\n')
    : '当前没有可展示的排期数据'
  const fullText = [
    `【${effectiveTitle.value} · ${effectiveDate.value}】`,
    `首要焦点：${displayNextAction.value.taskName || '暂无重点行动'}`,
    displayNextAction.value.description || '',
    `\n期限事项：\n${redlineText}`,
    `\n排期事项：\n${hearingText}`,
    `\n负荷：已排期 ${displayMetrics.value.committedHours ?? '0.0'}h，弹性余量 ${displayMetrics.value.freeSpaceHours ?? '0.0h'}`,
    displayContent.value ? `\n简报正文：\n${displayContent.value}` : '',
  ].filter(Boolean).join('\n')

  try {
    await navigator.clipboard.writeText(fullText)
    ElMessage.success('报告内容已复制')
  } catch {
    ElMessage.error('复制失败，请检查剪贴板权限')
  }
}

async function decodeImages(root: HTMLElement) {
  const images = Array.from(root.querySelectorAll('img'))
  await Promise.all(images.map(async (image) => {
    if (image.complete) return
    try {
      await image.decode()
    } catch {
      await new Promise<void>((resolve) => {
        image.addEventListener('load', () => resolve(), { once: true })
        image.addEventListener('error', () => resolve(), { once: true })
      })
    }
  }))
}

async function exportImage() {
  if (!reportSheetRef.value || exporting.value) return
  exporting.value = true
  let stage: HTMLDivElement | null = null

  try {
    await nextTick()
    await document.fonts?.ready
    const source = reportSheetRef.value
    const clone = source.cloneNode(true) as HTMLElement
    clone.classList.add('is-exporting')
    Object.assign(clone.style, {
      width: '780px',
      maxWidth: 'none',
      minHeight: '1040px',
      margin: '0',
      boxShadow: 'none',
      borderRadius: '2px',
    })

    stage = document.createElement('div')
    stage.setAttribute('aria-hidden', 'true')
    Object.assign(stage.style, {
      position: 'fixed',
      left: '-12000px',
      top: '0',
      width: '876px',
      padding: '48px',
      boxSizing: 'border-box',
      background: getComputedStyle(source).getPropertyValue('--export-mat').trim() || '#d8d2c7',
    })
    stage.appendChild(clone)
    document.body.appendChild(stage)

    await decodeImages(clone)
    const scale = clone.scrollHeight > 2600 ? 2 : 2.5
    const canvas = await html2canvas(stage, {
      scale,
      useCORS: true,
      backgroundColor: null,
      logging: false,
      imageTimeout: 15000,
    })
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
    if (!blob) throw new Error('PNG encoding failed')

    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    const safeDate = effectiveDate.value.replace(/[^0-9.\-年月日]/g, '') || 'report'
    link.download = `CASY_${props.type === 'daily' ? '早报' : '周报'}_${safeDate}.png`
    link.href = url
    link.click()
    window.setTimeout(() => URL.revokeObjectURL(url), 1000)
    ElMessage.success('高清长图已保存，内容未自动脱敏')
  } catch (error) {
    console.error('Briefing export failed', error)
    ElMessage.error('导出图片失败，请重试')
  } finally {
    stage?.remove()
    exporting.value = false
  }
}
</script>

<template>
  <transition name="brief-modal-fade">
    <div v-if="visible" class="brief-modal-backdrop" @click.self="close">
      <section class="brief-modal-shell" role="dialog" aria-modal="true" :aria-label="effectiveTitle">
        <button class="modal-close" type="button" aria-label="关闭报告" title="关闭" @click="close">
          <el-icon><Close /></el-icon>
        </button>

        <div class="report-viewport">
          <article
            ref="reportSheetRef"
            class="report-sheet"
            :class="`style-${activeStyle}`"
            :data-style="activeStyle"
            :data-family="activeMeta.family"
            :data-type="type"
            :style="reportStyleVars"
          >
            <div class="material-rule material-rule-top" aria-hidden="true"></div>
            <div class="report-corner report-corner-a" aria-hidden="true"></div>
            <div class="report-corner report-corner-b" aria-hidden="true"></div>
            <div class="skin-ornaments" aria-hidden="true">
              <span class="ornament-medallion">C</span>
              <span class="ornament-route">{{ reportTypeLabel }}</span>
              <span class="ornament-serial">{{ reportCode }}</span>
              <span class="ornament-pin ornament-pin-a"></span>
              <span class="ornament-pin ornament-pin-b"></span>
            </div>
            <div v-if="preview" class="sample-ribbon">SAMPLE / 仅作样式预览</div>

            <header class="report-header">
              <div class="brand-lockup">
                <span class="brand-mark">C</span>
                <span class="brand-copy">
                  <strong>CASY</strong>
                  <small>CASE INTELLIGENCE</small>
                </span>
              </div>
              <div class="report-index">
                <span>{{ reportTypeLabel }}</span>
                <strong>{{ reportCode }}</strong>
              </div>
            </header>

            <div class="title-block">
              <p class="style-kicker">{{ activeMeta.name }} / {{ activeMeta.label }}</p>
              <h1>{{ effectiveTitle }}</h1>
              <div class="title-meta">
                <span>{{ effectiveDate }}</span>
                <span>{{ reportTypeCn }}</span>
              </div>
            </div>

            <section class="focus-panel">
              <div class="section-number">01</div>
              <div class="focus-copy">
                <p class="section-label">首要焦点 / PRIMARY FOCUS</p>
                <h2>{{ displayNextAction.taskName || '暂无重点行动' }}</h2>
                <p>{{ displayNextAction.description || '当前没有可展示的行动说明。' }}</p>
                <div class="matter-reference">
                  <span>{{ displayNextAction.caseName || '未指定案件' }}</span>
                  <strong>{{ displayNextAction.caseCode || 'NO-CODE' }}</strong>
                </div>
              </div>
            </section>

            <section class="metric-grid" aria-label="简报指标">
              <div v-for="metric in metricCards" :key="metric.label" class="metric-cell">
                <span>{{ metric.label }}</span>
                <strong>{{ metric.value }}</strong>
                <small>{{ metric.note }}</small>
              </div>
            </section>

            <section v-if="displayContent" class="narrative-section">
              <div class="section-heading">
                <span class="section-number">02</span>
                <h2>简报摘要</h2>
              </div>
              <div class="markdown-body" v-html="renderBriefingMarkdown(displayContent)"></div>
            </section>

            <div class="detail-grid" :class="{ 'without-narrative': !displayContent }">
              <section class="detail-section redline-section">
                <div class="section-heading">
                  <span class="section-number">{{ displayContent ? '03' : '02' }}</span>
                  <div><p>DEADLINES</p><h2>期限与红线</h2></div>
                </div>
                <div v-if="displayRedlines.length" class="item-list">
                  <article v-for="(item, index) in displayRedlines" :key="itemKey(item, index)" class="report-item">
                    <div class="item-sequence">{{ String(index + 1).padStart(2, '0') }}</div>
                    <div class="item-copy">
                      <strong>{{ item.title || '未命名事项' }}</strong>
                      <span>{{ item.caseTitle || '未关联案件' }}</span>
                    </div>
                    <time>{{ item.timeText || '时间未标注' }}</time>
                  </article>
                </div>
                <p v-else class="honest-empty">当前没有可展示的期限数据</p>
              </section>

              <section class="detail-section schedule-section">
                <div class="section-heading">
                  <span class="section-number">{{ displayContent ? '04' : '03' }}</span>
                  <div><p>SCHEDULE</p><h2>排期与庭审</h2></div>
                </div>
                <div v-if="displayHearings.length" class="item-list">
                  <article v-for="(item, index) in displayHearings" :key="itemKey(item, index)" class="report-item">
                    <time class="schedule-time">{{ item.time || '待定' }}</time>
                    <div class="item-copy">
                      <strong>{{ item.title || '未命名排期' }}</strong>
                      <span>{{ item.court || item.timeRemaining || '地点未标注' }}</span>
                    </div>
                  </article>
                </div>
                <p v-else class="honest-empty">当前没有可展示的排期数据</p>
              </section>
            </div>

            <footer class="report-signoff">
              <div>
                <span>CASY BRIEFING</span>
                <small>生成时间 {{ generatedAt }} / 当前数据快照</small>
              </div>
              <img v-if="activeMeta.seal !== 'none'" class="material-seal" :src="sealSrc" alt="Casy 火漆章" />
              <strong>{{ reportCode }}</strong>
            </footer>
            <div class="material-rule material-rule-bottom" aria-hidden="true"></div>
          </article>
        </div>

        <footer class="modal-control-footer">
          <p>图片按当前显示内容生成，不自动脱敏</p>
          <div class="footer-actions">
            <button v-if="!preview" class="utility-action" type="button" :disabled="loading" title="重新生成" @click="onRegenerate">
              <el-icon><Refresh /></el-icon>
              <span>{{ loading ? '生成中' : '重新生成' }}</span>
            </button>
            <button v-if="!preview" class="utility-action" type="button" title="复制全文" @click="copyContent">
              <el-icon><CopyDocument /></el-icon>
              <span>复制</span>
            </button>
            <button class="export-action" type="button" :disabled="exporting" @click="exportImage">
              <el-icon><Download /></el-icon>
              <span>{{ exporting ? '正在导出' : '导出高清图片' }}</span>
            </button>
          </div>
        </footer>
      </section>
    </div>
  </transition>
</template>

<style scoped>
.brief-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: grid;
  place-items: center;
  padding: 24px;
  overflow: auto;
  background: rgba(15, 23, 42, 0.72);
  backdrop-filter: blur(16px) saturate(0.85);
  -webkit-backdrop-filter: blur(16px) saturate(0.85);
}

.brief-modal-shell {
  position: relative;
  width: min(840px, 100%);
  margin: auto;
}

.report-viewport {
  max-height: calc(100dvh - 126px);
  overflow: auto;
  border-radius: 3px 3px 0 0;
  box-shadow: 0 34px 90px rgba(2, 6, 23, 0.42);
}

.report-sheet {
  --paper: #f8f5ed;
  --ink: #17191d;
  --muted: #65635e;
  --accent: #8f252b;
  --accent-contrast: #fff9f2;
  --line: rgba(23, 25, 29, 0.24);
  --export-mat: #d8d2c7;
  --texture-opacity: 0.28;
  --display-font: Georgia, 'Songti SC', 'STSong', serif;
  --body-font: Inter, 'PingFang SC', 'Microsoft YaHei', sans-serif;
  --mono-font: 'SFMono-Regular', Consolas, 'Liberation Mono', monospace;
  position: relative;
  isolation: isolate;
  min-height: 960px;
  padding: 54px 58px 38px;
  overflow: hidden;
  color: var(--ink);
  background: var(--paper);
  font-family: var(--body-font);
  -webkit-font-smoothing: antialiased;
}

.report-sheet::before {
  position: absolute;
  z-index: -2;
  inset: 0;
  content: '';
  pointer-events: none;
  background-image: var(--paper-texture);
  background-size: 760px 760px;
  opacity: var(--texture-opacity);
  mix-blend-mode: multiply;
}

.report-sheet::after {
  position: absolute;
  z-index: -1;
  inset: 0;
  content: '';
  pointer-events: none;
  opacity: 0.8;
}

.report-header, .brand-lockup, .report-index, .title-meta, .matter-reference,
.section-heading, .report-item, .report-signoff, .modal-control-footer, .footer-actions {
  display: flex;
  align-items: center;
}

.report-header {
  justify-content: space-between;
  gap: 24px;
  padding-bottom: 20px;
  border-bottom: 1px solid var(--line);
}

.brand-lockup { gap: 10px; }
.brand-mark {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  color: var(--accent-contrast);
  background: var(--accent);
  border: 1px solid var(--accent);
  font: 700 21px/1 var(--display-font);
}
.brand-copy { display: grid; gap: 1px; }
.brand-copy strong { font: 800 13px/1 var(--body-font); letter-spacing: 0.12em; }
.brand-copy small { color: var(--muted); font: 600 8px/1.2 var(--mono-font); letter-spacing: 0.13em; }

.report-index { align-items: flex-end; flex-direction: column; gap: 3px; text-align: right; }
.report-index span, .report-index strong { font-family: var(--mono-font); letter-spacing: 0.08em; }
.report-index span { color: var(--accent); font-size: 9px; font-weight: 800; }
.report-index strong { color: var(--muted); font-size: 9px; font-weight: 600; }

.title-block { padding: 62px 0 38px; }
.style-kicker { margin: 0 0 14px; color: var(--accent); font: 800 10px/1.3 var(--mono-font); letter-spacing: 0.12em; text-transform: uppercase; }
.title-block h1 { max-width: 650px; margin: 0; color: var(--ink); font: 600 clamp(42px, 7vw, 72px)/0.98 var(--display-font); letter-spacing: -0.045em; }
.title-meta { justify-content: space-between; gap: 24px; margin-top: 26px; color: var(--muted); font: 700 10px/1.3 var(--mono-font); letter-spacing: 0.08em; }

.focus-panel {
  display: grid;
  grid-template-columns: 54px 1fr;
  gap: 24px;
  padding: 28px 0 30px;
  border-top: 3px solid var(--ink);
  border-bottom: 1px solid var(--line);
}
.section-number { color: var(--accent); font: 800 11px/1 var(--mono-font); letter-spacing: 0.05em; }
.section-label { margin: 0 0 9px; color: var(--muted); font: 800 9px/1.2 var(--mono-font); letter-spacing: 0.11em; }
.focus-copy h2 { margin: 0 0 10px; font: 600 28px/1.2 var(--display-font); letter-spacing: -0.02em; }
.focus-copy > p:not(.section-label) { max-width: 620px; margin: 0; color: var(--muted); font-size: 13px; line-height: 1.75; }
.matter-reference { gap: 8px; margin-top: 17px; }
.matter-reference span, .matter-reference strong { padding: 4px 7px; border: 1px solid var(--line); font: 700 9px/1.2 var(--mono-font); letter-spacing: 0.04em; }
.matter-reference strong { color: var(--accent); border-color: var(--accent); }

.metric-grid { display: grid; grid-template-columns: repeat(4, 1fr); margin: 28px 0 40px; border-block: 1px solid var(--line); }
.metric-cell { display: grid; align-content: start; min-height: 116px; padding: 18px 18px 16px; border-right: 1px solid var(--line); }
.metric-cell:last-child { border-right: 0; }
.metric-cell span { color: var(--muted); font: 800 9px/1.2 var(--mono-font); letter-spacing: 0.07em; }
.metric-cell strong { margin: 9px 0 5px; font: 600 32px/1 var(--display-font); font-variant-numeric: tabular-nums; }
.metric-cell small { color: var(--muted); font-size: 9px; line-height: 1.35; }

.narrative-section { margin-bottom: 42px; }
.section-heading { align-items: flex-start; gap: 18px; margin-bottom: 17px; }
.section-heading > div { display: grid; gap: 3px; }
.section-heading p { margin: 0; color: var(--accent); font: 800 8px/1 var(--mono-font); letter-spacing: 0.12em; }
.section-heading h2 { margin: 0; font: 600 19px/1.1 var(--display-font); }
.markdown-body { padding-left: 35px; color: var(--muted); font: 13px/1.75 var(--body-font); }
.markdown-body :deep(.brief-p) { margin: 0 0 10px; }
.markdown-body :deep(.brief-ul) { margin: 8px 0 12px; padding-left: 20px; }
.markdown-body :deep(.brief-h3), .markdown-body :deep(.brief-h4), .markdown-body :deep(.brief-h5) { margin: 18px 0 8px; color: var(--ink); font-family: var(--display-font); }

.detail-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 34px; padding-top: 34px; border-top: 1px solid var(--line); }
.detail-grid.without-narrative { margin-top: 4px; }
.detail-section { min-width: 0; }
.item-list { display: grid; }
.report-item { align-items: flex-start; gap: 11px; padding: 13px 0; border-top: 1px solid var(--line); }
.report-item:first-child { border-top-color: var(--ink); }
.item-sequence { flex: 0 0 auto; padding-top: 2px; color: var(--accent); font: 700 9px/1 var(--mono-font); }
.item-copy { display: grid; min-width: 0; flex: 1; gap: 4px; }
.item-copy strong { font: 600 12px/1.45 var(--body-font); }
.item-copy span { overflow-wrap: anywhere; color: var(--muted); font-size: 10px; line-height: 1.35; }
.report-item > time { flex: 0 0 auto; max-width: 92px; color: var(--accent); font: 800 9px/1.35 var(--mono-font); text-align: right; }
.report-item > .schedule-time { min-width: 44px; color: var(--ink); font-size: 11px; text-align: left; }
.honest-empty { margin: 0; padding: 18px 0; border-block: 1px solid var(--line); color: var(--muted); font-size: 11px; line-height: 1.5; }

.report-signoff { justify-content: space-between; min-height: 92px; gap: 20px; margin-top: 54px; padding-top: 22px; border-top: 3px solid var(--ink); }
.report-signoff > div { display: grid; gap: 4px; }
.report-signoff span { font: 800 10px/1 var(--mono-font); letter-spacing: 0.12em; }
.report-signoff small { color: var(--muted); font: 600 9px/1.4 var(--mono-font); }
.report-signoff > strong { margin-left: auto; color: var(--muted); font: 700 9px/1.2 var(--mono-font); letter-spacing: 0.05em; }
.material-seal { width: 90px; height: 90px; margin: -24px 0 -16px; object-fit: contain; transform: rotate(-8deg); filter: drop-shadow(0 6px 8px rgba(30, 16, 8, 0.2)); }

.sample-ribbon { position: absolute; z-index: 5; top: 16px; left: -41px; width: 160px; padding: 5px 0; color: var(--accent-contrast); background: var(--accent); transform: rotate(-34deg); font: 800 8px/1 var(--mono-font); letter-spacing: 0.05em; text-align: center; }
.material-rule, .report-corner { position: absolute; pointer-events: none; }
.material-rule { right: 58px; left: 58px; height: 1px; background: var(--line); }
.material-rule-top { top: 14px; }
.material-rule-bottom { bottom: 14px; }
.report-corner { width: 17px; height: 17px; border-color: var(--accent); opacity: 0.66; }
.report-corner-a { top: 24px; left: 24px; border-top: 1px solid; border-left: 1px solid; }
.report-corner-b { right: 24px; bottom: 24px; border-right: 1px solid; border-bottom: 1px solid; }

.skin-ornaments, .skin-ornaments span { position: absolute; pointer-events: none; }
.skin-ornaments { z-index: -1; inset: 0; overflow: hidden; }
.skin-ornaments span { display: none; }
.ornament-medallion { place-items: center; color: var(--accent); border: 1px solid var(--accent); border-radius: 50%; font: 500 24px/1 var(--display-font); }
.ornament-route, .ornament-serial { color: var(--muted); font: 700 8px/1 var(--mono-font); letter-spacing: 0.12em; text-transform: uppercase; }
.ornament-pin { width: 9px; height: 9px; border: 1px solid var(--accent); border-radius: 50%; }

.modal-close { position: absolute; z-index: 10; top: 14px; right: -52px; display: grid; width: 38px; height: 38px; place-items: center; color: #f8fafc; background: rgba(15, 23, 42, 0.58); border: 1px solid rgba(255, 255, 255, 0.18); border-radius: 50%; cursor: pointer; backdrop-filter: blur(8px); }
.modal-close:hover { background: rgba(15, 23, 42, 0.86); }
.modal-control-footer { justify-content: space-between; gap: 16px; padding: 12px 14px; color: var(--c-text-secondary); background: var(--c-bg-card); border-top: 1px solid var(--c-border); border-radius: 0 0 var(--c-radius-lg) var(--c-radius-lg); box-shadow: 0 18px 50px rgba(2, 6, 23, 0.22); }
.modal-control-footer p { margin: 0; font-size: 10.5px; }
.footer-actions { gap: 6px; }
.utility-action, .export-action { display: inline-flex; align-items: center; gap: 5px; min-height: 32px; padding: 0 10px; border-radius: var(--c-radius-md); font-size: 11px; font-weight: 700; cursor: pointer; }
.utility-action { color: var(--c-text); background: transparent; border: 1px solid transparent; }
.utility-action:hover { background: var(--c-bg-hover); border-color: var(--c-border); }
.export-action { padding-inline: 14px; color: var(--c-primary-contrast); background: var(--c-primary); border: 1px solid var(--c-primary); }
.export-action:hover { filter: brightness(1.06); }
.utility-action:disabled, .export-action:disabled { cursor: not-allowed; opacity: 0.55; }

/* Editorial */
.style-gazette { --paper: #f5f1e7; --ink: #171512; --muted: #666057; --accent: #82252a; --export-mat: #bdb4a4; }
.style-gazette::after { background: repeating-linear-gradient(90deg, transparent 0 31px, rgba(0, 0, 0, 0.022) 31px 32px); }
.style-gazette .report-header { padding-top: 10px; border-top: 6px double var(--ink); border-bottom: 2px solid var(--ink); }
.style-gazette .title-block { padding-block: 34px 30px; text-align: center; border-bottom: 1px solid var(--ink); }
.style-gazette .title-block h1 { margin-inline: auto; font-weight: 800; letter-spacing: -0.055em; text-transform: uppercase; }
.style-gazette .title-meta { justify-content: center; gap: 32px; }
.style-gazette .focus-copy > p:not(.section-label)::first-letter { float: left; margin: 5px 6px 0 0; color: var(--accent); font: 700 34px/0.75 var(--display-font); }
.style-gazette .detail-grid { gap: 24px; border-top: 4px double var(--ink); }

.style-vogue { --paper: #fbfaf7; --ink: #121212; --muted: #6d6962; --accent: #111; --export-mat: #dedbd5; --texture-opacity: 0.08; }
.style-vogue .brand-mark { color: var(--ink); background: transparent; border: 1px solid var(--ink); border-radius: 50%; }
.style-vogue .report-header { border-bottom: 0; }
.style-vogue .title-block { padding-block: 102px 72px; text-align: center; }
.style-vogue .style-kicker { margin-bottom: 24px; letter-spacing: 0.28em; }
.style-vogue .title-block h1 { max-width: 700px; margin-inline: auto; font-size: clamp(56px, 9vw, 94px); font-weight: 400; line-height: 0.86; }
.style-vogue .title-meta { justify-content: center; margin-top: 38px; }
.style-vogue .title-meta span + span::before { content: '/'; margin-right: 24px; }
.style-vogue .focus-panel { padding-inline: 50px; border-top-width: 1px; }
.style-vogue .metric-grid { margin-inline: 50px; }
.style-vogue .metric-cell { min-height: 100px; text-align: center; }

.style-chronicle { --paper: #f2eee5; --ink: #1e2933; --muted: #6d7277; --accent: #36546c; --export-mat: #aeb7be; }
.style-chronicle::after { background: linear-gradient(90deg, transparent 48px, rgba(54, 84, 108, 0.18) 49px, rgba(54, 84, 108, 0.18) 50px, transparent 51px); }
.style-chronicle .ornament-serial { right: -44px; top: 340px; display: block; transform: rotate(90deg); }
.style-chronicle .title-block { padding-left: 44px; }
.style-chronicle .title-block h1 { font-style: italic; font-weight: 500; }
.style-chronicle .focus-panel { grid-template-columns: 70px 1fr; border-top: 1px solid var(--accent); }
.style-chronicle .section-number { display: grid; width: 34px; height: 34px; place-items: center; color: var(--paper); background: var(--accent); border-radius: 50%; }
.style-chronicle .detail-grid { position: relative; padding-left: 44px; }
.style-chronicle .detail-grid::before { position: absolute; top: 28px; bottom: 0; left: 16px; width: 1px; content: ''; background: var(--accent); opacity: 0.45; }

/* Documents */
.style-typewriter, .style-telegraph, .style-dossier, .style-classified-file {
  --display-font: 'Courier New', Courier, monospace;
  --body-font: 'Courier New', Courier, monospace;
}
.style-typewriter { --paper: #f7f6f2; --ink: #151515; --muted: #595959; --accent: #151515; --export-mat: #c9c9c5; --texture-opacity: 0.18; padding-left: 86px; }
.style-typewriter::after { background: repeating-linear-gradient(0deg, transparent 0 27px, rgba(0, 0, 0, 0.045) 27px 28px), linear-gradient(90deg, transparent 63px, rgba(166, 57, 57, 0.32) 64px, transparent 65px); }
.style-typewriter .ornament-pin { left: 25px; display: block; width: 15px; height: 15px; border: 2px solid #9e9b93; box-shadow: inset 0 0 0 3px var(--paper); }
.style-typewriter .ornament-pin-a { top: 200px; }
.style-typewriter .ornament-pin-b { top: 520px; }
.style-typewriter .report-header { border-bottom-style: dashed; }
.style-typewriter .title-block h1 { font-size: clamp(40px, 6vw, 62px); letter-spacing: -0.04em; text-transform: uppercase; }
.style-typewriter .style-kicker::before { content: 'REF: '; }
.style-typewriter .focus-panel, .style-typewriter .detail-grid { border-top-style: dashed; }
.style-typewriter .report-item { border-top-style: dotted; }

.style-telegraph { --paper: #e7d9ad; --ink: #372d22; --muted: #6d5d49; --accent: #a03e28; --export-mat: #9b876c; --texture-opacity: 0.42; padding-top: 76px; }
.style-telegraph::after { background: radial-gradient(circle at 9px 12px, var(--export-mat) 0 3px, transparent 3.5px) 0 0 / 18px 24px repeat-x; opacity: 0.48; }
.style-telegraph .ornament-route { top: 28px; left: 58px; display: block; padding: 8px 12px; color: var(--accent); border: 2px solid var(--accent); transform: rotate(-1deg); }
.style-telegraph .report-header { border-block: 1px dashed var(--ink); padding-block: 14px; }
.style-telegraph .title-block { padding-block: 36px 28px; }
.style-telegraph .title-block h1 { font-size: clamp(44px, 7vw, 68px); line-height: 0.92; text-transform: uppercase; }
.style-telegraph .focus-panel { padding: 24px; border: 2px dashed var(--ink); }
.style-telegraph .metric-grid { border: 2px dashed rgba(55, 45, 34, 0.55); }
.style-telegraph .report-item { border-top-style: dashed; }

.style-dossier { --paper: #d7bd91; --ink: #30251b; --muted: #6f5b48; --accent: #8f252b; --export-mat: #6b4a36; --texture-opacity: 0.4; padding-top: 80px; }
.style-dossier::after { inset: 22px; border: 1px solid rgba(48, 37, 27, 0.38); box-shadow: inset 8px 0 rgba(78, 53, 33, 0.1); }
.style-dossier .ornament-route { top: 22px; right: 58px; display: block; min-width: 180px; padding: 12px 18px 18px; color: #f3e5ca; background: #6f4d32; text-align: center; clip-path: polygon(0 0, 100% 0, 92% 100%, 0 100%); }
.style-dossier .ornament-serial { top: 92px; right: 58px; display: block; color: var(--accent); border-bottom: 2px solid var(--accent); }
.style-dossier .report-header::after { content: 'FILE COPY'; margin-left: 12px; padding: 5px 8px; color: var(--accent); border: 2px solid var(--accent); transform: rotate(-3deg); font: 800 9px/1 var(--mono-font); }
.style-dossier .title-block { padding-block: 44px 28px; }
.style-dossier .focus-panel { padding: 26px; background: rgba(255, 245, 222, 0.22); border: 1px solid rgba(48, 37, 27, 0.4); }
.style-dossier .report-signoff { border-top-style: double; }

.style-classified-file { --paper: #d6b676; --ink: #201b14; --muted: #655239; --accent: #a1121a; --export-mat: #564536; --texture-opacity: 0.45; padding-top: 78px; }
.style-classified-file::after { background: linear-gradient(112deg, transparent 0 70%, rgba(48, 34, 18, 0.055) 70% 71%, transparent 71%), repeating-linear-gradient(0deg, transparent 0 39px, rgba(72, 48, 25, 0.035) 39px 40px); }
.style-classified-file .ornament-route { top: 0; left: 0; display: block; width: 100%; padding: 15px 0; color: #f5d8c0; background: var(--accent); text-align: center; letter-spacing: 0.42em; }
.style-classified-file .style-kicker { width: max-content; padding: 6px 9px; border: 3px double var(--accent); transform: rotate(-2deg); }
.style-classified-file .title-block h1 { max-width: 560px; font-weight: 800; text-transform: uppercase; }
.style-classified-file .metric-grid { border: 2px solid var(--ink); }
.style-classified-file .metric-cell { border-right-width: 2px; }
.style-classified-file .report-signoff strong { padding: 6px 9px; color: var(--accent); border: 2px solid var(--accent); transform: rotate(2deg); }

/* Grid and action */
.style-swiss-grid { --paper: #f7f7f5; --ink: #050505; --muted: #5d5d5d; --accent: #c62f24; --export-mat: #c5c5c0; --texture-opacity: 0.05; --display-font: Arial, Helvetica, sans-serif; }
.style-swiss-grid::after { background: repeating-linear-gradient(90deg, transparent 0 107px, rgba(0, 0, 0, 0.075) 107px 108px); }
.style-swiss-grid .ornament-medallion { top: 86px; right: 42px; display: grid; width: 138px; height: 138px; color: var(--accent); border: 12px solid var(--accent); border-radius: 0; font: 900 98px/1 Arial, sans-serif; opacity: 0.12; }
.style-swiss-grid .report-header { border-bottom: 6px solid var(--ink); }
.style-swiss-grid .title-block { padding-block: 54px 44px; }
.style-swiss-grid .title-block h1 { max-width: 520px; font-size: clamp(52px, 8vw, 84px); font-weight: 900; line-height: 0.86; text-transform: uppercase; }
.style-swiss-grid .style-kicker { display: inline-block; padding: 5px 8px; color: white; background: var(--accent); }
.style-swiss-grid .focus-panel { grid-template-columns: 88px 1fr; border-top-width: 10px; }
.style-swiss-grid .section-number { color: var(--ink); font-size: 30px; }
.style-swiss-grid .metric-grid { border: 4px solid var(--ink); }
.style-swiss-grid .metric-cell { border-right: 4px solid var(--ink); }

.style-focus-matrix { --paper: #f6ff56; --ink: #090909; --muted: #323232; --accent: #ed3e98; --accent-contrast: #090909; --export-mat: #6448d8; --texture-opacity: 0.03; --display-font: Arial, Helvetica, sans-serif; border: 8px solid var(--ink); }
.style-focus-matrix::after { background: linear-gradient(135deg, transparent 0 78%, rgba(100, 72, 216, 0.23) 78%); }
.style-focus-matrix .ornament-route { top: 78px; right: -35px; display: block; padding: 9px 48px; color: #fff; background: #6448d8; transform: rotate(36deg); }
.style-focus-matrix .report-header { border-bottom: 4px solid var(--ink); }
.style-focus-matrix .brand-mark { box-shadow: 4px 4px 0 #6448d8; }
.style-focus-matrix .title-block h1 { font: 900 clamp(48px, 8vw, 82px)/0.84 Arial, sans-serif; text-transform: uppercase; }
.style-focus-matrix .focus-panel, .style-focus-matrix .metric-grid { box-shadow: 6px 6px 0 var(--ink); }
.style-focus-matrix .focus-panel { padding: 24px; background: #fff; border: 3px solid var(--ink); transform: rotate(-0.4deg); }
.style-focus-matrix .metric-grid { background: #6ae6d3; border: 3px solid var(--ink); transform: rotate(0.3deg); }
.style-focus-matrix .detail-section:first-child { padding: 20px; background: #ff98ca; border: 3px solid var(--ink); }
.style-focus-matrix .detail-section:last-child { padding: 20px; background: #fff; border: 3px solid var(--ink); }

.style-action-board { --paper: #ece7dc; --ink: #1e2933; --muted: #66717b; --accent: #be6b17; --export-mat: #8c8172; --texture-opacity: 0.2; }
.style-action-board::after { background: linear-gradient(90deg, rgba(93, 69, 36, 0.035) 1px, transparent 1px), linear-gradient(rgba(93, 69, 36, 0.035) 1px, transparent 1px); background-size: 24px 24px; }
.style-action-board .ornament-route { top: 160px; left: 46%; display: block; width: 120px; height: 22px; overflow: hidden; color: transparent; background: rgba(224, 193, 133, 0.78); transform: rotate(3deg); }
.style-action-board .ornament-pin { display: block; width: 12px; height: 12px; background: #b84c3c; border: 2px solid rgba(89, 30, 24, 0.5); box-shadow: 0 3px 6px rgba(57, 43, 20, 0.25); }
.style-action-board .ornament-pin-a { top: 190px; left: 51%; }
.style-action-board .ornament-pin-b { right: 80px; bottom: 330px; background: #3f7187; }
.style-action-board .focus-panel { padding: 30px; background: #f6df83; border: 0; box-shadow: 0 10px 22px rgba(57, 43, 20, 0.16); transform: rotate(-0.6deg); }
.style-action-board .metric-grid { gap: 9px; border: 0; }
.style-action-board .metric-cell { background: rgba(255, 255, 255, 0.62); border: 0; box-shadow: 0 5px 12px rgba(57, 43, 20, 0.08); }
.style-action-board .detail-section { padding: 22px; background: rgba(255, 255, 255, 0.58); box-shadow: 0 8px 20px rgba(57, 43, 20, 0.11); }
.style-action-board .detail-section:first-child { transform: rotate(0.35deg); }
.style-action-board .detail-section:last-child { transform: rotate(-0.35deg); }

/* Atmospheric */
.style-narrative-air { --paper: #f6fbf9; --ink: #183a35; --muted: #60766f; --accent: #318875; --export-mat: #afc7c0; --texture-opacity: 0.1; }
.style-narrative-air::after { background: radial-gradient(circle at 94% 8%, rgba(120, 194, 175, 0.22), transparent 29%), radial-gradient(circle at 2% 58%, rgba(216, 185, 121, 0.12), transparent 25%); }
.style-narrative-air .ornament-medallion { top: 118px; right: 66px; display: grid; width: 90px; height: 90px; border: 1px solid rgba(49, 136, 117, 0.28); font-size: 42px; opacity: 0.32; }
.style-narrative-air .report-header { border-bottom-color: rgba(49, 136, 117, 0.18); }
.style-narrative-air .title-block { max-width: 590px; padding-block: 94px 64px; }
.style-narrative-air .title-block h1 { font-weight: 400; line-height: 1.05; }
.style-narrative-air .focus-panel { padding: 34px; background: rgba(178, 222, 210, 0.27); border: 1px solid rgba(49, 136, 117, 0.25); border-radius: 22px; }
.style-narrative-air .metric-grid { gap: 9px; border: 0; }
.style-narrative-air .metric-cell { border: 0; border-radius: 12px; background: rgba(255, 255, 255, 0.62); }
.style-narrative-air .detail-grid { gap: 46px; border-top-color: rgba(49, 136, 117, 0.18); }

.style-glassmorphism { --paper: #101827; --ink: #f3f7fb; --muted: #9eb0c5; --accent: #75e6da; --accent-contrast: #08121d; --line: rgba(211, 236, 255, 0.2); --export-mat: #07101d; --texture-opacity: 0.04; }
.style-glassmorphism::after { background: radial-gradient(circle at 12% 18%, rgba(36, 166, 185, 0.34), transparent 30%), radial-gradient(circle at 88% 82%, rgba(88, 101, 242, 0.3), transparent 36%); }
.style-glassmorphism .ornament-medallion { right: -46px; top: 118px; display: grid; width: 210px; height: 210px; color: rgba(117, 230, 218, 0.28); border-color: rgba(117, 230, 218, 0.22); font-size: 88px; }
.style-glassmorphism .report-header { padding: 14px 16px; background: rgba(255, 255, 255, 0.035); border: 1px solid var(--line); }
.style-glassmorphism .title-block { padding: 70px 20px 44px; }
.style-glassmorphism .title-block h1 { text-shadow: 0 0 32px rgba(117, 230, 218, 0.18); }
.style-glassmorphism .focus-panel, .style-glassmorphism .metric-cell, .style-glassmorphism .detail-section { background: rgba(255, 255, 255, 0.055); border: 1px solid var(--line); }
.style-glassmorphism .focus-panel { padding: 28px; }
.style-glassmorphism .metric-grid { gap: 8px; border: 0; }
.style-glassmorphism .metric-cell { border-radius: 2px; }
.style-glassmorphism .detail-grid { gap: 12px; border: 0; }
.style-glassmorphism .detail-section { padding: 22px; }

.style-milestones { --paper: #fff8fb; --ink: #3a2240; --muted: #7f667f; --accent: #b94080; --export-mat: #d0b4c7; --texture-opacity: 0.06; }
.style-milestones::after { background: radial-gradient(circle at 92% 6%, rgba(235, 115, 172, 0.25), transparent 28%), radial-gradient(circle at 0 70%, rgba(112, 208, 210, 0.18), transparent 32%); }
.style-milestones .ornament-route { top: 112px; right: 58px; display: block; width: 110px; height: 22px; overflow: hidden; color: transparent; border-block: 1px solid rgba(185, 64, 128, 0.35); transform: rotate(-7deg); }
.style-milestones .title-block h1 { font-weight: 400; }
.style-milestones .focus-panel { padding: 28px; border: 1px solid rgba(185, 64, 128, 0.22); border-radius: 30px 8px 30px 8px; }
.style-milestones .metric-grid { gap: 10px; border: 0; }
.style-milestones .metric-cell { border: 0; border-radius: 18px 5px 18px 5px; background: rgba(255, 255, 255, 0.66); }
.style-milestones .metric-cell strong { color: var(--accent); }
.style-milestones .detail-section { position: relative; padding-left: 27px; }
.style-milestones .detail-section::before { position: absolute; top: 4px; bottom: 4px; left: 6px; width: 2px; content: ''; background: linear-gradient(var(--accent), #70bfc2); opacity: 0.5; }
.style-milestones .report-item::before { width: 7px; height: 7px; margin: 4px 0 0 -25px; content: ''; background: var(--paper); border: 2px solid var(--accent); border-radius: 50%; }

/* Executive */
.style-executive { --paper: #f6f1e5; --ink: #172335; --muted: #69717c; --accent: #a97728; --accent-contrast: #fff9eb; --export-mat: #172335; --texture-opacity: 0.22; padding-left: 86px; }
.style-executive::after { inset: 14px; border: 1px solid rgba(169, 119, 40, 0.46); box-shadow: inset 18px 0 #172335; }
.style-executive .ornament-serial { top: 280px; left: 25px; display: block; color: #f3ead7; transform: rotate(-90deg); transform-origin: left top; }
.style-executive .report-header { border-bottom: 2px solid var(--accent); }
.style-executive .title-block { padding-block: 52px 38px; }
.style-executive .title-block h1 { max-width: 570px; font-weight: 500; }
.style-executive .focus-panel { padding: 28px; border: 1px solid rgba(169, 119, 40, 0.45); }
.style-executive .metric-grid { background: #172335; border: 0; }
.style-executive .metric-cell { color: #f6f1e5; border-color: rgba(246, 241, 229, 0.18); }
.style-executive .metric-cell span, .style-executive .metric-cell small { color: #c2bdaF; }
.style-executive .metric-cell strong { color: #d9b66c; }

.style-ledger { --paper: #111418; --ink: #f4e8c6; --muted: #aea487; --accent: #d4a84e; --accent-contrast: #111418; --line: rgba(212, 168, 78, 0.28); --export-mat: #050607; --texture-opacity: 0.04; }
.style-ledger::after { inset: 17px; border: 3px double rgba(212, 168, 78, 0.5); background: repeating-linear-gradient(0deg, transparent 0 31px, rgba(212, 168, 78, 0.025) 31px 32px); }
.style-ledger .ornament-medallion { top: 116px; right: 62px; display: grid; width: 74px; height: 74px; border: 3px double var(--accent); border-radius: 2px; font-size: 38px; opacity: 0.68; }
.style-ledger .report-header { border-bottom: 3px double var(--accent); }
.style-ledger .title-block h1 { font-weight: 400; letter-spacing: -0.015em; }
.style-ledger .focus-panel { border-block: 3px double var(--accent); }
.style-ledger .metric-grid { border: 1px solid var(--accent); }
.style-ledger .metric-cell { min-height: 104px; border-color: var(--accent); }
.style-ledger .metric-cell strong { color: var(--accent); }
.style-ledger .detail-grid { border-top: 3px double var(--accent); }
.style-ledger .report-item { display: grid; grid-template-columns: 28px 1fr auto; border-top-color: rgba(212, 168, 78, 0.44); }

.style-partner-brief { --paper: #f8f2e4; --ink: #392919; --muted: #776856; --accent: #8e6335; --export-mat: #5b4631; --texture-opacity: 0.3; padding-inline: 82px; }
.style-partner-brief .ornament-route { top: 46px; right: 82px; display: block; padding-bottom: 7px; color: var(--accent); border-bottom: 1px solid var(--accent); }
.style-partner-brief .brand-mark { color: var(--accent); background: transparent; border: 0; font-size: 34px; }
.style-partner-brief .report-header { align-items: flex-end; border-bottom: 1px solid var(--accent); }
.style-partner-brief .title-block { padding-block: 88px 58px; }
.style-partner-brief .title-block h1 { font-style: italic; font-weight: 400; line-height: 1.05; }
.style-partner-brief .focus-panel { display: block; padding: 34px 0; border-top: 1px solid var(--line); }
.style-partner-brief .focus-panel > .section-number { display: none; }
.style-partner-brief .focus-copy > p:not(.section-label) { font-family: var(--display-font); font-size: 15px; line-height: 1.9; }
.style-partner-brief .metric-grid { border: 0; border-left: 1px solid var(--accent); }
.style-partner-brief .report-signoff { align-items: flex-end; border-top: 1px solid var(--accent); }
.style-partner-brief .material-seal { margin-left: auto; }

/* Heritage */
.style-magic-prophet { --paper: #dfc994; --ink: #302517; --muted: #6f5e44; --accent: #6f271f; --export-mat: #57483a; --texture-opacity: 0.48; }
.style-magic-prophet::after { background: radial-gradient(ellipse at center, transparent 52%, rgba(72, 47, 27, 0.15)), repeating-linear-gradient(90deg, transparent 0 190px, rgba(65, 43, 25, 0.055) 190px 191px); }
.style-magic-prophet .ornament-medallion { top: 104px; left: 50%; display: grid; width: 68px; height: 68px; color: var(--accent); border: 3px double var(--accent); transform: translateX(-50%); opacity: 0.7; }
.style-magic-prophet .report-header { border-block: 4px double var(--ink); padding-block: 10px; }
.style-magic-prophet .title-block { padding-top: 92px; text-align: center; }
.style-magic-prophet .title-block h1 { margin-inline: auto; font-weight: 800; text-align: center; text-transform: uppercase; }
.style-magic-prophet .style-kicker, .style-magic-prophet .title-meta { justify-content: center; text-align: center; }
.style-magic-prophet .focus-panel { border-block: 4px double var(--ink); }
.style-magic-prophet .detail-grid { gap: 22px; border-top: 4px double var(--ink); }

.style-bulletin { --paper: #d9bb79; --ink: #3a2417; --muted: #6d513c; --accent: #8d281f; --export-mat: #5d4430; --texture-opacity: 0.5; border: 12px solid #573723; outline: 2px solid #b98b57; outline-offset: -22px; }
.style-bulletin::after { inset: 30px; border: 1px solid rgba(87, 55, 35, 0.48); }
.style-bulletin .ornament-route { top: 30px; left: 50%; display: block; min-width: 210px; padding: 8px 24px; color: #eed7ae; background: #573723; transform: translateX(-50%); text-align: center; }
.style-bulletin .report-header { padding-top: 30px; border-bottom: 4px solid var(--ink); }
.style-bulletin .title-block { padding-block: 48px 34px; text-align: center; }
.style-bulletin .title-block h1 { margin-inline: auto; font-size: clamp(52px, 8vw, 84px); font-weight: 900; line-height: 0.88; text-transform: uppercase; }
.style-bulletin .focus-panel { padding: 28px; border: 4px solid var(--ink); }
.style-bulletin .metric-grid { border-block: 4px solid var(--ink); }
.style-bulletin .report-signoff { border-top: 4px solid var(--ink); }

.style-scroll { --paper: #f7f0df; --ink: #24231f; --muted: #706c63; --accent: #a12c27; --export-mat: #8c352f; --texture-opacity: 0.34; --display-font: 'STKaiti', 'KaiTi', 'Kaiti SC', serif; border-block: 16px solid #8f252b; padding-inline: 82px; }
.style-scroll::after { background: linear-gradient(90deg, rgba(161, 44, 39, 0.1) 0 18px, transparent 18px calc(100% - 18px), rgba(161, 44, 39, 0.1) calc(100% - 18px)); }
.style-scroll .ornament-route { top: 124px; right: 28px; display: block; color: var(--accent); writing-mode: vertical-rl; letter-spacing: 0.28em; }
.style-scroll .report-header { border-bottom: 1px solid var(--accent); }
.style-scroll .title-block { text-align: center; }
.style-scroll .title-block h1 { margin-inline: auto; font-weight: 400; letter-spacing: 0.1em; }
.style-scroll .brand-mark { border-radius: 4px; }
.style-scroll .focus-panel { border-block-color: var(--accent); }
.style-scroll .section-number { width: 24px; height: 24px; padding-top: 5px; color: white; background: var(--accent); text-align: center; }
.style-scroll .report-signoff { border-top: 1px solid var(--accent); }

.style-hogwarts-letter { --paper: #eee2bd; --ink: #183f35; --muted: #5b6e65; --accent: #9a252c; --export-mat: #405b52; --texture-opacity: 0.42; padding-inline: 84px; }
.style-hogwarts-letter::after { background: linear-gradient(0deg, transparent 49%, rgba(24, 63, 53, 0.08) 49.1%, transparent 49.3%), linear-gradient(90deg, transparent 49%, rgba(24, 63, 53, 0.08) 49.1%, transparent 49.3%); }
.style-hogwarts-letter .ornament-medallion { top: 112px; left: 50%; display: grid; width: 82px; height: 82px; color: #174b3c; border: 3px double #174b3c; transform: translateX(-50%); }
.style-hogwarts-letter .report-header { border-bottom: 2px solid #174b3c; }
.style-hogwarts-letter .title-block { padding-top: 112px; text-align: center; }
.style-hogwarts-letter .title-block h1 { margin-inline: auto; color: #174b3c; font-style: italic; font-weight: 400; }
.style-hogwarts-letter .title-meta { justify-content: center; }
.style-hogwarts-letter .focus-panel { display: block; padding-block: 36px; border-block: 1px solid #174b3c; }
.style-hogwarts-letter .focus-panel > .section-number { display: none; }
.style-hogwarts-letter .detail-grid { border-top: 1px solid #174b3c; }

/* Technical */
.style-blueprint { --paper: #153e71; --ink: #eefaff; --muted: #b9d5e7; --accent: #7fe9f0; --accent-contrast: #153e71; --line: rgba(238, 250, 255, 0.34); --export-mat: #092541; --texture-opacity: 0.03; --display-font: 'Courier New', Courier, monospace; }
.style-blueprint::after { background-image: linear-gradient(rgba(255, 255, 255, 0.075) 1px, transparent 1px), linear-gradient(90deg, rgba(255, 255, 255, 0.075) 1px, transparent 1px); background-size: 20px 20px; }
.style-blueprint .ornament-pin { display: block; width: 16px; height: 16px; border-radius: 0; }
.style-blueprint .ornament-pin::before, .style-blueprint .ornament-pin::after { position: absolute; content: ''; background: var(--accent); }
.style-blueprint .ornament-pin::before { top: 7px; left: -7px; width: 28px; height: 1px; }
.style-blueprint .ornament-pin::after { top: -7px; left: 7px; width: 1px; height: 28px; }
.style-blueprint .ornament-pin-a { top: 34px; left: 34px; }
.style-blueprint .ornament-pin-b { right: 34px; bottom: 34px; }
.style-blueprint .report-header { border: 1px solid var(--ink); padding: 12px; }
.style-blueprint .title-block h1 { font-weight: 400; letter-spacing: -0.03em; text-transform: uppercase; }
.style-blueprint .focus-panel { padding: 24px; border: 1px solid var(--ink); }
.style-blueprint .metric-grid, .style-blueprint .metric-cell { border-color: var(--ink); }
.style-blueprint .detail-section { padding: 18px; border: 1px solid var(--line); }
.style-blueprint .report-signoff { display: grid; grid-template-columns: 1fr auto auto; padding: 14px; border: 1px solid var(--ink); }

.style-terminal { --paper: #080d0a; --ink: #9fffb5; --muted: #61aa73; --accent: #d7ff5b; --accent-contrast: #080d0a; --line: rgba(159, 255, 181, 0.25); --export-mat: #020503; --texture-opacity: 0.02; --display-font: 'Courier New', Courier, monospace; --body-font: 'Courier New', Courier, monospace; border: 10px solid #171d19; }
.style-terminal::after { background: repeating-linear-gradient(0deg, transparent 0 3px, rgba(159, 255, 181, 0.04) 3px 4px), radial-gradient(circle at center, transparent 45%, rgba(0, 0, 0, 0.5)); }
.style-terminal .ornament-route { top: 27px; left: 58px; display: block; color: var(--accent); }
.style-terminal .ornament-route::before { content: '[SYS] '; }
.style-terminal .report-header { padding-top: 12px; border-top: 1px solid var(--line); }
.style-terminal .brand-mark { border-radius: 50%; box-shadow: 0 0 14px rgba(159, 255, 181, 0.25); }
.style-terminal .title-block h1::before { content: '> '; color: var(--accent); }
.style-terminal .focus-panel { padding: 22px; border: 1px solid var(--accent); }
.style-terminal .metric-grid { gap: 7px; border: 0; }
.style-terminal .metric-cell { border: 1px solid var(--line); }
.style-terminal .section-heading h2::before { content: './'; color: var(--accent); }
.style-terminal .report-signoff { border-top: 1px dashed var(--accent); }

.style-analytics { --paper: #0d1728; --ink: #edf5ff; --muted: #9badc5; --accent: #55b6ff; --accent-contrast: #07111e; --line: rgba(132, 187, 255, 0.2); --export-mat: #050a13; --texture-opacity: 0.03; }
.style-analytics::after { background: linear-gradient(135deg, rgba(44, 108, 180, 0.18), transparent 45%), repeating-linear-gradient(90deg, transparent 0 79px, rgba(85, 182, 255, 0.04) 79px 80px); }
.style-analytics .ornament-serial { top: 30px; right: 58px; display: block; color: var(--accent); }
.style-analytics .report-header { padding-top: 18px; border-top: 2px solid var(--accent); }
.style-analytics .title-block { padding-block: 46px 34px; }
.style-analytics .focus-panel { padding: 24px; background: rgba(85, 182, 255, 0.055); border: 1px solid rgba(85, 182, 255, 0.35); }
.style-analytics .metric-grid { gap: 9px; border: 0; }
.style-analytics .metric-cell { position: relative; min-height: 128px; overflow: hidden; background: rgba(85, 182, 255, 0.055); border: 1px solid var(--line); }
.style-analytics .metric-cell::after { position: absolute; right: 16px; bottom: 13px; left: 16px; height: 2px; content: ''; background: linear-gradient(90deg, var(--accent) 0 36%, rgba(85, 182, 255, 0.16) 36%); }
.style-analytics .metric-cell strong { color: #9dd6ff; }
.style-analytics .detail-grid { gap: 12px; border: 0; }
.style-analytics .detail-section { padding: 20px; background: rgba(255, 255, 255, 0.025); border: 1px solid var(--line); }

.style-cyber-matrix { --paper: #08131d; --ink: #d9fbff; --muted: #75aab0; --accent: #33e6d7; --accent-contrast: #04100f; --line: rgba(51, 230, 215, 0.24); --export-mat: #02080c; --texture-opacity: 0.02; --display-font: 'Courier New', Courier, monospace; border: 1px solid var(--accent); box-shadow: inset 0 0 46px rgba(51, 230, 215, 0.07); }
.style-cyber-matrix::after { background-image: linear-gradient(rgba(51, 230, 215, 0.045) 1px, transparent 1px), linear-gradient(90deg, rgba(51, 230, 215, 0.045) 1px, transparent 1px); background-size: 36px 36px; }
.style-cyber-matrix .ornament-pin { display: block; width: 36px; height: 36px; border-radius: 0; }
.style-cyber-matrix .ornament-pin-a { top: 24px; left: 24px; border-right: 0; border-bottom: 0; }
.style-cyber-matrix .ornament-pin-b { right: 24px; bottom: 24px; border-top: 0; border-left: 0; }
.style-cyber-matrix .ornament-route { top: 24px; left: 50%; display: block; color: var(--accent); transform: translateX(-50%); }
.style-cyber-matrix .report-header { padding: 12px; background: rgba(51, 230, 215, 0.035); border: 1px solid var(--line); }
.style-cyber-matrix .title-block h1 { color: var(--ink); text-shadow: 0 0 20px rgba(51, 230, 215, 0.22); }
.style-cyber-matrix .focus-panel { padding: 26px; border: 1px solid var(--accent); box-shadow: inset 3px 0 var(--accent); }
.style-cyber-matrix .metric-grid { border: 1px solid var(--accent); }
.style-cyber-matrix .metric-cell { border-color: var(--accent); }
.style-cyber-matrix .section-number::before { content: '['; }
.style-cyber-matrix .section-number::after { content: ']'; }

/* Collectible */
.style-polaroid { --paper: #f7f5ee; --ink: #20252a; --muted: #73716c; --accent: #466985; --export-mat: #7d8488; --texture-opacity: 0.13; padding: 34px 34px 86px; }
.style-polaroid::after { inset: 12px; border: 1px solid rgba(32, 37, 42, 0.08); box-shadow: inset 0 -64px rgba(229, 225, 214, 0.75); }
.style-polaroid .ornament-route { right: 48px; bottom: 36px; display: block; color: #4a4a45; font-family: 'STKaiti', 'KaiTi', cursive; font-size: 12px; transform: rotate(-3deg); }
.style-polaroid .report-header { margin-inline: 12px; border-bottom: 0; }
.style-polaroid .title-block { min-height: 390px; display: flex; flex-direction: column; justify-content: flex-end; margin-top: 14px; padding: 46px; color: white; background: linear-gradient(155deg, #233a4d, #617a82 54%, #c7b99b); box-shadow: inset 0 0 70px rgba(0, 0, 0, 0.24); }
.style-polaroid .title-block::before { content: ''; position: absolute; width: 160px; height: 22px; top: 124px; left: 50%; background: rgba(235, 222, 191, 0.72); transform: translateX(-50%) rotate(-2deg); }
.style-polaroid .title-block h1, .style-polaroid .title-block .style-kicker, .style-polaroid .title-block .title-meta { color: #fff; }
.style-polaroid .title-block h1 { max-width: 560px; font-weight: 400; }
.style-polaroid .focus-panel, .style-polaroid .metric-grid, .style-polaroid .narrative-section, .style-polaroid .detail-grid, .style-polaroid .report-signoff { margin-inline: 20px; }
.style-polaroid .report-signoff { padding-bottom: 12px; }

.style-ticket { --paper: #f5e4df; --ink: #641c33; --muted: #875b68; --accent: #9c2348; --export-mat: #6d263d; --texture-opacity: 0.18; border: 2px solid var(--accent); }
.style-ticket::after { left: 72px; right: auto; width: 1px; border-left: 2px dashed rgba(100, 28, 51, 0.45); background: radial-gradient(circle at 0 14px, var(--export-mat) 0 5px, transparent 5.5px) 0 0 / 12px 28px repeat-y; }
.style-ticket .ornament-serial { top: 105px; left: 27px; display: block; color: var(--accent); writing-mode: vertical-rl; }
.style-ticket .ornament-pin { left: 64px; display: block; width: 16px; height: 16px; background: var(--export-mat); border: 0; }
.style-ticket .ornament-pin-a { top: 220px; }
.style-ticket .ornament-pin-b { bottom: 220px; }
.style-ticket .report-header, .style-ticket .title-block, .style-ticket .focus-panel, .style-ticket .metric-grid, .style-ticket .narrative-section, .style-ticket .detail-grid, .style-ticket .report-signoff { margin-left: 42px; }
.style-ticket .report-header { border-bottom: 3px double var(--accent); }
.style-ticket .title-block h1 { font-style: italic; font-weight: 500; }
.style-ticket .focus-panel { border-block-color: var(--accent); }
.style-ticket .metric-grid { border: 1px solid var(--accent); }

.style-vinyl-record { --paper: #d45b32; --ink: #fff3d7; --muted: #f4c6a9; --accent: #1e1a18; --accent-contrast: #fff3d7; --line: rgba(255, 243, 215, 0.32); --export-mat: #28211e; --texture-opacity: 0.06; }
.style-vinyl-record::after { right: 0; left: auto; top: 70px; width: 300px; height: 420px; border-radius: 55% 0 0 55%; background: repeating-radial-gradient(circle at 85% 50%, #161412 0 4px, #28231f 5px 8px); opacity: 0.34; }
.style-vinyl-record .ornament-medallion { top: 185px; right: 0; display: grid; width: 88px; height: 88px; color: #1e1a18; background: #e7be65; border: 15px solid #1e1a18; font-size: 28px; }
.style-vinyl-record .report-header { border-bottom: 4px solid var(--ink); }
.style-vinyl-record .title-block { min-height: 320px; padding-right: 230px; }
.style-vinyl-record .title-block h1 { max-width: 420px; font-style: italic; font-weight: 400; }
.style-vinyl-record .focus-panel { border-block: 4px solid var(--ink); }
.style-vinyl-record .metric-grid { background: rgba(30, 26, 24, 0.15); border-color: var(--ink); }
.style-vinyl-record .detail-grid { border-top: 4px solid var(--ink); }
.style-vinyl-record .item-sequence::before { content: 'A'; margin-right: 2px; }

.style-passport { --paper: #e9edf0; --ink: #182c46; --muted: #657487; --accent: #204d83; --accent-contrast: #f8fbff; --export-mat: #182c46; --texture-opacity: 0.1; --display-font: Georgia, serif; --body-font: 'SFMono-Regular', Consolas, monospace; border-left: 18px solid #173b67; padding-left: 74px; }
.style-passport::after { background: repeating-linear-gradient(135deg, transparent 0 11px, rgba(32, 77, 131, 0.04) 11px 12px), linear-gradient(90deg, rgba(23, 59, 103, 0.1) 0 34px, transparent 34px); }
.style-passport .ornament-medallion { top: 118px; right: 64px; display: grid; width: 110px; height: 110px; border: 3px double var(--accent); font-size: 54px; opacity: 0.28; }
.style-passport .ornament-serial { right: 70px; bottom: 35px; display: block; color: var(--ink); letter-spacing: 0.2em; }
.style-passport .report-header { border-bottom: 3px double var(--accent); }
.style-passport .title-block h1 { max-width: 500px; font-weight: 400; }
.style-passport .focus-panel { padding: 24px; border: 1px solid var(--accent); }
.style-passport .matter-reference span, .style-passport .matter-reference strong { padding: 12px 8px; border-radius: 50%; transform: rotate(-4deg); }
.style-passport .metric-grid { border: 1px solid var(--accent); }
.style-passport .report-signoff { padding-bottom: 30px; border-top: 3px double var(--accent); }

/* Ceremonial */
.style-tarot { --paper: #17152f; --ink: #f2e5ad; --muted: #bdb28a; --accent: #d2ac54; --accent-contrast: #17152f; --line: rgba(210, 172, 84, 0.35); --export-mat: #080716; --texture-opacity: 0.03; border: 14px solid #28234b; outline: 1px solid var(--accent); outline-offset: -24px; text-align: center; }
.style-tarot::after { inset: 30px; border: 1px solid rgba(210, 172, 84, 0.4); background: radial-gradient(circle at 18% 14%, var(--accent) 0 1px, transparent 2px), radial-gradient(circle at 82% 23%, var(--accent) 0 1px, transparent 2px), radial-gradient(circle at 24% 78%, var(--accent) 0 1px, transparent 2px), radial-gradient(circle at 76% 72%, var(--accent) 0 1px, transparent 2px); }
.style-tarot .ornament-medallion { top: 118px; left: 50%; display: grid; width: 96px; height: 96px; border: 3px double var(--accent); transform: translateX(-50%); font-size: 46px; }
.style-tarot .report-header, .style-tarot .title-meta, .style-tarot .report-signoff { justify-content: center; }
.style-tarot .report-index, .style-tarot .report-signoff > strong { display: none; }
.style-tarot .title-block { padding-top: 140px; }
.style-tarot .title-block h1 { margin-inline: auto; max-width: 560px; font-weight: 400; }
.style-tarot .focus-panel { display: block; margin-inline: 46px; padding: 30px; border: 3px double var(--accent); }
.style-tarot .focus-panel > .section-number { display: none; }
.style-tarot .metric-grid { margin-inline: 46px; border: 1px solid var(--accent); }
.style-tarot .detail-grid { margin-inline: 46px; border-top: 3px double var(--accent); }

.style-bank-note { --paper: #dfe7d7; --ink: #174c36; --muted: #547263; --accent: #8a5c2f; --accent-contrast: #f6f1dc; --export-mat: #365c4b; --texture-opacity: 0.18; border: 12px double #174c36; padding-inline: 70px; }
.style-bank-note::after { inset: 24px; border: 1px solid rgba(23, 76, 54, 0.55); background: repeating-radial-gradient(ellipse at center, transparent 0 15px, rgba(23, 76, 54, 0.055) 16px 17px); }
.style-bank-note .ornament-medallion { top: 118px; left: 50%; display: grid; width: 112px; height: 112px; color: var(--accent); border: 5px double var(--ink); box-shadow: 0 0 0 8px rgba(23, 76, 54, 0.12); transform: translateX(-50%); font-size: 52px; }
.style-bank-note .ornament-serial { right: 42px; bottom: 52px; display: block; color: var(--ink); transform: rotate(-90deg); }
.style-bank-note .report-header { border-block: 3px double var(--ink); padding-block: 10px; }
.style-bank-note .title-block { padding-top: 146px; text-align: center; }
.style-bank-note .title-block h1 { margin-inline: auto; font-weight: 500; }
.style-bank-note .focus-panel { padding: 26px; border: 3px double var(--ink); }
.style-bank-note .metric-grid { border: 3px double var(--ink); }
.style-bank-note .detail-grid { border-top: 3px double var(--ink); }

.style-steampunk { --paper: #27231f; --ink: #f0d596; --muted: #b7a16f; --accent: #c58434; --accent-contrast: #1e1a17; --line: rgba(197, 132, 52, 0.35); --export-mat: #100e0c; --texture-opacity: 0.04; border: 10px ridge #9a632d; }
.style-steampunk::after { background: radial-gradient(circle at 90% 10%, transparent 0 44px, rgba(197, 132, 52, 0.2) 45px 53px, transparent 54px), radial-gradient(circle at 5% 88%, transparent 0 68px, rgba(197, 132, 52, 0.14) 69px 78px, transparent 79px), repeating-linear-gradient(90deg, transparent 0 13px, rgba(197, 132, 52, 0.025) 13px 14px); }
.style-steampunk .ornament-pin { display: block; width: 13px; height: 13px; background: radial-gradient(circle, #f1c26c 0 2px, #7c4b20 3px 6px, #d19a50 7px); border: 1px solid #e0af63; box-shadow: 0 2px 5px #000; }
.style-steampunk .ornament-pin-a { top: 25px; left: 25px; }
.style-steampunk .ornament-pin-b { right: 25px; bottom: 25px; }
.style-steampunk .ornament-medallion { top: 118px; right: 58px; display: grid; width: 104px; height: 104px; border: 10px double var(--accent); box-shadow: inset 0 0 0 8px rgba(197, 132, 52, 0.12); font-size: 48px; transform: rotate(9deg); }
.style-steampunk .report-header { padding: 12px; border: 3px double var(--accent); }
.style-steampunk .title-block h1 { max-width: 500px; font-weight: 400; }
.style-steampunk .focus-panel, .style-steampunk .metric-grid { border-color: var(--accent); }
.style-steampunk .metric-grid { border: 3px double var(--accent); }
.style-steampunk .detail-section { padding: 20px; border: 1px solid var(--line); }

.style-wax-sealed-parchment { --paper: #ede0b9; --ink: #4b2430; --muted: #77605e; --accent: #a46e1f; --accent-contrast: #fff8df; --export-mat: #6a293c; --texture-opacity: 0.44; padding-inline: 82px; }
.style-wax-sealed-parchment::after { inset: 20px; border: 3px double rgba(164, 110, 31, 0.52); box-shadow: inset 0 0 60px rgba(94, 48, 34, 0.1); }
.style-wax-sealed-parchment .ornament-medallion { top: 120px; left: 50%; display: grid; width: 86px; height: 86px; color: var(--accent); border: 3px double var(--accent); transform: translateX(-50%); font-size: 42px; opacity: 0.72; }
.style-wax-sealed-parchment .ornament-route { top: 226px; left: 50%; display: block; color: var(--accent); transform: translateX(-50%); letter-spacing: 0.22em; }
.style-wax-sealed-parchment .report-header { justify-content: center; padding-bottom: 22px; border-bottom: 3px double var(--accent); }
.style-wax-sealed-parchment .report-index { display: none; }
.style-wax-sealed-parchment .title-block { padding-top: 146px; text-align: center; }
.style-wax-sealed-parchment .title-block h1 { margin-inline: auto; max-width: 600px; font-style: italic; font-weight: 400; }
.style-wax-sealed-parchment .title-meta { justify-content: center; }
.style-wax-sealed-parchment .focus-panel { padding: 34px; border: 3px double var(--accent); }
.style-wax-sealed-parchment .focus-copy h2::first-letter { color: var(--accent); font-size: 2.2em; font-family: var(--display-font); }
.style-wax-sealed-parchment .metric-grid { border: 3px double var(--accent); }
.style-wax-sealed-parchment .report-signoff { border-top: 3px double var(--accent); }
.style-wax-sealed-parchment .material-seal { width: 112px; height: 112px; }

.is-exporting { box-sizing: border-box; }
.is-exporting .sample-ribbon { print-color-adjust: exact; }

.brief-modal-fade-enter-active, .brief-modal-fade-leave-active { transition: opacity 180ms ease-out; }
.brief-modal-fade-enter-active .brief-modal-shell, .brief-modal-fade-leave-active .brief-modal-shell { transition: transform 180ms ease-out; }
.brief-modal-fade-enter-from, .brief-modal-fade-leave-to { opacity: 0; }
.brief-modal-fade-enter-from .brief-modal-shell, .brief-modal-fade-leave-to .brief-modal-shell { transform: translateY(10px) scale(0.985); }

@media (max-width: 940px) {
  .modal-close { top: -45px; right: 0; }
}

@media (max-width: 700px) {
  .brief-modal-backdrop { padding: 54px 10px 10px; }
  .report-sheet { min-height: 0; padding: 38px 26px 30px; }
  .title-block { padding-block: 46px 30px; }
  .title-block h1 { font-size: 42px; }
  .focus-panel { grid-template-columns: 34px 1fr; gap: 12px; }
  .metric-grid { grid-template-columns: 1fr 1fr; }
  .metric-cell:nth-child(2) { border-right: 0; }
  .metric-cell:nth-child(-n + 2) { border-bottom: 1px solid var(--line); }
  .detail-grid { grid-template-columns: 1fr; }
  .report-header { align-items: flex-start; }
  .report-signoff { flex-wrap: wrap; }
  .material-rule { right: 26px; left: 26px; }
  .modal-control-footer { align-items: flex-start; flex-direction: column; }
  .modal-control-footer p { display: none; }
  .footer-actions { width: 100%; justify-content: flex-end; }
  .utility-action span { display: none; }
  .style-ticket .report-header, .style-ticket .title-block, .style-ticket .focus-panel, .style-ticket .metric-grid, .style-ticket .narrative-section, .style-ticket .detail-grid, .style-ticket .report-signoff { margin-left: 18px; }
}

@media (prefers-reduced-motion: reduce) {
  .brief-modal-fade-enter-active, .brief-modal-fade-leave-active,
  .brief-modal-fade-enter-active .brief-modal-shell, .brief-modal-fade-leave-active .brief-modal-shell { transition: none; }
}
</style>
