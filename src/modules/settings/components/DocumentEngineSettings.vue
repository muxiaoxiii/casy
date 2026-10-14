<script setup>
/**
 * 文档引擎状态（O1）
 *
 * DocumentEngineStatus 一直回传 9 个可用性字段，但此前只有 CaseFilesView 的
 * "引擎未就绪：{missing}" 用到其中两个——用户无法知道缺的是渲染器、坐标模型、
 * 韩文模型还是 layout 模型，也不知道该跑哪条命令补。这里集中展示 + 给下载指引。
 */
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { Refresh, WarningFilled } from '../../../shared/icons'

const status = ref(null)
const loading = ref(false)
const error = ref('')
const copied = ref('')

const CHECKS = [
  { key: 'rendererAvailable', label: '页面渲染器（pdftoppm）', hint: '扫描件/PDF 逐页渲染；macOS: brew install poppler，Windows 由 prepare-windows-renderer.ps1 提供' },
  { key: 'coordinateModelAvailable', label: '坐标模型（版面几何）', hint: '识别框与原件对齐；随 OCR 模型档位一起准备' },
  { key: 'koreanModelAvailable', label: '韩文识别模型', hint: '韩文候选识别；npm run prepare:ocr-models 后放入 models/korean-ppocrv5-mobile' },
  { key: 'layoutModelAvailable', label: '版面分析模型（PP-DocLayout）', hint: '表格/印章检测与阅读顺序；npm run prepare:ocr-models 后放入 models/layout' },
  { key: 'ovisModelAvailable', label: 'Ovis 视觉模型（可选）', hint: '复杂版面兜底；缺失时自动降级，不影响主流程' },
  { key: 'searchablePdfAvailable', label: '可搜索 PDF 生成', hint: 'OCR 后生成可搜索 PDF 与文字备份' },
]

const PROVIDER_LABELS = { builtin: '内置引擎（本地）', myna: '远端 Myna' }

async function load() {
  loading.value = true
  error.value = ''
  const result = await tauriCallSafe('get_document_engine_status', {}, { silent: true })
  loading.value = false
  if (result.ok && result.data) status.value = result.data
  else { status.value = null; error.value = result.error || '引擎状态读取失败' }
}

function overall() {
  if (!status.value) return 'unknown'
  return status.value.available ? 'ready' : 'broken'
}

async function copyFixHint() {
  const text = [
    '# 一次性准备全部运行时模型（OCR 档位 + embedding + 字体）',
    'npm run runtime:prepare',
    '',
    '# 只补 OCR 模型（small 档默认）',
    'npm run prepare:ocr-models',
    '',
    '# 只补 embedding 模型',
    'npm run prepare:embed-models',
  ].join('\n')
  try {
    await navigator.clipboard.writeText(text)
    copied.value = true
    ElMessage.success('已复制准备命令')
    setTimeout(() => { copied.value = false }, 2000)
  } catch {
    ElMessage.warning('复制失败，请手动执行：npm run runtime:prepare')
  }
}

onMounted(load)
</script>

<template>
  <section class="engine-settings ui-col">
    <header class="engine-head ui-row ui-row--between">
      <div>
        <h3>文档引擎</h3>
        <p class="engine-sub">OCR、版面分析与可搜索 PDF 的本地运行环境。模型文件按需准备，不随代码分发。</p>
      </div>
      <el-button :icon="Refresh" circle :loading="loading" title="重新探测" aria-label="重新探测" @click="load" />
    </header>

    <el-alert v-if="error" :title="error" type="error" :closable="false">
      <el-button text @click="load">重试</el-button>
    </el-alert>

    <template v-else-if="status">
      <div class="engine-overall" :class="overall()">
        <el-icon :size="18"><WarningFilled v-if="overall() === 'broken'" /><component :is="overall() === 'ready' ? 'CircleCheck' : 'QuestionFilled'" v-else /></el-icon>
        <div>
          <strong>{{ overall() === 'ready' ? '引擎就绪' : '引擎未就绪' }}</strong>
          <span v-if="status.version"> · 版本 {{ status.version }}</span>
          <span v-if="status.executable" class="engine-path">{{ status.executable }}</span>
        </div>
      </div>

      <dl class="engine-facts">
        <div><dt>识别方式</dt><dd>{{ PROVIDER_LABELS[status.provider] || status.provider || '内置引擎' }}<span v-if="status.providerEndpoint" class="engine-endpoint">{{ status.providerEndpoint }}</span></dd></div>
      </dl>

      <ul class="engine-checks">
        <li v-for="check in CHECKS" :key="check.key" :class="{ ok: status[check.key], bad: !status[check.key] }">
          <span class="check-mark">{{ status[check.key] ? '✓' : '✕' }}</span>
          <span class="check-body">
            <strong>{{ check.label }}</strong>
            <small v-if="!status[check.key]">{{ check.hint }}</small>
          </span>
        </li>
      </ul>

      <div v-if="status.missing?.length" class="engine-missing">
        <p>缺失组件：{{ status.missing.join('、') }}</p>
        <el-button type="primary" size="small" @click="copyFixHint">{{ copied ? '已复制' : '复制准备命令' }}</el-button>
        <span class="engine-note">模型体积较大（OCR small 档约 30MB、embedding 约 135MB、layout 约 124MB），按需下载而非随安装包分发。</span>
      </div>
      <p v-else class="engine-all-good">全部组件就绪，无需额外准备。</p>
    </template>

    <p v-else-if="loading" class="engine-loading">正在探测本地引擎…</p>
  </section>
</template>

<style scoped>
.engine-settings { gap: 16px; }
.engine-head h3 { margin: 0; font-size: 15px; }
.engine-sub { margin: 4px 0 0; font-size: 12px; color: var(--c-text-secondary); line-height: 1.6; }
.engine-overall { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 6px; font-size: 13px; }
.engine-overall.ready { background: var(--c-success-light); color: var(--c-success); }
.engine-overall.broken { background: var(--c-danger-light); color: var(--c-danger); }
.engine-overall.unknown { background: var(--c-bg-subtle); color: var(--c-text-secondary); }
.engine-path { display: block; font-size: 11px; color: var(--c-text-secondary); word-break: break-all; }
.engine-facts { margin: 0; }
.engine-facts dt { font-size: 12px; color: var(--c-text-secondary); }
.engine-facts dd { margin: 2px 0 0; font-size: 13px; }
.engine-endpoint { margin-left: 8px; font-size: 11px; color: var(--c-text-secondary); }
.engine-checks { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
.engine-checks li { display: flex; gap: 8px; padding: 8px 10px; border: 1px solid var(--c-border-light); border-radius: 6px; font-size: 13px; }
.engine-checks li.ok { color: var(--c-text-regular); }
.engine-checks li.bad { border-color: var(--c-warning); background: var(--c-warning-light); }
.check-mark { font-weight: 700; }
.engine-checks li.ok .check-mark { color: var(--c-success); }
.engine-checks li.bad .check-mark { color: var(--c-warning); }
.check-body { display: flex; flex-direction: column; gap: 2px; }
.check-body small { font-size: 11px; color: var(--c-text-secondary); line-height: 1.6; }
.engine-missing { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; padding: 10px 12px; border-radius: 6px; background: var(--c-bg-subtle); font-size: 12px; }
.engine-missing p { margin: 0; color: var(--c-text-regular); }
.engine-note { color: var(--c-text-secondary); line-height: 1.6; }
.engine-all-good, .engine-loading { font-size: 12px; color: var(--c-text-secondary); }
</style>
