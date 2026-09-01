<script setup lang="ts">
import { computed, type PropType } from 'vue'
import {
  Refresh,
  CopyDocument,
  Printer,
  Close,
  Check,
  Warning,
  Clock,
  Briefcase,
  OfficeBuilding,
  Document,
  TrendCharts,
  Calendar,
  Finished,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import html2canvas from 'html2canvas'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  type: {
    type: String, // 'daily' | 'weekly'
    default: 'daily',
  },
  styleVariant: {
    type: String,
    default: '',
  },
  title: {
    type: String,
    default: '',
  },
  dateText: {
    type: String,
    default: '',
  },
  content: {
    type: String,
    default: '',
  },
  dateRange: {
    type: String,
    default: '',
  },
  loading: {
    type: Boolean,
    default: false,
  },
  // 结构化数据
  nextAction: {
    type: Object as PropType<any>,
    default: null,
  },
  redlines: {
    type: Array as PropType<any[]>,
    default: () => [],
  },
  hearings: {
    type: Array as PropType<any[]>,
    default: () => [],
  },
  metrics: {
    type: Object as PropType<any>,
    default: () => ({
      committedHours: '0.0',
      freeSpaceHours: '8.5h',
      waitingCount: 0,
      completedCount: 0,
      totalCases: 0,
    }),
  },
})

const emit = defineEmits(['update:visible', 'regenerate'])

const activeStyle = computed(() => {
  if (props.styleVariant) return props.styleVariant
  return props.type === 'daily' ? 'gazette' : 'dossier'
})

const effectiveTitle = computed(() => {
  if (props.title) return props.title
  return props.type === 'daily' ? 'Casy·日报' : 'Casy·本周周报'
})

const effectiveDate = computed(() => {
  if (props.type === 'weekly' && props.dateRange) {
    return props.dateRange
  }
  if (props.dateText) return props.dateText
  const now = new Date()
  return `${now.getFullYear()}年${now.getMonth() + 1}月${now.getDate()}日`
})

// 情绪价值文案种子库 (空状态语录) - 易于扩展和更新
const emptyStateSeeds = [
  {
    taskName: '当前无焦点任务',
    description: '您的待办列表处于清空状态。去喝杯咖啡，或者主动找点案源开拓一下吧！',
    caseName: '系统状态',
    caseCode: 'SYSTEM'
  },
  {
    taskName: '享受当下的宁静',
    description: '今天没有任何紧急硬性任务在追赶你，保持这种良好的节奏，享受清醒的头脑。',
    caseName: '身心管理',
    caseCode: 'ZEN'
  },
  {
    taskName: '一切尽在掌握中',
    description: '当前暂无焦点待办任务，您可以把精力留给深度思考、案卷沉淀与战略筹划。',
    caseName: '状态播报',
    caseCode: 'CLEAR'
  },
  {
    taskName: '给自己放个短假',
    description: '没有永远打不完的仗，既然今天没有紧迫的焦点任务，不如提前规划一下本周的长期目标。',
    caseName: '精力恢复',
    caseCode: 'REST'
  },
  {
    taskName: '静水流深，厚积薄发',
    description: '手头暂无紧急火情。不妨整理一下过去的案卷文档，看看有哪些经验可以沉淀到您的专属知识库。',
    caseName: '知识沉淀',
    caseCode: 'BUILD'
  },
  {
    taskName: '预言家日报：一切安好',
    description: '魔法部的傲罗们似乎今天没有派发新的通缉令，魔法界迎来了一个和平的早晨，请安心享用黄油啤酒。',
    caseName: '魔法部日程',
    caseCode: 'MAGIC'
  },
  {
    taskName: '时间转换器的闲暇',
    description: '你不需要使用赫敏的时间转换器来赶进度了。当下就是最好的时间，用来研读一本厚重的魔法咒语书吧。',
    caseName: '时空管理',
    caseCode: 'RETRO'
  },
  {
    taskName: '活点地图：无人来访',
    description: '活点地图上没有正在靠近的麻烦，你的领地十分安全。现在是恶作剧完毕、好好休息的绝佳时机。',
    caseName: '安全确认',
    caseCode: 'MAP'
  },
  {
    taskName: '有求必应屋的宁静',
    description: '当你有求于清闲时，这间屋子便为你屏蔽了外界的喧嚣。案卷已经封印，享受纯粹的阅读时光。',
    caseName: '隐秘之境',
    caseCode: 'ROOM'
  },
  {
    taskName: '冥想盆里的沉淀',
    description: '记忆已经抽取，烦恼已被封存。与其向外索求，不如在冥想盆中回顾曾经的经典战役。',
    caseName: '记忆沉思',
    caseCode: 'PENS'
  },
  {
    taskName: '福灵剂的幸运时刻',
    description: '似乎你今天喝下了一剂福灵剂——所有麻烦都自动绕开了你。保持直觉，去做你最想做的那件事。',
    caseName: '幸运满溢',
    caseCode: 'LUCK'
  },
  {
    taskName: '守护神已巡视完毕',
    description: '你的守护神在周围盘旋，驱散了所有的摄魂怪与焦虑。今天是个阳光明媚的绝佳工作日。',
    caseName: '守护时刻',
    caseCode: 'PATR'
  },
  {
    taskName: '霍格沃茨特快列车',
    description: '列车正平稳行驶在苏格兰高地，没有突发的紧急制动。放松心情，看看窗外掠过的风景。',
    caseName: '平稳旅程',
    caseCode: 'TRAIN'
  },
  {
    taskName: '老魔杖的暂时歇息',
    description: '即使是最强大的法器也需要休息。今天没有需要你火力全开的辩护，收起魔杖，享受午后红茶。',
    caseName: '魔力恢复',
    caseCode: 'WAND'
  },
  {
    taskName: '猫头鹰尚未带来信件',
    description: '早晨的猫头鹰棚屋静悄悄的，说明外界并无急情。你可以按自己的节奏，翻开新的一页。',
    caseName: '静谧清晨',
    caseCode: 'OWL'
  },
  {
    taskName: '邓布利多的微笑',
    description: '校长在打量着你，并为你出色的清空待办事项的能力而微微颔首。去吧，去探寻更有趣的魔法知识。',
    caseName: '智慧启迪',
    caseCode: 'WISE'
  }
]

const displayNextAction = computed(() => {
  if (props.nextAction) return props.nextAction
  // 使用当前日期(一年中的第几天)来选择种子，保证每天不同但刷新不抖动
  const now = new Date()
  const start = new Date(now.getFullYear(), 0, 0)
  const diff = now.getTime() - start.getTime()
  const dayOfYear = Math.floor(diff / (1000 * 60 * 60 * 24))
  return emptyStateSeeds[dayOfYear % emptyStateSeeds.length]
})

function close() {
  emit('update:visible', false)
}

function onRegenerate() {
  emit('regenerate')
}

async function copyContent() {
  const fullText = `【${effectiveTitle.value} · ${effectiveDate.value}】\n\n` +
    `首要焦点：${displayNextAction.value.taskName}\n` +
    `硬性红线：${props.redlines?.map((r: any) => `• ${r.title} (${r.timeText})`).join('\n') || '今日无到期红线'}\n` +
    `排期日程：${props.hearings?.map((h: any) => `• ${h.time} ${h.title} [${h.court}]`).join('\n') || '无开庭'}\n` +
    `负荷指标：已排期 ${props.metrics?.committedHours}h，弹性余量 ${props.metrics?.freeSpaceHours}。\n\n` +
    (props.content ? `正文详述：\n${props.content}` : '')

  try {
    await navigator.clipboard.writeText(fullText)
    ElMessage.success('报告内容已复制到剪贴板')
  } catch {
    ElMessage.info(fullText)
  }
}

async function exportImage() {
  const element = document.querySelector('.brief-modal-shell') as HTMLElement
  if (!element) return

  // Clone the element to avoid touching the live UI and to guarantee full capture
  const clone = element.cloneNode(true) as HTMLElement
  
  // Apply full border-radius to paper-card for the image
  const paperCard = clone.querySelector('.paper-card') as HTMLElement
  if (paperCard) {
    paperCard.style.borderRadius = 'var(--c-radius-xl)';
    paperCard.style.overflow = 'visible';
  }

  // Add exporting class to handle html2canvas quirks (like gradient text)
  clone.classList.add('exporting-mode')

  // Inject dark background for glassmorphism so it doesn't export as transparent/white
  if (activeStyle.value === 'glassmorphism') {
    clone.style.background = '#0f172a';
    const paper = clone.querySelector('.glass-paper') as HTMLElement
    if (paper) {
      paper.style.backdropFilter = 'none'
      paper.style.setProperty('-webkit-backdrop-filter', 'none')
      paper.style.background = 'rgba(255, 255, 255, 0.15)'
    }
  }

  // Remove max-height and overflow to render full content
  const scrollContents = clone.querySelectorAll('.paper-scroll-content')
  scrollContents.forEach((el) => {
    (el as HTMLElement).style.maxHeight = 'none';
    (el as HTMLElement).style.overflowY = 'visible';
    (el as HTMLElement).style.height = 'auto';
  })

  // Hide the footer so it's not in the image
  const footer = clone.querySelector('.modal-control-footer') as HTMLElement
  if (footer) footer.style.display = 'none'

  // 根据样式动态选择打码字符
  const getRedactChar = (style: string) => {
    if (['cyber', 'analytics'].includes(style)) return '░░░░' // 赛博/暗黑：数字杂讯感
    if (['dossier', 'typewriter'].includes(style)) return '[REDACTED]' // 绝密档案/打字机：英文涂改感
    if (['executive', 'ledger', 'partner-brief', 'gazette'].includes(style)) return '***' // 商业简报：星号脱敏
    if (['narrative-air', 'vogue', 'swiss-grid', 'glassmorphism', 'milestones'].includes(style)) return '某某' // 优雅现代风格：中式化名
    return 'XXX' // 默认占位
  }
  
  const rep = getRedactChar(activeStyle.value)

  // Redact sensitive text
  const redact = (text: string) => {
    return text
      .replace(/[\u4e00-\u9fa5]+人民法院/g, `${rep}人民法院`)
      .replace(/[\u4e00-\u9fa5]{2,}知识产权法院/g, `${rep}知识产权法院`)
      .replace(/[\u4e00-\u9fa5]{2,}法院/g, `${rep}法院`)
      .replace(/([a-zA-Z\u4e00-\u9fa5]+)(股份|科技|有限|责任)公司/g, `${rep}$2公司`)
  }

  const walker = document.createTreeWalker(clone, NodeFilter.SHOW_TEXT, null)
  let node
  while ((node = walker.nextNode())) {
    if (node.nodeValue) {
      node.nodeValue = redact(node.nodeValue)
    }
  }

  // Mount clone off-screen
  clone.style.position = 'absolute'
  clone.style.top = '-9999px'
  clone.style.left = '0'
  clone.style.margin = '0'
  clone.style.width = '620px' // Keep original width
  document.body.appendChild(clone)

  try {
    const canvas = await html2canvas(clone, {
      scale: 2,
      useCORS: true,
      backgroundColor: null
    })
    
    document.body.removeChild(clone)

    const link = document.createElement('a')
    link.download = `CASY_Report_${effectiveDate.value}.png`
    link.href = canvas.toDataURL('image/png')
    link.click()
    ElMessage.success('已成功保存为长图')
  } catch (e) {
    document.body.removeChild(clone)
    ElMessage.error('导出图片失败，请重试')
  }
}

function renderMarkdown(md: string) {
  if (!md) return ''
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const inline = (s: string) => esc(s).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  let html = ''
  let inList = false
  for (const line of md.split('\n')) {
    const t = line.trim()
    if (t.startsWith('- ') || t.startsWith('* ')) {
      if (!inList) { html += '<ul class="brief-ul">'; inList = true }
      html += `<li class="brief-li">${inline(t.slice(2))}</li>`
      continue
    }
    if (inList) { html += '</ul>'; inList = false }
    if (!t) continue
    if (t.startsWith('### ')) html += `<h5 class="brief-h5">${inline(t.slice(4))}</h5>`
    else if (t.startsWith('## ')) html += `<h4 class="brief-h4">${inline(t.slice(3))}</h4>`
    else if (t.startsWith('# ')) html += `<h3 class="brief-h3">${inline(t.slice(2))}</h3>`
    else html += `<p class="brief-p">${inline(t)}</p>`
  }
  if (inList) html += '</ul>'
  return html
}
</script>

<template>
  <transition name="brief-modal-fade">
    <div v-if="visible" class="brief-modal-backdrop" @click.self="close">
      <div class="brief-modal-shell" :class="[`style-${activeStyle}`, `type-${type}`]">
        <!-- ═══ 1. GAZETTE / DISPATCH 风格 (复古报纸与小票) ═══ -->
        <div v-if="activeStyle === 'gazette'" class="paper-card gazette-paper">
          <div class="receipt-clip-top"><span class="clip-bar"></span></div>
          <div class="gazette-masthead">
            <div class="masthead-preline">
              <span>LEGAL DISPATCH · CASY INTELLIGENCE</span>
              <span>ISSUE #{{ new Date().getDate() }}</span>
            </div>
            <h1 class="gazette-title">{{ type === 'daily' ? 'CASY DAILY' : 'CASY WEEKLY' }}</h1>
            <div class="gazette-sub">{{ effectiveTitle }} · {{ effectiveDate }}</div>
            <div class="gazette-double-line"></div>
          </div>

          <div class="paper-scroll-content">
            <div v-if="content" class="md-article-view" v-html="renderMarkdown(content)"></div>
            <div v-else class="structured-editorial-flow">
              <div class="editorial-box">
                <div class="kicker-tag">✦ LEAD HEADLINE / 重点聚焦</div>
                <h3 class="lead-h3">{{ displayNextAction.taskName }}</h3>
                <p class="lead-p">{{ displayNextAction.description }}</p>
              </div>
              <div class="thin-rule"></div>
              <div class="editorial-box">
                <div class="kicker-tag">✦ STATUTORY REDLINES / 硬性红线</div>
                <div class="redlines-stack">
                  <div v-for="r in redlines" :key="r.id" class="redline-row">
                    <span class="dot-red"></span>
                    <div class="r-text"><strong>{{ r.title }}</strong><small>{{ r.caseTitle }} · {{ r.timeText }}</small></div>
                  </div>
                </div>
              </div>
              <div class="thin-rule"></div>
              <div class="editorial-box">
                <div class="kicker-tag">✦ DAILY DOCKET / 庭审排期</div>
                <div class="hearings-stack">
                  <div v-for="h in hearings" :key="h.id" class="hearing-row">
                    <span class="h-time">{{ h.time }}</span>
                    <div class="h-text"><strong>{{ h.title }}</strong><small>{{ h.court }} ({{ h.judge }})</small></div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="tear-barcode-section">
            <div class="cut-dash">✂ - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -</div>
            <div class="barcode-row">
              <div class="barcode-bars">|||| | |||| || | ||||| | ||| |||||</div>
              <span class="barcode-num">CASY-AUTH-{{ new Date().getFullYear() }}-{{ String(new Date().getMonth()+1).padStart(2,'0') }}</span>
            </div>
          </div>
          <div class="sawtooth-bottom"></div>
        </div>

        <!-- ═══ 2. TYPEWRITER 风格 (打字机黑白公文) ═══ -->
        <div v-else-if="activeStyle === 'typewriter'" class="paper-card typewriter-paper">
          <div class="typewriter-header">
            <div class="tw-doc-id">MEMORANDUM // {{ effectiveDate }}</div>
            <h1 class="tw-main-title">[ EXECUTIVE SUMMARY // {{ effectiveTitle.toUpperCase() }} ]</h1>
            <div class="tw-meta-line">CLASSIFICATION: ATTORNEY WORK PRODUCT · RESTRICTED</div>
            <div class="tw-divider-thick"></div>
          </div>
          <div class="paper-scroll-content tw-font">
            <div v-if="content" v-html="renderMarkdown(content)"></div>
            <div v-else class="tw-content-body">
              <p><strong>01. PRIMARY DIRECTIVE:</strong><br>> {{ displayNextAction.taskName }} (CASE: {{ displayNextAction.caseCode }})<br>{{ displayNextAction.description }}</p>
              <div class="tw-dash-divider">--------------------------------------------------</div>
              <p><strong>02. TIME-SENSITIVE STATUTORY EXPOSURE:</strong></p>
              <div v-for="r in redlines" :key="r.id" class="tw-item-line">
                [!] {{ r.title }} -- {{ r.caseTitle }} (DUE: {{ r.timeText }})
              </div>
              <div class="tw-dash-divider">--------------------------------------------------</div>
              <p><strong>03. PROCEEDINGS & HEARINGS:</strong></p>
              <div v-for="h in hearings" :key="h.id" class="tw-item-line">
                [*] {{ h.time }} // {{ h.title }} // LOC: {{ h.court }}
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 3. SWISS GRID 风格 (瑞士国际主义网格) ═══ -->
        <div v-else-if="activeStyle === 'swiss-grid'" class="paper-card swiss-paper">
          <div class="swiss-top-bar">
            <div class="swiss-big-num">{{ new Date().getDate() }}</div>
            <div class="swiss-title-block">
              <span class="swiss-kicker">SWISS GRID REPORT · DISPATCH</span>
              <h1 class="swiss-title">{{ effectiveTitle }}</h1>
            </div>
          </div>
          <div class="swiss-grid-cols">
            <div class="swiss-col-left">
              <div class="swiss-metric-big">
                <span class="s-val">{{ redlines.length }}</span>
                <span class="s-lbl">CRITICAL DEADLINES</span>
              </div>
              <div class="swiss-metric-big">
                <span class="s-val">{{ metrics.committedHours }}h</span>
                <span class="s-lbl">COMMITTED HOURS</span>
              </div>
            </div>
            <div class="swiss-col-right paper-scroll-content">
              <h4 class="swiss-h4">ACTION PRIORITIES</h4>
              <div class="swiss-card-box">
                <strong>{{ displayNextAction.taskName }}</strong>
                <p>{{ displayNextAction.description }}</p>
              </div>
              <h4 class="swiss-h4">COURT PROCEEDINGS</h4>
              <div v-for="h in hearings" :key="h.id" class="swiss-h-card">
                <span class="badge-red">{{ h.time }}</span>
                <span>{{ h.title }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 4. VOGUE EDITORIAL (时尚杂志) ═══ -->
        <div v-else-if="activeStyle === 'vogue'" class="paper-card vogue-paper">
          <div class="vogue-head">
            <span class="vogue-issue">ISSUE NO. {{ new Date().getMonth()+1 }}</span>
            <h1 class="vogue-title">CASY<br>VOGUE</h1>
            <div class="vogue-date">{{ effectiveDate }}</div>
          </div>
          <div class="paper-scroll-content vogue-body">
            <h2 class="vogue-lead">{{ effectiveTitle }}</h2>
            <div class="vogue-divider"></div>
            <div class="vogue-focus">
              <span class="v-tag">THE FOCUS</span>
              <h3>{{ displayNextAction.taskName }}</h3>
              <p>{{ displayNextAction.description }}</p>
            </div>
            <div class="vogue-grid">
              <div class="vogue-col">
                <span class="v-tag">DEADLINES</span>
                <div v-for="r in redlines" :key="r.id" class="v-redline">{{ r.title }}<br><small>{{ r.timeText }}</small></div>
              </div>
              <div class="vogue-col">
                <span class="v-tag">HEARINGS</span>
                <div v-for="h in hearings" :key="h.id" class="v-hearing">{{ h.time }} // {{ h.title }}<br><small>{{ h.court }}</small></div>
              </div>
            </div>
          </div>
        </div>
        <!-- ═══ 5. NARRATIVE AIR 风格 (清风优雅叙事) ═══ -->
        <div v-else-if="activeStyle === 'narrative-air'" class="paper-card air-paper">
          <div class="air-header">
            <span class="air-date-pill">{{ effectiveDate }}</span>
            <h1 class="air-title">{{ effectiveTitle }}</h1>
            <p class="air-lead">静水流深 · 每日秩序律动与重点关注</p>
          </div>
          <div class="paper-scroll-content air-body">
            <p class="air-prose">
              今日核心推进工作集中在<strong>「{{ displayNextAction.taskName }}」</strong>。请重点跟进相关证据对照与法条检索。
            </p>
            <div class="air-card-quote">
              <div class="quote-bar"></div>
              <div>
                <strong>当前硬性绝限：</strong>
                <span v-for="r in redlines" :key="r.id">【{{ r.title }}（{{ r.timeText }}）】 </span>
              </div>
            </div>
            <p class="air-prose">
              今日法庭庭审共计 {{ hearings.length }} 场，全天已承诺办案负荷为 {{ metrics.committedHours }} 小时，保持 {{ metrics.freeSpaceHours }} 的弹性应对窗口。
            </p>
          </div>
        </div>

        <!-- ═══ 6. GLASSMORPHISM (毛玻璃光晕) ═══ -->
        <div v-else-if="activeStyle === 'glassmorphism'" class="paper-card glass-paper">
          <div class="glass-bg-blob blob-1"></div>
          <div class="glass-bg-blob blob-2"></div>
          <div class="glass-content-wrap">
            <div class="glass-header">
              <div class="g-date-pill">{{ effectiveDate }}</div>
              <h1>{{ effectiveTitle }}</h1>
            </div>
            <div class="paper-scroll-content glass-body">
              <div class="glass-card g-focus">
                <strong>⚡️ TODAY'S FOCUS</strong>
                <h2>{{ displayNextAction.taskName }}</h2>
                <p>{{ displayNextAction.description }}</p>
              </div>
              <div class="glass-row">
                <div class="glass-card g-alert">
                  <strong>🚨 REDLINES</strong>
                  <div v-for="r in redlines" :key="r.id">{{ r.title }} ({{ r.timeText }})</div>
                </div>
                <div class="glass-card g-schedule">
                  <strong>📅 COURT SCHEDULE</strong>
                  <div v-for="h in hearings" :key="h.id">{{ h.time }} - {{ h.title }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
        <!-- ═══ 7. ACTION KANBAN 风格 (便签行动看板) ═══ -->
        <div v-else-if="activeStyle === 'action-board'" class="paper-card kanban-board-paper">
          <div class="kb-header">
            <h2 class="kb-title">📌 {{ effectiveTitle }} · 便签卡片</h2>
            <span class="kb-date">{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content kb-cols-grid">
            <div class="kb-sticky-col yellow">
              <div class="kb-col-head">🔥 今日必达 (Must Do)</div>
              <div class="kb-note-card">
                <strong>{{ displayNextAction.taskName }}</strong>
                <p>{{ displayNextAction.description }}</p>
              </div>
            </div>
            <div class="kb-sticky-col red">
              <div class="kb-col-head">⚠️ 红线绝限 (Redlines)</div>
              <div v-for="r in redlines" :key="r.id" class="kb-note-card mini">
                <strong>{{ r.title }}</strong>
                <small>{{ r.timeText }}</small>
              </div>
            </div>
            <div class="kb-sticky-col blue">
              <div class="kb-col-head">⚖️ 庭审会见 (Schedule)</div>
              <div v-for="h in hearings" :key="h.id" class="kb-note-card mini">
                <strong>{{ h.time }} · {{ h.title }}</strong>
                <small>{{ h.court }}</small>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 8. EXECUTIVE MEMO 风格 (律所高管简报) ═══ -->
        <div v-else-if="activeStyle === 'executive'" class="paper-card executive-paper">
          <div class="exec-header">
            <div class="exec-seal">⚖️</div>
            <div class="exec-header-text">
              <span class="exec-sup">PARTNER LEVEL BRIEFING</span>
              <h1 class="exec-title">{{ effectiveTitle }}</h1>
              <span class="exec-date">{{ effectiveDate }} · CONFIDENTIAL</span>
            </div>
          </div>
          <div class="exec-gold-divider"></div>
          <div class="paper-scroll-content exec-body">
            <div class="exec-kpi-row">
              <div class="exec-kpi-item">
                <span class="lbl">FOCUS CASE</span>
                <strong>{{ displayNextAction.caseName || '重点专案' }}</strong>
              </div>
              <div class="exec-kpi-item">
                <span class="lbl">REDLINE RISK</span>
                <strong class="text-risk">{{ redlines.length }} 项需关注</strong>
              </div>
              <div class="exec-kpi-item">
                <span class="lbl">TOTAL LOAD</span>
                <strong>{{ metrics.committedHours }} 小时</strong>
              </div>
            </div>
            <div class="exec-content-block">
              <h4>战略要务推进</h4>
              <p><strong>{{ displayNextAction.taskName }}</strong>：{{ displayNextAction.description }}</p>
            </div>
          </div>
        </div>

        <!-- ═══ 9. DOSSIER (绝密归档) ═══ -->
        <div v-else-if="activeStyle === 'dossier'" class="paper-card dossier-paper">
          <div class="dossier-stamp">CONFIDENTIAL</div>
          <div class="dossier-header">
            <span class="dossier-file-no">FILE REF: WK-{{ new Date().getFullYear() }}-{{ String(new Date().getMonth()+1).padStart(2,'0') }}</span>
            <h1 class="dossier-title">{{ effectiveTitle }}</h1>
            <div class="dossier-sub">{{ effectiveDate }} · LEGAL DOSSIER</div>
          </div>
          <div class="dossier-rule"></div>
          <div class="paper-scroll-content dossier-body">
            <div class="dossier-grid-two">
              <div class="dossier-section">
                <h3>[I] MAJOR LITIGATION</h3>
                <p class="d-val">{{ displayNextAction.taskName || '重点专案推进' }}</p>
                <p>{{ displayNextAction.description }}</p>
              </div>
              <div class="dossier-section">
                <h3>[II] CRITICAL EXPOSURE</h3>
                <div v-for="r in redlines" :key="r.id" class="dossier-bullet">
                  <strong>X</strong> {{ r.title }} <em>({{ r.timeText }})</em>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 10. NEON DASHBOARD (暗黑仪表盘) ═══ -->
        <div v-else-if="activeStyle === 'analytics'" class="paper-card neon-dash-paper">
          <div class="neon-head">
            <h1>{{ effectiveTitle }}</h1>
            <span class="neon-badge">{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content neon-body">
            <div class="neon-kpi-grid">
              <div class="n-card">
                <span class="n-lbl">COMPLETED</span>
                <strong class="n-val text-blue">{{ metrics.completedCount || 18 }}</strong>
              </div>
              <div class="n-card">
                <span class="n-lbl">HOURS</span>
                <strong class="n-val text-purple">{{ metrics.committedHours || 32 }}h</strong>
              </div>
              <div class="n-card">
                <span class="n-lbl">PENDING</span>
                <strong class="n-val text-pink">{{ metrics.waitingCount || 4 }}</strong>
              </div>
            </div>
            <div class="n-card glow-card mt-3">
              <span class="n-lbl">PRIORITY METRIC</span>
              <h3>{{ displayNextAction.taskName }}</h3>
              <div class="neon-progress"><div class="n-bar" style="width:75%"></div></div>
            </div>
          </div>
        </div>

        <!-- ═══ 11. SUPREME LEDGER (黑金台账) ═══ -->
        <div v-else-if="activeStyle === 'ledger'" class="paper-card luxury-ledger-paper">
          <div class="lux-header">
            <div class="lux-logo">CASY</div>
            <div class="lux-title-box">
              <h2>{{ effectiveTitle }}</h2>
              <span>{{ effectiveDate }} // WEEKLY LEDGER</span>
            </div>
          </div>
          <div class="paper-scroll-content lux-body">
            <table class="lux-table">
              <thead>
                <tr>
                  <th>MATTER / ITEM</th>
                  <th>STATUS</th>
                  <th>VARIANCE</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>{{ displayNextAction.taskName }}</td>
                  <td>ACTIVE</td>
                  <td class="t-gold">ON TRACK</td>
                </tr>
                <tr v-for="r in redlines" :key="r.id">
                  <td>{{ r.title }}</td>
                  <td>{{ r.timeText }}</td>
                  <td class="t-red">URGENT</td>
                </tr>
                <tr v-for="h in hearings" :key="h.id">
                  <td>{{ h.title }} ({{ h.court }})</td>
                  <td>{{ h.time }}</td>
                  <td>SCHEDULED</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- ═══ 12. FLUID MILESTONES (流体时间轴) ═══ -->
        <div v-else-if="activeStyle === 'milestones'" class="paper-card fluid-ms-paper">
          <div class="fluid-head">
            <h1>{{ effectiveTitle }}</h1>
            <p>{{ effectiveDate }}</p>
          </div>
          <div class="paper-scroll-content fluid-body">
            <div class="fluid-track">
              <div class="f-node done">
                <div class="f-bubble"></div>
                <div class="f-content">
                  <strong>Initiation</strong>
                  <p>Filings completed</p>
                </div>
              </div>
              <div class="f-node active">
                <div class="f-bubble glow"></div>
                <div class="f-content">
                  <strong>Current Phase</strong>
                  <p>{{ displayNextAction.taskName }}</p>
                </div>
              </div>
              <div class="f-node">
                <div class="f-bubble"></div>
                <div class="f-content">
                  <strong>Upcoming</strong>
                  <p>Awaiting court decision</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 13. PARTNER LETTER (合伙人复盘信函) ═══ -->
        <div v-else-if="activeStyle === 'partner-brief'" class="paper-card partner-letter-paper">
          <div class="letter-head">
            <div class="letter-firm-name">CASY PARTNERSHIP ATTORNEYS AT LAW</div>
            <div class="letter-meta-row"><span>DATE: {{ effectiveDate }}</span><span>MEMORANDUM TO PARTNERS</span></div>
          </div>
          <div class="letter-divider"></div>
          <div class="paper-scroll-content letter-body">
            <p>Dear Partner,</p>
            <p>本周业务推进平稳有序。我们在<strong>「{{ displayNextAction.caseName || '重点专案' }}」</strong>取得了关键进展，完成了庭审证据链反驳工作。</p>
            <p>在风险控制方面，本周共有 {{ redlines.length }} 项法定绝限得到严密监控与执行，无逾期违规情况。</p>
            <p class="letter-sign">Respectfully submitted,<br><span class="sig-font">Casy Lead Counsel</span></p>
          </div>
        </div>

        <!-- ═══ 14. NEO BRUTALISM (新粗野主义) ═══ -->
        <div v-else-if="activeStyle === 'focus-matrix'" class="paper-card brutal-paper">
          <div class="brutal-head">
            <h1>{{ effectiveTitle }}</h1>
            <span class="brutal-tag">{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content brutal-grid">
            <div class="brutal-box b-pink">
              <h3>FOCUS</h3>
              <p>{{ displayNextAction.taskName }}</p>
            </div>
            <div class="brutal-box b-yellow">
              <h3>DUE / EXPOSURE</h3>
              <p v-for="r in redlines" :key="r.id">{{ r.title }} ({{ r.timeText }})</p>
            </div>
            <div class="brutal-box b-blue">
              <h3>HEARINGS</h3>
              <p v-for="h in hearings" :key="h.id">{{ h.time }} - {{ h.title }}</p>
            </div>
          </div>
        </div>

        <!-- ═══ 15. CHRONICLE (编年史) ═══ -->
        <div v-else-if="activeStyle === 'chronicle'" class="paper-card dark-chronicle-paper">
          <div class="chronicle-head">
            <h1>THE WEEKLY CHRONICLE</h1>
            <span>{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content dark-chron-flow">
            <div class="dc-row"><div class="dc-dot"></div><strong>MON</strong><span>立案受理与客户沟通会议完成</span></div>
            <div class="dc-row"><div class="dc-dot glow"></div><strong>WED</strong><span class="t-highlight">{{ displayNextAction.taskName }}</span></div>
            <div class="dc-row"><div class="dc-dot"></div><strong>FRI</strong><span>法庭庭审质证及周度总结</span></div>
          </div>
        </div>

        <!-- ═══ 16. CYBER MATRIX (赛博战力周报) ═══ -->
        <div v-else-if="activeStyle === 'cyber-matrix'" class="paper-card cyber-paper">
          <div class="cyber-head">
            <span class="cyber-glitch">[ CYBER_ORDER_MATRIX // WEEKLY ]</span>
            <h2>HUD BATTLE REPORT</h2>
          </div>
          <div class="paper-scroll-content cyber-body">
            <div class="cyber-stat-bar">POWER INDEX: 98.4% // ORDERS COMPLETED: {{ metrics.completedCount || 18 }}</div>
            <div class="cyber-box">
              TARGET LOCKED: {{ displayNextAction.taskName }}
            </div>
            <div class="cyber-grid">
              <div v-for="r in redlines" :key="r.id" class="c-danger">[!] {{ r.title }} ({{ r.timeText }})</div>
            </div>
          </div>
        </div>

        <!-- ═══ 17. MAGIC PROPHET (预言家日报) ═══ -->
        <div v-else-if="activeStyle === 'magic-prophet'" class="paper-card prophet-paper">
          <div class="prophet-head">
            <div class="prophet-ornament">⚯͛</div>
            <h1>The Daily Prophet</h1>
            <div class="prophet-meta">{{ effectiveDate }} · EXCLUSIVE EDITION</div>
          </div>
          <div class="prophet-divider"></div>
          <div class="paper-scroll-content prophet-body">
            <h2 class="prophet-lead">{{ effectiveTitle }}</h2>
            <div class="prophet-grid">
              <div class="p-col-main">
                <h3>MAGIC FOCUS</h3>
                <p><strong>{{ displayNextAction.taskName }}</strong></p>
                <p class="p-desc">{{ displayNextAction.description }}</p>
              </div>
              <div class="p-col-side">
                <h3>CRITICAL</h3>
                <div v-for="r in redlines" :key="r.id" class="p-red">{{ r.title }}<br><small>{{ r.timeText }}</small></div>
                <h3 class="mt-2">HEARINGS</h3>
                <div v-for="h in hearings" :key="h.id" class="p-hear">{{ h.time }} - {{ h.title }}</div>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 18. HOGWARTS LETTER (霍格沃茨录取信) ═══ -->
        <div v-else-if="activeStyle === 'hogwarts-letter'" class="paper-card hogwarts-paper">
          <div class="hogwarts-head">
            <div class="h-crest">H</div>
            <h2>HOGWARTS SCHOOL of WITCHCRAFT and WIZARDRY</h2>
            <p>Headmaster: Albus Dumbledore</p>
          </div>
          <div class="paper-scroll-content hogwarts-body">
            <p>Dear Mr/Ms,</p>
            <p>We are pleased to inform you that your summary for <strong>{{ effectiveTitle }}</strong> ({{ effectiveDate }}) is complete.</p>
            <p>Your primary focus shall be <strong>{{ displayNextAction.taskName }}</strong>. {{ displayNextAction.description }}</p>
            <p v-if="redlines.length">Please note the following critical exposures:<br>
              <span v-for="r in redlines" :key="r.id"> - {{ r.title }} ({{ r.timeText }})<br></span>
            </p>
            <p>Yours sincerely,</p>
            <div class="h-sig">Minerva McGonagall<br><span>Deputy Headmistress</span></div>
          </div>
          <div class="wax-seal">
            <div class="seal-inner">H</div>
          </div>
        </div>

        <!-- ═══ 19. TELEGRAPH (复古电报) ═══ -->
        <div v-else-if="activeStyle === 'telegraph'" class="paper-card telegraph-paper">
          <div class="tele-stamp">RECEIVED {{ effectiveDate }}</div>
          <div class="tele-head">
            <h2>INTERNATIONAL TELEGRAPH</h2>
            <p>CASY COMMUNICATION NETWORK</p>
          </div>
          <div class="paper-scroll-content tele-body">
            <p>URGENT DISPATCH STOP</p>
            <p>FOCUS ITEM: {{ displayNextAction.taskName }} STOP</p>
            <p>{{ displayNextAction.description }} STOP</p>
            <p v-if="redlines.length">WARNING: {{ redlines[0].title }} DUE {{ redlines[0].timeText }} STOP</p>
            <p>END OF MESSAGE STOP</p>
          </div>
        </div>

        <!-- ═══ 20. BULLETIN (通缉令/警情通报) ═══ -->
        <div v-else-if="activeStyle === 'bulletin'" class="paper-card bulletin-paper">
          <div class="bull-head">
            <h1>WANTED</h1>
            <h2>FOR IMMEDIATE ACTION</h2>
            <p>REWARD ISSUED BY CASY DEPT.</p>
          </div>
          <div class="paper-scroll-content bull-body">
            <div class="bull-img-placeholder">PHOTO NOT AVAILABLE</div>
            <h3>{{ displayNextAction.taskName }}</h3>
            <p>{{ displayNextAction.description }}</p>
            <div class="bull-details">
              <strong>CRIMES/EXPOSURE:</strong>
              <div v-for="r in redlines" :key="r.id">{{ r.title }} ({{ r.timeText }})</div>
            </div>
          </div>
        </div>

        <!-- ═══ 21. BLUEPRINT (工业蓝图) ═══ -->
        <div v-else-if="activeStyle === 'blueprint'" class="paper-card blueprint-paper">
          <div class="bp-grid-overlay"></div>
          <div class="bp-content">
            <div class="bp-head">
              <h1>PROJECT ARCHITECTURE</h1>
              <div class="bp-meta">
                <span>DATE: {{ effectiveDate }}</span>
                <span>SCALE: 1:1</span>
                <span>DRW: CASY-{{ metrics.committedHours }}</span>
              </div>
            </div>
            <div class="paper-scroll-content bp-body">
              <div class="bp-box">
                <div class="bp-lbl">ELEVATION A: FOCUS</div>
                <h3>{{ displayNextAction.taskName }}</h3>
                <p>{{ displayNextAction.description }}</p>
              </div>
              <div class="bp-row">
                <div class="bp-box">
                  <div class="bp-lbl">STRUCTURAL REDLINES</div>
                  <div v-for="r in redlines" :key="r.id">-> {{ r.title }} ({{ r.timeText }})</div>
                </div>
                <div class="bp-box">
                  <div class="bp-lbl">TIMELINE</div>
                  <div v-for="h in hearings" :key="h.id">-> {{ h.time }} {{ h.title }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 22. TERMINAL (复古终端机) ═══ -->
        <div v-else-if="activeStyle === 'terminal'" class="paper-card terminal-paper">
          <div class="term-head">
            CASY DOS V1.0 (C) 1985<br>
            C:\> RUN {{ type === 'daily' ? 'DAILY.EXE' : 'WEEKLY.EXE' }}
          </div>
          <div class="paper-scroll-content term-body">
            <div>Loading data... OK.</div>
            <br>
            <div>[FOCUS] {{ displayNextAction.taskName }}</div>
            <div>[DESC] {{ displayNextAction.description }}</div>
            <br>
            <div v-if="redlines.length">[WARNINGS]</div>
            <div v-for="r in redlines" :key="r.id">> {{ r.title }} ERRCD:{{ r.timeText }}</div>
            <br>
            <div class="term-cursor">_</div>
          </div>
        </div>

        <!-- ═══ 23. POLAROID (拍立得相纸) ═══ -->
        <div v-else-if="activeStyle === 'polaroid'" class="paper-card polaroid-paper">
          <div class="pol-photo">
            <div class="pol-inner">
              <div class="pol-text-overlay">
                <h3>{{ displayNextAction.taskName }}</h3>
                <p>{{ displayNextAction.description }}</p>
                <div class="pol-reds">
                  <div v-for="r in redlines" :key="r.id">! {{ r.title }}</div>
                </div>
              </div>
            </div>
          </div>
          <div class="pol-marker">{{ effectiveTitle }} - {{ effectiveDate }}</div>
        </div>

        <!-- ═══ 24. THEATRE TICKET (复古票根) ═══ -->
        <div v-else-if="activeStyle === 'ticket'" class="paper-card ticket-paper">
          <div class="tick-stub">
            <div class="tick-vert">ADMIT ONE</div>
          </div>
          <div class="tick-main">
            <div class="tick-head">CASY GRAND THEATRE</div>
            <div class="paper-scroll-content tick-body">
              <div class="tick-title">{{ effectiveTitle }}</div>
              <div class="tick-date">{{ effectiveDate }}</div>
              <div class="tick-feat">FEATURING: {{ displayNextAction.taskName }}</div>
              <div class="tick-desc">{{ displayNextAction.description }}</div>
              <div class="tick-row" v-if="hearings.length">
                <span>SHOWTIME: {{ hearings[0].time }}</span>
                <span>SEAT: {{ hearings[0].court }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 25. SCROLL (东方卷轴) ═══ -->
        <div v-else-if="activeStyle === 'scroll'" class="paper-card scroll-paper">
          <div class="scroll-head">
            <div class="scroll-inkan">律</div>
            <h1>{{ effectiveTitle }}</h1>
            <span class="scroll-date">{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content scroll-body">
            <p class="s-lead">今日所重：{{ displayNextAction.taskName }}</p>
            <p class="s-desc">{{ displayNextAction.description }}</p>
            <div v-if="redlines.length" class="s-danger">
              <p>急迫事项：</p>
              <p v-for="r in redlines" :key="r.id">· {{ r.title }} ({{ r.timeText }})</p>
            </div>
          </div>
        </div>

        <!-- ═══ 26. CLASSIFIED FILE (冷战绝密档案) ═══ -->
        <div v-else-if="activeStyle === 'classified-file'" class="paper-card classified-paper">
          <div class="class-tab">FILE No. 4077</div>
          <div class="class-top-stamp">TOP SECRET</div>
          <div class="paper-scroll-content class-body">
            <h2>SUBJECT: {{ displayNextAction.taskName }}</h2>
            <p><strong>DATE:</strong> {{ effectiveDate }}</p>
            <div class="class-redact-box">
              <p><strong>DETAILS:</strong> {{ displayNextAction.description }}</p>
            </div>
            <h3>EXPOSURES</h3>
            <ul class="class-list">
              <li v-for="r in redlines" :key="r.id">{{ r.title }} [DUE: {{ r.timeText }}]</li>
            </ul>
          </div>
          <div class="class-clip"></div>
        </div>

        <!-- ═══ 27. VINYL RECORD (黑胶唱片) ═══ -->
        <div v-else-if="activeStyle === 'vinyl-record'" class="paper-card vinyl-paper">
          <div class="vinyl-record-bg"></div>
          <div class="vinyl-sleeve">
            <div class="v-band-name">CASY SOUNDS PRESENTS</div>
            <h1 class="v-album">{{ effectiveTitle }}</h1>
            <div class="paper-scroll-content v-tracklist">
              <h3>SIDE A (FOCUS)</h3>
              <p>1. {{ displayNextAction.taskName }}</p>
              <h3>SIDE B (EXPOSURE)</h3>
              <p v-for="(r, i) in redlines" :key="r.id">{{ i+2 }}. {{ r.title }} ({{ r.timeText }})</p>
            </div>
          </div>
        </div>

        <!-- ═══ 28. TAROT (神秘塔罗牌) ═══ -->
        <div v-else-if="activeStyle === 'tarot'" class="paper-card tarot-paper">
          <div class="tarot-border">
            <div class="tarot-num">XIV</div>
            <div class="tarot-art">
              <div class="t-moon"></div>
              <div class="t-stars">✦ ✧ ✦</div>
            </div>
            <div class="paper-scroll-content tarot-body">
              <h2>{{ displayNextAction.taskName }}</h2>
              <p>{{ displayNextAction.description }}</p>
              <div class="t-fate" v-if="redlines.length">
                <span>OMENS</span>
                <div v-for="r in redlines" :key="r.id">{{ r.title }}</div>
              </div>
            </div>
            <div class="tarot-bottom">{{ effectiveTitle }}</div>
          </div>
        </div>

        <!-- ═══ 29. BANK NOTE (复古钞票) ═══ -->
        <div v-else-if="activeStyle === 'bank-note'" class="paper-card bank-paper">
          <div class="bank-border">
            <div class="bank-corners">
              <span>{{ metrics.completedCount || 100 }}</span><span>{{ metrics.completedCount || 100 }}</span>
            </div>
            <div class="bank-head">
              <h2>CASY RESERVE NOTE</h2>
              <p>LEGAL TENDER FOR ALL DEBTS, PUBLIC AND PRIVATE</p>
            </div>
            <div class="paper-scroll-content bank-body">
              <div class="bank-center-art">
                <div class="b-seal">C</div>
                <div class="b-text">
                  <h3>{{ displayNextAction.taskName }}</h3>
                  <p>{{ displayNextAction.description }}</p>
                </div>
              </div>
            </div>
            <div class="bank-corners bottom">
              <span>{{ effectiveDate }}</span><span>{{ effectiveDate }}</span>
            </div>
          </div>
        </div>

        <!-- ═══ 30. PASSPORT (国际护照) ═══ -->
        <div v-else-if="activeStyle === 'passport'" class="paper-card passport-paper">
          <div class="pass-head">
            <div class="pass-crest">⚖️</div>
            <h1>PASSPORT</h1>
            <p>CASY REPUBLIC</p>
          </div>
          <div class="pass-inner">
            <div class="pass-photo"></div>
            <div class="paper-scroll-content pass-data">
              <div class="p-row"><span>Type/P</span><span>Code/CAS</span><span>No. 198234</span></div>
              <div class="p-row"><span>Name</span><strong>{{ displayNextAction.taskName }}</strong></div>
              <div class="p-row"><span>Details</span><strong>{{ displayNextAction.description }}</strong></div>
              <div class="p-row"><span>Date</span><strong>{{ effectiveDate }}</strong></div>
            </div>
          </div>
          <div class="pass-mrz">
            P&lt;CAS&lt;&lt;{{ effectiveDate.replace(/\D/g, '') }}&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;<br>
            {{ String(displayNextAction.caseCode).padEnd(20, '<') }}&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;&lt;
          </div>
        </div>

        <!-- ═══ 31. STEAMPUNK (蒸汽朋克) ═══ -->
        <div v-else-if="activeStyle === 'steampunk'" class="paper-card steampunk-paper">
          <div class="steam-border">
            <div class="rivet r1"></div><div class="rivet r2"></div><div class="rivet r3"></div><div class="rivet r4"></div>
            <div class="steam-head">
              <h1>THE BRASS CHRONICLE</h1>
              <p>-- {{ effectiveDate }} --</p>
            </div>
            <div class="paper-scroll-content steam-body">
              <div class="s-gear-bg"></div>
              <h2>⚙️ PRIMARY DIRECTIVE</h2>
              <p><strong>{{ displayNextAction.taskName }}</strong></p>
              <p>{{ displayNextAction.description }}</p>
              <div v-if="redlines.length">
                <h2>⏱️ PRESSURE VALVES</h2>
                <p v-for="r in redlines" :key="r.id">>> {{ r.title }} ({{ r.timeText }})</p>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 32. ROYAL DECREE (王室诏书) ═══ -->
        <div v-else class="paper-card decree-paper">
          <div class="decree-head">
            <div class="d-crown">♔</div>
            <h1>Royal Decree</h1>
            <p>By the Grace of Casy</p>
          </div>
          <div class="paper-scroll-content decree-body">
            <p class="d-dropcap">
              <span class="d-first-letter">W</span>hereas it has been brought to our attention that the matter of <strong>{{ displayNextAction.taskName }}</strong> requires immediate action.
            </p>
            <p>{{ displayNextAction.description }}</p>
            <p v-if="redlines.length">Furthermore, we mandate compliance with the following:</p>
            <ul class="d-list">
              <li v-for="r in redlines" :key="r.id">{{ r.title }} by {{ r.timeText }}</li>
            </ul>
            <div class="d-sign">
              <span>Given this {{ effectiveDate }}</span>
              <div class="d-wax"></div>
            </div>
          </div>
        </div>

        <!-- ═══ 底部统一工具栏 ═══ -->
        <div class="modal-control-footer">
          <div class="footer-left-meta">
            <span>样式：{{ activeStyle }}</span>
            <span class="type-pill">{{ type === 'daily' ? '日报模态' : '周报模态' }}</span>
          </div>
          <div class="footer-btn-group">
            <button class="btn-tool" :disabled="loading" @click="onRegenerate">
              <el-icon><Refresh /></el-icon>
              <span>{{ loading ? '生成中…' : '重新生成' }}</span>
            </button>
            <button class="btn-tool" @click="copyContent">
              <el-icon><CopyDocument /></el-icon>
              <span>复制全文</span>
            </button>
            <button class="btn-tool" @click="exportImage">
              <el-icon><Printer /></el-icon>
              <span>保存为图片</span>
            </button>
            <button class="btn-tool-close" @click="close">
              <el-icon><Close /></el-icon>
              <span>关闭</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
/* ═══════════════════════════════════════════════════════════
   Universal Briefing & Reports Modal Styles (16 Styles)
   ═══════════════════════════════════════════════════════════ */
.brief-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 3000;
  background: rgba(0, 0, 0, 0.65);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  display: flex;
  padding: 24px;
  overflow-y: auto;
}

.brief-modal-shell {
  position: relative;
  max-width: 620px;
  width: 100%;
  margin: auto;
  perspective: 1000px;
}

.paper-card {
  position: relative;
  background: var(--c-bg-card);
  color: var(--c-text);
  border-radius: var(--c-radius-xl) var(--c-radius-xl) 0 0;
  box-shadow: 0 25px 60px rgba(0, 0, 0, 0.35);
  padding: 24px 28px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
}

.paper-scroll-content {
  max-height: 420px;
  overflow-y: auto;
  padding-right: 4px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 1. GAZETTE (报纸小票) */
.gazette-paper {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
}
.receipt-clip-top { display: flex; justify-content: center; margin-bottom: 2px; }
.clip-bar { width: 45px; height: 4px; background: var(--c-border-strong); border-radius: 2px; }
.gazette-masthead { text-align: center; display: flex; flex-direction: column; gap: 4px; }
.masthead-preline { display: flex; justify-content: space-between; font-family: var(--font-mono); font-size: 10px; color: var(--slate-gray-light); font-weight: 700; }
.gazette-title { font-family: 'Times New Roman', serif; font-size: 24px; font-weight: 900; letter-spacing: 2px; color: var(--c-text-heading); margin: 0; }
.gazette-sub { font-size: 12px; color: var(--slate-gray-light); }
.gazette-double-line { height: 4px; border-top: 2px solid var(--c-text-heading); border-bottom: 1px solid var(--c-text-heading); margin-top: 6px; }
.kicker-tag { font-family: var(--font-mono); font-size: 10.5px; font-weight: 700; color: var(--c-primary); }
.lead-h3 { font-size: 14.5px; font-weight: 700; color: var(--c-text-heading); margin: 2px 0 0; }
.lead-p { font-size: 12px; color: var(--c-text-secondary); margin: 2px 0 0; line-height: 1.5; }
.thin-rule { height: 1px; background: var(--c-border); margin: 6px 0; }
.redlines-stack, .hearings-stack { display: flex; flex-direction: column; gap: 6px; }
.redline-row, .hearing-row { display: flex; align-items: center; gap: 8px; padding: 6px 8px; background: var(--c-bg-page); border-radius: var(--c-radius); border: 1px solid var(--c-border); font-size: 12px; }
.dot-red { width: 7px; height: 7px; border-radius: 50%; background: var(--status-risk); flex-shrink: 0; }
.h-time { font-family: var(--font-mono); font-size: 11px; font-weight: 700; color: var(--c-primary); flex-shrink: 0; }
.cut-dash { text-align: center; font-family: var(--font-mono); font-size: 10px; color: var(--slate-gray-light); letter-spacing: 2px; }
.barcode-row { display: flex; flex-direction: column; align-items: center; gap: 2px; font-family: var(--font-mono); font-size: 9px; color: var(--slate-gray-light); }
.barcode-bars { font-size: 18px; letter-spacing: 1px; color: var(--c-text-heading); }
.sawtooth-bottom { position: absolute; bottom: 0; left: 0; right: 0; height: 6px; background: radial-gradient(circle, transparent, transparent 50%, var(--c-bg-card) 50%, var(--c-bg-card) 100%); background-size: 12px 12px; }

/* 2. TYPEWRITER (打字机公文) */
.typewriter-paper {
  background: #fdfdfd;
  color: #111;
  border: 1px solid #111;
  font-family: 'Courier New', Courier, monospace;
}
.tw-doc-id { font-size: 11px; color: #666; font-weight: bold; }
.tw-main-title { font-size: 16px; font-weight: 900; margin: 4px 0; letter-spacing: 1px; }
.tw-meta-line { font-size: 10.5px; color: #444; }
.tw-divider-thick { height: 3px; background: #111; margin-top: 6px; }
.tw-dash-divider { color: #888; margin: 8px 0; }
.tw-content-body { font-size: 12px; line-height: 1.6; }
.tw-item-line { margin: 4px 0; }

/* 3. SWISS GRID (瑞士网格) */
.swiss-paper {
  background: #ffffff;
  color: #000;
  border-left: 6px solid #B4554F;
  border-top: 2px solid #000;
}
.swiss-top-bar { display: flex; align-items: center; gap: 16px; border-bottom: 2px solid #000; padding-bottom: 10px; }
.swiss-big-num { font-size: 38px; font-weight: 900; line-height: 1; color: #000; font-family: Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; }
.swiss-title-block { display: flex; flex-direction: column; }
.swiss-kicker { font-size: 10px; font-weight: 800; letter-spacing: 1px; color: #B4554F; }
.swiss-title { font-size: 18px; font-weight: 900; margin: 0; color: #000; }
.swiss-grid-cols { display: grid; grid-template-columns: 140px 1fr; gap: 16px; }
.swiss-metric-big { padding: 10px; background: #f4f4f5; border-radius: 4px; margin-bottom: 8px; }
.s-val { font-size: 22px; font-weight: 900; color: #000; display: block; }
.s-lbl { font-size: 9px; font-weight: 700; color: #71717a; }
.swiss-h4 { font-size: 11px; font-weight: 800; margin: 6px 0 4px; color: #000; letter-spacing: 0.5px; }
.swiss-card-box { padding: 8px; border: 1px solid #000; font-size: 12px; margin-bottom: 8px; }
.swiss-h-card { display: flex; align-items: center; gap: 6px; padding: 4px 0; font-size: 12px; border-bottom: 1px solid #e4e4e7; }
.badge-red { background: #000; color: #fff; padding: 1px 4px; font-size: 10px; font-weight: 700; }

/* 4. VOGUE EDITORIAL */
.vogue-paper { background: #fff; color: #111; font-family: 'Times New Roman', Times, serif; border: 12px solid #f8f8f8; box-shadow: 0 10px 40px rgba(0,0,0,0.1); }
.vogue-head { text-align: center; border-bottom: 1px solid #111; padding-bottom: 20px; margin-bottom: 20px; }
.vogue-issue { font-family: var(--font-mono); font-size: 10px; letter-spacing: 2px; display: block; margin-bottom: 10px; }
.vogue-title { font-size: 48px; font-weight: 400; line-height: 0.85; margin: 0; letter-spacing: -1px; }
.vogue-date { margin-top: 10px; font-size: 12px; font-style: italic; }
.vogue-lead { font-size: 18px; text-align: center; font-weight: normal; margin: 0 0 10px; }
.vogue-divider { width: 40px; height: 1px; background: #111; margin: 0 auto 20px; }
.vogue-focus { text-align: center; margin-bottom: 20px; }
.vogue-focus h3 { font-size: 20px; margin: 8px 0; font-weight: normal; }
.vogue-focus p { font-size: 14px; color: #555; }
.v-tag { font-family: var(--font-mono); font-size: 9px; letter-spacing: 1px; font-weight: bold; text-transform: uppercase; border-bottom: 1px solid #111; padding-bottom: 2px; margin-bottom: 8px; display: inline-block; }
.vogue-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; text-align: left; }
.vogue-col { border-top: 1px solid #eee; padding-top: 10px; }
.v-redline, .v-hearing { font-size: 13px; margin-bottom: 10px; line-height: 1.4; }

/* 5. NARRATIVE AIR (清风散文叙事) */
.air-paper {
  background: #fdfdfd;
  border: 1px solid #e2e8f0;
}
.air-date-pill { padding: 2px 8px; border-radius: 20px; background: #e0f2fe; color: #0369a1; font-size: 11px; font-weight: 600; display: inline-block; }
.air-title { font-size: 20px; font-weight: 700; margin: 6px 0 2px; color: #0f172a; }
.air-lead { font-size: 12.5px; color: #64748b; margin: 0; }
.air-prose { font-size: 13.5px; line-height: 1.7; color: #334155; }
.air-card-quote { padding: 12px 14px; background: #f8fafc; border-radius: 8px; display: flex; gap: 10px; font-size: 12.5px; }
.quote-bar { width: 3px; background: #3b82f6; border-radius: 2px; }

/* 6. GLASSMORPHISM */
.glass-paper { background: rgba(255, 255, 255, 0.1); border: 1px solid rgba(255,255,255,0.2); backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px); overflow: hidden; }
.glass-bg-blob { position: absolute; z-index: 0; opacity: 0.8; pointer-events: none; }
.blob-1 { width: 300px; height: 300px; background: radial-gradient(circle, rgba(168,85,247,0.7) 0%, rgba(168,85,247,0) 70%); top: -100px; left: -100px; }
.blob-2 { width: 250px; height: 250px; background: radial-gradient(circle, rgba(236,72,153,0.7) 0%, rgba(236,72,153,0) 70%); bottom: -50px; right: -50px; }
.glass-content-wrap { position: relative; z-index: 1; display: flex; flex-direction: column; height: 100%; color: #fff; text-shadow: 0 1px 2px rgba(0,0,0,0.1); }
.glass-header { margin-bottom: 20px; }
.g-date-pill { display: inline-block; padding: 4px 12px; background: rgba(255,255,255,0.2); border-radius: 20px; font-size: 10px; font-weight: bold; letter-spacing: 1px; margin-bottom: 8px; }
.glass-header h1 { font-size: 24px; font-weight: 800; margin: 0; letter-spacing: -0.5px; }
.glass-card { background: rgba(255,255,255,0.15); border: 1px solid rgba(255,255,255,0.3); border-radius: 12px; padding: 16px; margin-bottom: 12px; backdrop-filter: blur(10px); }
.g-focus strong, .g-alert strong, .g-schedule strong { font-size: 11px; opacity: 0.9; margin-bottom: 6px; display: block; }
.g-focus h2 { font-size: 18px; margin: 0 0 4px; }
.g-focus p { font-size: 13px; opacity: 0.8; margin: 0; }
.glass-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.g-alert div, .g-schedule div { font-size: 12px; margin-bottom: 4px; padding-bottom: 4px; border-bottom: 1px solid rgba(255,255,255,0.1); }

/* 7. ACTION KANBAN (便签看板) */
.kanban-board-paper {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
}
.kb-header { display: flex; justify-content: space-between; align-items: center; }
.kb-title { font-size: 16px; font-weight: 700; margin: 0; color: #0f172a; }
.kb-cols-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
.kb-sticky-col { padding: 10px; border-radius: 8px; display: flex; flex-direction: column; gap: 8px; }
.kb-sticky-col.yellow { background: #fef9c3; border: 1px solid #fef08a; color: #713f12; }
.kb-sticky-col.red { background: #fee2e2; border: 1px solid #fecaca; color: #7f1d1d; }
.kb-sticky-col.blue { background: #e0f2fe; border: 1px solid #bae6fd; color: #0c4a6e; }
.kb-col-head { font-size: 11.5px; font-weight: 700; }
.kb-note-card { background: #fff; padding: 6px 8px; border-radius: 6px; box-shadow: var(--shadow-sm); font-size: 11.5px; }

/* 8. EXECUTIVE (律所高管简报) */
.executive-paper {
  background: #181d28;
  color: #f1f5f9;
  border: 1.5px solid #d97706;
}
.exec-header { display: flex; align-items: center; gap: 12px; }
.exec-seal { font-size: 26px; }
.exec-sup { font-size: 10px; color: #fbbf24; letter-spacing: 1px; font-weight: 700; }
.exec-title { font-size: 18px; font-weight: 800; margin: 2px 0; color: #fff; }
.exec-gold-divider { height: 2px; background: linear-gradient(90deg, #d97706, #fbbf24, transparent); margin: 8px 0; }
.exec-kpi-row { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin-bottom: 12px; }
.exec-kpi-item { background: #222838; padding: 8px 10px; border-radius: 6px; }
.exec-kpi-item .lbl { font-size: 9.5px; color: #94a3b8; display: block; }
.exec-kpi-item strong { font-size: 13px; color: #fff; }

/* 9. DOSSIER */
.dossier-paper { background: #efebe4; color: #2a2826; border: 2px solid #8b3a3a; font-family: 'Courier New', Courier, monospace; box-shadow: inset 0 0 30px rgba(0,0,0,0.05); }
.dossier-stamp { position: absolute; right: 20px; top: 30px; border: 3px solid #b91c1c; color: #b91c1c; padding: 4px 12px; font-size: 14px; font-weight: 900; transform: rotate(15deg); opacity: 0.8; letter-spacing: 2px; }
.dossier-file-no { font-size: 10px; color: #8b3a3a; font-weight: 700; display: block; margin-bottom: 4px; }
.dossier-title { font-size: 22px; font-weight: 800; margin: 0; color: #111; font-family: 'Times New Roman', serif; }
.dossier-sub { font-size: 12px; color: #555; }
.dossier-rule { height: 2px; background: #8b3a3a; margin: 12px 0; }
.dossier-grid-two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
.dossier-section h3 { font-size: 14px; font-weight: 900; margin: 0 0 8px; color: #8b3a3a; border-bottom: 1px dashed #8b3a3a; padding-bottom: 4px; }
.d-val { font-weight: bold; font-size: 14px; }
.dossier-bullet { font-size: 12px; margin-bottom: 6px; }

/* 10. NEON DASHBOARD */
.neon-dash-paper { background: #0f172a; color: #e2e8f0; border: 1px solid #1e293b; font-family: Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; }
.neon-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; }
.neon-head h1 { font-size: 20px; font-weight: 800; margin: 0; background: linear-gradient(90deg, #3b82f6, #8b5cf6); -webkit-background-clip: text; color: transparent; }
.exporting-mode .neon-head h1 { background: none !important; -webkit-background-clip: initial !important; color: #8b5cf6 !important; text-shadow: 0 0 10px rgba(139,92,246,0.5); }
.neon-badge { background: rgba(59,130,246,0.2); color: #60a5fa; padding: 4px 10px; border-radius: 12px; font-size: 10px; font-weight: bold; border: 1px solid #3b82f6; }
.neon-kpi-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
.n-card { background: #1e293b; border-radius: 10px; padding: 12px; border: 1px solid #334155; }
.n-lbl { font-size: 10px; color: #94a3b8; font-weight: 700; display: block; margin-bottom: 4px; }
.n-val { font-size: 24px; font-weight: 900; }
.text-blue { color: #3b82f6; text-shadow: 0 0 10px rgba(59,130,246,0.5); }
.text-purple { color: #8b5cf6; text-shadow: 0 0 10px rgba(139,92,246,0.5); }
.text-pink { color: #ec4899; text-shadow: 0 0 10px rgba(236,72,153,0.5); }
.glow-card { border-color: #3b82f6; box-shadow: 0 0 15px rgba(59,130,246,0.15); }
.glow-card h3 { font-size: 16px; margin: 0 0 12px; color: #fff; }
.neon-progress { height: 6px; background: #0f172a; border-radius: 3px; overflow: hidden; }
.n-bar { height: 100%; background: linear-gradient(90deg, #3b82f6, #ec4899); box-shadow: 0 0 8px #ec4899; }

/* 11. SUPREME LEDGER */
.luxury-ledger-paper { background: #111; color: #d4d4d4; border: 2px solid #d4af37; border-radius: 4px; }
.lux-header { display: flex; align-items: center; gap: 16px; border-bottom: 1px solid #333; padding-bottom: 16px; margin-bottom: 16px; }
.lux-logo { font-size: 28px; font-family: 'Times New Roman', serif; color: #d4af37; border-right: 1px solid #333; padding-right: 16px; }
.lux-title-box h2 { font-size: 16px; margin: 0; color: #fff; font-weight: 600; letter-spacing: 1px; }
.lux-title-box span { font-size: 10px; color: #d4af37; font-family: var(--font-mono); letter-spacing: 2px; }
.lux-table { width: 100%; border-collapse: collapse; font-size: 12px; font-family: var(--font-mono); }
.lux-table th { text-align: left; padding: 10px 8px; border-bottom: 1px solid #d4af37; color: #d4af37; font-weight: normal; }
.lux-table td { padding: 12px 8px; border-bottom: 1px solid #222; }
.t-gold { color: #d4af37; }
.t-red { color: #ef4444; }

/* 12. FLUID MILESTONES */
.fluid-ms-paper { background: #fdf2f8; border: none; border-top: 6px solid #ec4899; box-shadow: 0 10px 30px rgba(236,72,153,0.1); }
.fluid-head h1 { font-size: 22px; color: #831843; margin: 0; font-weight: 800; }
.fluid-head p { font-size: 12px; color: #be185d; margin: 4px 0 16px; }
.fluid-track { display: flex; flex-direction: column; gap: 0; position: relative; padding-left: 20px; }
.fluid-track::before { content: ''; position: absolute; left: 24px; top: 10px; bottom: 10px; width: 2px; background: linear-gradient(to bottom, #ec4899, #fbcfe8); }
.f-node { display: flex; gap: 16px; padding: 12px 0; position: relative; z-index: 1; }
.f-bubble { width: 10px; height: 10px; border-radius: 50%; background: #fbcfe8; border: 2px solid #fff; margin-top: 4px; flex-shrink: 0; }
.f-node.done .f-bubble { background: #ec4899; }
.f-node.active .f-bubble.glow { background: #db2777; box-shadow: 0 0 0 4px rgba(219, 39, 119, 0.2); }
.f-content strong { display: block; font-size: 14px; color: #831843; }
.f-content p { margin: 2px 0 0; font-size: 12px; color: #9d174d; }

/* 13. PARTNER LETTER */
.partner-letter-paper { background: #fdfbf7; border: 1px solid #d4c5b9; font-family: Georgia, serif; }
.letter-firm-name { font-size: 13px; font-weight: 700; letter-spacing: 1px; color: #433; text-align: center; margin-bottom: 10px; }
.letter-meta-row { display: flex; justify-content: space-between; font-size: 11px; color: #655; text-transform: uppercase; }
.letter-divider { height: 1px; background: #a89f91; margin: 12px 0; }
.letter-body { font-size: 14px; line-height: 1.8; color: #222; }
.letter-sign { margin-top: 20px; }
.sig-font { font-family: 'Brush Script MT', 'Lucida Handwriting', 'Segoe Print', cursive; font-size: 24px; color: #111; display: block; margin-top: 8px; }

/* 14. NEO BRUTALISM */
.brutal-paper { background: #fff; border: 4px solid #000; box-shadow: 8px 8px 0px #000; border-radius: 0; }
.brutal-head { border-bottom: 4px solid #000; padding-bottom: 12px; margin-bottom: 16px; display: flex; justify-content: space-between; align-items: flex-end; }
.brutal-head h1 { font-size: 28px; font-weight: 900; margin: 0; text-transform: uppercase; letter-spacing: -1px; }
.brutal-tag { font-family: var(--font-mono); font-weight: bold; background: #000; color: #fff; padding: 4px 8px; font-size: 12px; }
.brutal-grid { display: flex; flex-direction: column; gap: 16px; }
.brutal-box { border: 3px solid #000; padding: 12px; box-shadow: 4px 4px 0px #000; }
.b-pink { background: #fbcfe8; }
.b-yellow { background: #fef08a; }
.b-blue { background: #bfdbfe; }
.brutal-box h3 { font-size: 14px; font-weight: 900; margin: 0 0 8px; border-bottom: 2px solid #000; display: inline-block; padding-bottom: 2px; }
.brutal-box p { font-size: 13px; font-weight: 600; margin: 4px 0; }

/* 15. CHRONICLE */
.dark-chronicle-paper { background: #18181b; color: #fafafa; border: 1px solid #3f3f46; border-radius: 12px; }
.chronicle-head { margin-bottom: 20px; }
.chronicle-head h1 { font-size: 22px; margin: 0 0 4px; font-weight: 800; letter-spacing: -0.5px; }
.chronicle-head span { font-size: 12px; color: #a1a1aa; }
.dark-chron-flow { display: flex; flex-direction: column; gap: 16px; }
.dc-row { display: flex; align-items: center; gap: 12px; font-size: 13px; background: #27272a; padding: 12px; border-radius: 8px; }
.dc-dot { width: 8px; height: 8px; border-radius: 50%; background: #52525b; }
.dc-dot.glow { background: #10b981; box-shadow: 0 0 8px #10b981; }
.dc-row strong { font-family: var(--font-mono); color: #d4d4d8; }
.t-highlight { color: #34d399; font-weight: 600; }

/* 16. CYBER MATRIX */
.cyber-paper { background: #020617; color: #0ea5e9; border: 1px solid #0369a1; font-family: var(--font-mono); }
.cyber-head { display: flex; flex-direction: column; gap: 4px; border-bottom: 1px dashed #0369a1; padding-bottom: 12px; margin-bottom: 12px; }
.cyber-glitch { font-size: 10px; color: #38bdf8; letter-spacing: 2px; }
.cyber-head h2 { font-size: 20px; font-weight: 800; margin: 0; color: #bae6fd; }
.cyber-stat-bar { background: #0c4a6e; color: #e0f2fe; padding: 8px; font-size: 11px; font-weight: 700; margin-bottom: 12px; border-left: 4px solid #38bdf8; }
.cyber-box { border: 1px solid #0284c7; background: rgba(2,132,199,0.1); padding: 12px; font-size: 12px; margin-bottom: 12px; }
.c-danger { color: #f43f5e; font-size: 11px; margin-bottom: 6px; }

/* 17. MAGIC PROPHET (预言家日报) */
.prophet-paper { background: #fdf6e3; color: #292524; border: 4px double #78350f; font-family: 'Times New Roman', serif; box-shadow: inset 0 0 40px rgba(120,53,15,0.1); }
.prophet-head { text-align: center; border-bottom: 2px solid #78350f; padding-bottom: 8px; margin-bottom: 4px; }
.prophet-ornament { font-size: 24px; color: #78350f; margin-bottom: -4px; }
.prophet-head h1 { font-size: 32px; font-weight: 900; margin: 0; font-family: Georgia, serif; text-transform: uppercase; letter-spacing: -1px; text-shadow: 1px 1px 0px rgba(0,0,0,0.2); }
.prophet-meta { font-size: 11px; font-family: var(--font-mono); text-transform: uppercase; letter-spacing: 2px; color: #57534e; }
.prophet-divider { height: 1px; background: #78350f; margin-bottom: 12px; }
.prophet-lead { font-size: 22px; text-align: center; margin: 0 0 16px; border-bottom: 1px dashed #a8a29e; padding-bottom: 12px; font-style: italic; }
.prophet-grid { display: grid; grid-template-columns: 2fr 1fr; gap: 16px; }
.p-col-main, .p-col-side { display: flex; flex-direction: column; gap: 8px; }
.p-col-main h3, .p-col-side h3 { font-size: 14px; font-weight: 900; margin: 0; text-transform: uppercase; border-bottom: 2px solid #78350f; display: inline-block; padding-bottom: 2px; }
.p-desc { font-size: 13px; line-height: 1.6; text-align: justify; }
.p-red, .p-hear { font-size: 12px; border-bottom: 1px solid #d6d3d1; padding-bottom: 4px; }

/* 18. HOGWARTS LETTER (霍格沃茨录取信) */
.hogwarts-paper { background: #fefce8; color: #14532d; border: 1px solid #dcfce7; font-family: Georgia, serif; }
.hogwarts-head { text-align: center; border-bottom: 1px solid #86efac; padding-bottom: 16px; margin-bottom: 20px; }
.h-crest { width: 40px; height: 40px; margin: 0 auto 8px; border: 2px solid #14532d; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 24px; font-weight: bold; font-family: 'Times New Roman', serif; }
.hogwarts-head h2 { font-size: 16px; font-weight: 900; margin: 0 0 4px; text-transform: uppercase; letter-spacing: 1px; }
.hogwarts-head p { font-size: 11px; margin: 0; font-style: italic; }
.hogwarts-body p { font-size: 14px; line-height: 1.8; margin-bottom: 12px; }
.h-sig { margin-top: 24px; font-family: 'Brush Script MT', 'Lucida Handwriting', 'Segoe Print', cursive; font-size: 24px; line-height: 1; }
.h-sig span { font-family: Georgia, serif; font-size: 11px; color: #166534; font-style: italic; }
.wax-seal { position: absolute; bottom: 30px; right: 40px; width: 60px; height: 60px; background: #991b1b; border-radius: 50%; box-shadow: inset 0 0 10px rgba(0,0,0,0.5), 2px 4px 6px rgba(0,0,0,0.2); display: flex; align-items: center; justify-content: center; transform: rotate(-15deg); }
.seal-inner { width: 44px; height: 44px; border: 2px solid rgba(255,255,255,0.2); border-radius: 50%; display: flex; align-items: center; justify-content: center; color: rgba(255,255,255,0.8); font-size: 28px; font-family: 'Times New Roman', serif; }

/* 19. TELEGRAPH (复古电报) */
.telegraph-paper { background: #fef08a; color: #422006; font-family: 'Courier New', Courier, monospace; border: none; box-shadow: inset 0 0 20px rgba(161,98,7,0.2); }
.tele-stamp { position: absolute; top: 16px; right: 20px; border: 2px dashed #b45309; color: #b45309; padding: 4px 8px; transform: rotate(5deg); font-weight: bold; font-size: 12px; }
.tele-head { border-bottom: 2px solid #422006; padding-bottom: 12px; margin-bottom: 16px; }
.tele-head h2 { font-size: 24px; margin: 0; letter-spacing: -1px; }
.tele-head p { font-size: 12px; margin: 4px 0 0; }
.tele-body p { font-size: 14px; text-transform: uppercase; margin-bottom: 12px; line-height: 1.5; font-weight: 700; border-bottom: 1px solid rgba(66,32,6,0.1); padding-bottom: 4px; }

/* 20. BULLETIN (通缉令/警情通报) */
.bulletin-paper { background: #ffedd5; color: #451a03; border: 8px solid #78350f; outline: 2px solid #ffedd5; outline-offset: -12px; font-family: 'Times New Roman', serif; text-align: center; }
.bull-head h1 { font-size: 48px; font-weight: 900; margin: 0; letter-spacing: 4px; text-transform: uppercase; }
.bull-head h2 { font-size: 18px; margin: 4px 0 8px; }
.bull-head p { font-size: 12px; margin: 0; font-family: var(--font-mono); }
.bull-img-placeholder { width: 100%; height: 160px; border: 4px solid #78350f; margin: 16px 0; display: flex; align-items: center; justify-content: center; font-size: 18px; font-weight: bold; color: #a8a29e; background: #f5f5f4; }
.bull-body h3 { font-size: 24px; margin: 0 0 8px; text-transform: uppercase; }
.bull-body p { font-size: 14px; margin: 0 0 16px; }
.bull-details { border-top: 2px dashed #78350f; padding-top: 12px; text-align: left; }
.bull-details strong { display: block; margin-bottom: 6px; font-size: 16px; }
.bull-details div { font-size: 14px; margin-bottom: 4px; }

/* 21. BLUEPRINT (工业蓝图) */
.blueprint-paper { background: #1e3a8a; color: #eff6ff; font-family: 'Courier New', Courier, monospace; position: relative; border: 4px solid #eff6ff; }
.bp-grid-overlay { position: absolute; inset: 0; background-size: 20px 20px; background-image: linear-gradient(to right, rgba(255,255,255,0.1) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.1) 1px, transparent 1px); pointer-events: none; z-index: 0; }
.bp-content { position: relative; z-index: 1; }
.bp-head { border-bottom: 2px solid #eff6ff; padding-bottom: 12px; margin-bottom: 16px; }
.bp-head h1 { font-size: 24px; font-weight: 400; letter-spacing: 2px; margin: 0 0 8px; }
.bp-meta { display: flex; justify-content: space-between; font-size: 11px; }
.bp-box { border: 1px solid #eff6ff; padding: 12px; margin-bottom: 16px; background: rgba(30,58,138,0.5); }
.bp-lbl { font-size: 10px; border-bottom: 1px solid #eff6ff; display: inline-block; padding-bottom: 2px; margin-bottom: 8px; }
.bp-row { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
.bp-box h3 { font-size: 16px; margin: 0 0 8px; font-weight: 400; }
.bp-box p { font-size: 12px; margin: 0; }
.bp-box div { font-size: 11px; margin-bottom: 4px; }

/* 22. TERMINAL (复古终端机) */
.terminal-paper { background: #000; color: #22c55e; font-family: 'Courier New', Courier, monospace; border: 12px solid #111; border-radius: 8px; }
.term-head { margin-bottom: 20px; font-size: 14px; font-weight: bold; }
.term-body { font-size: 14px; line-height: 1.5; }
.term-cursor { display: inline-block; width: 10px; height: 16px; background: #22c55e; animation: blink 1s step-end infinite; }
@keyframes blink { 50% { opacity: 0; } }

/* 23. POLAROID (拍立得相纸) */
.polaroid-paper { background: #fafafa; border: none; border-radius: 2px; box-shadow: 0 10px 25px rgba(0,0,0,0.2); padding: 16px 16px 40px; }
.pol-photo { width: 100%; height: 320px; background: #111; margin-bottom: 16px; display: flex; align-items: center; justify-content: center; position: relative; overflow: hidden; }
.pol-inner { position: absolute; inset: 0; background: linear-gradient(45deg, #1e293b, #0f172a); padding: 20px; display: flex; align-items: flex-end; }
.pol-text-overlay { background: rgba(255,255,255,0.9); padding: 12px; border-radius: 4px; width: 100%; color: #111; }
.pol-text-overlay h3 { font-size: 16px; margin: 0 0 4px; }
.pol-text-overlay p { font-size: 12px; margin: 0 0 8px; }
.pol-reds div { font-size: 11px; color: #b91c1c; font-family: var(--font-mono); }
.pol-marker { text-align: center; font-family: 'Brush Script MT', 'Lucida Handwriting', 'Segoe Print', cursive; font-size: 24px; color: #111; transform: rotate(-2deg); }

/* 24. THEATRE TICKET (复古票根) */
.ticket-paper { background: #fef2f2; color: #881337; border: 2px solid #881337; border-radius: 8px; display: flex; flex-direction: row; padding: 0; overflow: visible; position: relative; }
.tick-stub { width: 60px; border-right: 2px dashed #881337; display: flex; align-items: center; justify-content: center; position: relative; }
.tick-stub::before, .tick-stub::after { content: ''; position: absolute; right: -10px; width: 20px; height: 20px; background: rgba(0,0,0,0.65); border-radius: 50%; }
.tick-stub::before { top: -10px; }
.tick-stub::after { bottom: -10px; }
.tick-vert { transform: rotate(-90deg); font-weight: 900; letter-spacing: 4px; font-size: 18px; white-space: nowrap; }
.tick-main { flex: 1; padding: 20px; }
.tick-head { text-align: center; font-weight: 900; letter-spacing: 2px; font-size: 14px; border-bottom: 2px solid #881337; padding-bottom: 8px; margin-bottom: 12px; }
.tick-title { font-size: 32px; font-family: 'Times New Roman', serif; text-align: center; margin: 0 0 4px; }
.tick-date { text-align: center; font-family: var(--font-mono); font-size: 12px; margin-bottom: 16px; }
.tick-feat { font-size: 14px; font-weight: bold; background: #881337; color: #fff; padding: 4px 8px; display: inline-block; margin-bottom: 8px; }
.tick-desc { font-size: 12px; margin-bottom: 16px; }
.tick-row { display: flex; justify-content: space-between; font-family: var(--font-mono); font-size: 11px; border-top: 1px solid #881337; padding-top: 8px; }

/* 25. SCROLL (东方卷轴) */
.scroll-paper { background: #fffcf0; color: #27272a; border-top: 12px solid #b91c1c; border-bottom: 12px solid #b91c1c; font-family: 'STKaiti', 'KaiTi', '楷体', 'BiauKai', serif; padding: 32px; }
.scroll-head { display: flex; align-items: center; gap: 16px; margin-bottom: 24px; border-bottom: 1px solid #d4d4d8; padding-bottom: 16px; }
.scroll-inkan { width: 32px; height: 32px; border: 2px solid #b91c1c; color: #b91c1c; font-size: 18px; display: flex; align-items: center; justify-content: center; font-weight: bold; border-radius: 4px; }
.scroll-head h1 { font-size: 28px; font-weight: normal; margin: 0; letter-spacing: 2px; }
.scroll-date { font-size: 14px; color: #71717a; margin-left: auto; }
.s-lead { font-size: 18px; font-weight: bold; margin: 0 0 12px; }
.s-desc { font-size: 15px; line-height: 1.8; margin: 0 0 20px; }
.s-danger p { font-size: 14px; color: #b91c1c; margin: 4px 0; }

/* 26. CLASSIFIED FILE (冷战绝密档案) */
.classified-paper { background: #fde68a; color: #111; border: none; position: relative; border-radius: 0 8px 8px 8px; margin-top: 20px; }
.class-tab { position: absolute; top: -20px; left: 0; background: #fde68a; padding: 4px 16px; font-family: var(--font-mono); font-size: 10px; border-radius: 8px 8px 0 0; }
.class-top-stamp { font-size: 32px; font-family: 'Times New Roman', serif; font-weight: 900; color: #b91c1c; border: 4px double #b91c1c; padding: 4px 12px; display: inline-block; transform: rotate(-5deg); margin-bottom: 20px; letter-spacing: 2px; opacity: 0.8; }
.class-body h2 { font-size: 20px; font-family: var(--font-mono); border-bottom: 2px solid #111; padding-bottom: 4px; margin: 0 0 8px; }
.class-body p { font-family: var(--font-mono); font-size: 13px; margin: 0 0 12px; }
.class-redact-box { background: rgba(0,0,0,0.05); padding: 12px; border-left: 4px solid #111; margin-bottom: 20px; }
.class-list { list-style: square; padding-left: 20px; font-family: var(--font-mono); font-size: 12px; color: #b91c1c; font-weight: bold; }
.class-clip { position: absolute; top: 10px; right: 20px; width: 12px; height: 40px; border: 2px solid #94a3b8; border-radius: 10px; background: transparent; transform: rotate(15deg); }

/* 27. VINYL RECORD (黑胶唱片) */
.vinyl-paper { background: #f97316; color: #fffcf0; border: none; padding: 0; display: flex; position: relative; overflow: hidden; }
.vinyl-record-bg { position: absolute; right: -50px; top: 50%; transform: translateY(-50%); width: 200px; height: 200px; background: radial-gradient(circle, #111 30%, #333 40%, #111 50%, #333 60%, #111 70%); border-radius: 50%; border: 4px solid #111; z-index: 0; }
.vinyl-sleeve { position: relative; z-index: 1; padding: 32px; width: 80%; background: linear-gradient(135deg, #f97316, #ea580c); box-shadow: 10px 0 20px rgba(0,0,0,0.5); }
.v-band-name { font-size: 10px; font-weight: bold; letter-spacing: 2px; margin-bottom: 4px; }
.v-album { font-size: 32px; font-family: Georgia, serif; font-weight: 900; font-style: italic; margin: 0 0 24px; text-shadow: 2px 2px 0 rgba(0,0,0,0.2); }
.v-tracklist h3 { font-size: 14px; font-weight: bold; border-bottom: 1px solid #fffcf0; padding-bottom: 4px; margin: 16px 0 8px; }
.v-tracklist p { font-size: 12px; margin: 4px 0; }

/* 28. TAROT (神秘塔罗牌) */
.tarot-paper { background: #1e1b4b; color: #fef08a; border: 12px solid #312e81; padding: 12px; text-align: center; font-family: 'Times New Roman', serif; }
.tarot-border { border: 2px solid #fef08a; padding: 16px; height: 100%; display: flex; flex-direction: column; }
.tarot-num { font-size: 18px; font-weight: bold; letter-spacing: 4px; margin-bottom: 12px; }
.tarot-art { height: 80px; display: flex; flex-direction: column; align-items: center; justify-content: center; border: 1px solid rgba(254,240,138,0.3); margin-bottom: 16px; }
.t-moon { width: 30px; height: 30px; border-radius: 50%; box-shadow: -6px 6px 0 0 #fef08a; margin-bottom: 8px; }
.t-stars { font-size: 12px; letter-spacing: 8px; }
.tarot-body h2 { font-size: 20px; text-transform: uppercase; margin: 0 0 8px; font-weight: normal; letter-spacing: 2px; }
.tarot-body p { font-size: 13px; line-height: 1.6; margin: 0 0 16px; font-style: italic; }
.t-fate span { font-size: 10px; text-transform: uppercase; border-bottom: 1px solid #fef08a; padding-bottom: 2px; display: inline-block; margin-bottom: 6px; }
.t-fate div { font-size: 12px; margin-bottom: 4px; }
.tarot-bottom { margin-top: auto; font-size: 16px; font-weight: bold; letter-spacing: 2px; border-top: 2px solid #fef08a; padding-top: 12px; }

/* 29. BANK NOTE (复古钞票) */
.bank-paper { background: #f0fdf4; color: #14532d; border: 12px solid #14532d; padding: 8px; font-family: Georgia, serif; text-align: center; }
.bank-border { border: 2px solid #14532d; padding: 16px; position: relative; height: 100%; display: flex; flex-direction: column; }
.bank-corners { position: absolute; top: -6px; left: -6px; right: -6px; display: flex; justify-content: space-between; font-size: 12px; font-weight: bold; background: #f0fdf4; padding: 0 4px; }
.bank-corners.bottom { top: auto; bottom: -6px; font-size: 10px; font-family: var(--font-mono); }
.bank-head h2 { font-size: 24px; font-weight: 900; letter-spacing: 2px; margin: 0 0 4px; }
.bank-head p { font-size: 8px; letter-spacing: 1px; margin: 0 0 16px; }
.bank-center-art { border: 2px solid #14532d; border-radius: 40px; padding: 20px; display: flex; align-items: center; gap: 16px; text-align: left; background: repeating-linear-gradient(45deg, rgba(20,83,45,0.05), rgba(20,83,45,0.05) 2px, transparent 2px, transparent 4px); }
.b-seal { width: 60px; height: 60px; border: 4px solid #14532d; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 32px; font-weight: 900; background: #fff; }
.b-text h3 { font-size: 14px; margin: 0 0 4px; text-transform: uppercase; }
.b-text p { font-size: 11px; margin: 0; line-height: 1.4; }

/* 30. PASSPORT (国际护照) */
.passport-paper { background: #f8fafc; color: #0f172a; border-left: 20px solid #1e40af; font-family: var(--font-mono); }
.pass-head { display: flex; align-items: center; gap: 12px; border-bottom: 2px solid #cbd5e1; padding-bottom: 12px; margin-bottom: 16px; }
.pass-crest { font-size: 24px; }
.pass-head h1 { font-size: 24px; font-weight: bold; margin: 0; letter-spacing: 2px; }
.pass-head p { font-size: 12px; color: #64748b; margin: 0; }
.pass-inner { display: flex; gap: 16px; margin-bottom: 20px; }
.pass-photo { width: 100px; height: 120px; background: #e2e8f0; border: 1px solid #cbd5e1; display: flex; align-items: center; justify-content: center; }
.pass-data { flex: 1; display: flex; flex-direction: column; gap: 8px; }
.p-row { font-size: 10px; color: #64748b; display: flex; gap: 12px; }
.p-row strong { font-size: 12px; color: #0f172a; display: block; margin-top: 2px; }
.pass-mrz { background: #f1f5f9; padding: 10px; font-size: 12px; font-weight: bold; letter-spacing: 2px; border-radius: 4px; }

/* 31. STEAMPUNK (蒸汽朋克) */
.steampunk-paper { background: #292524; color: #fcd34d; border: 8px solid #b45309; position: relative; font-family: 'Times New Roman', serif; }
.steam-border { border: 2px solid #fcd34d; padding: 20px; position: relative; height: 100%; display: flex; flex-direction: column; }
.rivet { position: absolute; width: 12px; height: 12px; background: radial-gradient(circle, #fcd34d, #b45309); border-radius: 50%; box-shadow: 2px 2px 4px rgba(0,0,0,0.5); }
.r1 { top: -6px; left: -6px; } .r2 { top: -6px; right: -6px; } .r3 { bottom: -6px; left: -6px; } .r4 { bottom: -6px; right: -6px; }
.steam-head { text-align: center; border-bottom: 1px dashed #fcd34d; padding-bottom: 16px; margin-bottom: 16px; }
.steam-head h1 { font-size: 28px; font-weight: 900; margin: 0; letter-spacing: 2px; text-shadow: 2px 2px 0 #78350f; }
.steam-head p { font-size: 12px; margin: 4px 0 0; font-family: var(--font-mono); }
.steam-body { position: relative; }
.s-gear-bg { position: absolute; right: 0; bottom: 0; width: 100px; height: 100px; border: 10px dashed rgba(252,211,77,0.1); border-radius: 50%; z-index: 0; pointer-events: none; }
.steam-body h2 { font-size: 16px; margin: 0 0 8px; position: relative; z-index: 1; }
.steam-body p { font-size: 13px; margin: 0 0 12px; line-height: 1.5; position: relative; z-index: 1; }

/* 32. ROYAL DECREE (王室诏书) */
.decree-paper { background: #fefce8; color: #4c0519; border: 1px solid #fde047; font-family: Georgia, serif; box-shadow: inset 0 0 30px rgba(253,224,71,0.2); }
.decree-head { text-align: center; margin-bottom: 24px; }
.d-crown { font-size: 32px; color: #ca8a04; margin-bottom: -8px; }
.decree-head h1 { font-size: 36px; font-weight: normal; margin: 0; font-style: italic; }
.decree-head p { font-size: 14px; margin: 0; color: #831843; }
.d-dropcap { font-size: 14px; line-height: 1.8; margin-bottom: 16px; }
.d-first-letter { float: left; font-size: 48px; line-height: 0.8; padding-right: 8px; color: #ca8a04; font-family: 'Times New Roman', serif; }
.decree-body p { font-size: 14px; line-height: 1.8; margin-bottom: 12px; }
.d-list { list-style: none; padding: 0; font-size: 13px; margin: 0 0 24px; border-left: 2px solid #ca8a04; padding-left: 12px; }
.d-list li { margin-bottom: 4px; }
.d-sign { display: flex; align-items: center; gap: 16px; font-style: italic; font-size: 16px; }
.d-wax { width: 40px; height: 40px; background: #9f1239; border-radius: 50%; box-shadow: inset 0 0 10px rgba(0,0,0,0.5); position: relative; }
.d-wax::after { content: 'C'; position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; color: rgba(255,255,255,0.5); font-family: 'Times New Roman', serif; font-size: 20px; }

/* 底部操作栏 */
.modal-control-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--c-border);
  background: var(--c-bg-card);
  border-radius: 0 0 var(--c-radius-xl) var(--c-radius-xl);
}

.footer-left-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--slate-gray-light);
}

.type-pill {
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--c-bg-subtle);
  font-family: var(--font-mono);
}

.footer-btn-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-tool {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  border-radius: var(--c-radius-md);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-tool:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-primary);
  color: var(--c-primary);
}

.btn-tool-close {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 14px;
  border-radius: var(--c-radius-md);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-tool-close:hover {
  filter: brightness(1.1);
}

.brief-modal-fade-enter-active,
.brief-modal-fade-leave-active {
  transition: all 0.25s ease-out;
}

.brief-modal-fade-enter-from,
.brief-modal-fade-leave-to {
  opacity: 0;
  transform: scale(0.94) translateY(10px);
}
</style>
