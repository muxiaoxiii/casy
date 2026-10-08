<script setup lang="ts">
/**
 * FilePreviewPanel —— 文件预览侧板（index-v2 preview-panel 规格）
 *
 * - 440px 右侧滑出；图片经资产协议真实预览，PDF/其他给类型化占位+打开引导
 * - 头部：文件名+路径+关闭；操作：打开 / Finder 定位
 * - 信息栏：大小 / 类型 / 登记时间
 */
import { computed } from 'vue'
import { ElMessage } from 'element-plus'
import { Close, FolderOpened } from '../../../shared/icons'
import { convertFileSrc } from '@tauri-apps/api/core'
import { casyContext } from '../../../core/plugin/context'

export interface PreviewFile {
  id?: string
  fileName: string
  filePath: string
  fileType?: string | null
  fileSize?: number | null
  createdAt?: string | null
}

const props = defineProps<{
  modelValue: boolean
  file: PreviewFile | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
}>()

const ext = computed(() => {
  const f = props.file?.filePath || props.file?.fileName || ''
  const m = f.split('.').pop()
  return (m || '').toLowerCase()
})
const kind = computed(() => {
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'heic'].includes(ext.value)) return 'img'
  if (ext.value === 'pdf') return 'pdf'
  if (['doc', 'docx', 'wps', 'rtf'].includes(ext.value)) return 'doc'
  if (['zip', 'rar', '7z'].includes(ext.value)) return 'zip'
  return 'other'
})
const isImage = computed(() => kind.value === 'img')
const srcUrl = computed(() =>
  isImage.value && props.file ? convertFileSrc(props.file.filePath) : '',
)

function fmtSize(b?: number | null): string {
  if (!b) return '—'
  if (b > 1024 * 1024) return (b / 1024 / 1024).toFixed(1) + ' MB'
  return Math.max(1, Math.round(b / 1024)) + ' KB'
}

async function openDefault() {
  if (!props.file) return
  const r = await casyContext.files.openDefault(props.file.filePath)
  if (!r.ok) ElMessage.error(r.error || '打开失败')
}
async function reveal() {
  if (!props.file) return
  const r = await casyContext.files.reveal(props.file.filePath)
  if (!r.ok) ElMessage.error(r.error || '定位失败')
}
</script>

<template>
  <Teleport to="body">
    <div class="pp-mask" :class="{ open: modelValue && file }" @click.self="emit('update:modelValue', false)">
      <aside class="ui-col preview-panel" style="gap:0" :class="{ open: modelValue && file }">
        <!-- 头部 -->
        <div class="ui-row pp-head" style="gap:10px">
          <div class="pp-title">
            <div class="ui-truncate n">{{ file?.fileName }}</div>
            <div class="p">{{ file?.filePath }}</div>
          </div>
          <div class="pp-actions">
            <el-button size="small" text :icon="FolderOpened" title="Finder 定位" @click="reveal" />
            <button class="ui-row pp-close" title="关闭" @click="emit('update:modelValue', false)">
              <el-icon><Close /></el-icon>
            </button>
          </div>
        </div>

        <!-- 预览主体 -->
        <div class="ui-row pp-stage">
          <template v-if="file">
            <img
              v-if="isImage"
              :src="srcUrl"
              :alt="file.fileName"
              class="pv-img"
              @error="e => ((e.target as HTMLImageElement).style.display = 'none')"
            />
            <div v-else class="pv-placeholder">
              <div class="ui-row pv-ico" :class="kind">
                <span class="pv-ext">{{ ext || '文件' }}</span>
              </div>
              <div class="pv-name">{{ file.fileName }}</div>
              <div class="pv-size">{{ fmtSize(file.fileSize) }}{{ kind === 'pdf' ? ' · PDF' : '' }}</div>
              <div v-if="kind !== 'pdf'" class="pv-note">点击下方「打开」使用系统应用查看</div>
              <div v-else class="pv-note">PDF 应用内预览将在后续版本提供，当前请「打开」查看</div>
            </div>
          </template>
        </div>

        <!-- 信息栏 -->
        <div class="ui-col--tight pp-info">
          <div class="ui-row pi-row"><span class="k">类型</span><span class="v">{{ ext.toUpperCase() || '—' }}</span></div>
          <div class="ui-row pi-row"><span class="k">大小</span><span class="v">{{ fmtSize(file?.fileSize) }}</span></div>
          <div class="ui-row pi-row"><span class="k">登记时间</span><span class="v">{{ file?.createdAt || '—' }}</span></div>
        </div>

        <!-- 底部操作 -->
        <div class="ui-row pp-footer" style="gap:6px">
          <el-button type="primary" @click="openDefault">打开文件</el-button>
          <el-button :icon="FolderOpened" @click="reveal">Finder 定位</el-button>
        </div>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.pp-mask {
  position: fixed;
  inset: 0;
  z-index: 2000;
  background: rgba(20, 24, 35, 0.18);
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--motion-fast) var(--ease-out);
}
.pp-mask.open {
  opacity: 1;
  pointer-events: auto;
}
.preview-panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: min(440px, 92vw);
  background: var(--c-surface);
  border-left: 1px solid var(--c-border);
  transform: translateX(100%);
  transition: transform var(--motion-base) var(--ease-out);
  box-shadow: -8px 0 32px rgba(20, 24, 35, 0.12);
}
.preview-panel.open {
  transform: translateX(0);
}

.pp-head {
  padding: 12px 16px;
  border-bottom: 1px solid var(--c-border);
  flex-shrink: 0;
}
.pp-title { flex: 1; min-width: 0; }
.pp-title .n {
  font-size: 13px; font-weight: 600; color: var(--c-text);
}
.pp-title .p {
  font-size: 10.5px; color: var(--c-text-secondary); font-family: var(--font-mono);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 2px;
}
.pp-close {
  width: 26px; height: 26px; border-radius: var(--c-radius-btn);
  justify-content: center;
  color: var(--c-text-secondary); cursor: pointer; border: none; background: transparent;
}
.pp-close:hover { background: var(--gray-100); color: var(--c-text); }

.pp-stage {
  flex: 1; min-height: 0;
  background: #F1F2F5;
  justify-content: center;
  overflow: auto;
}
.pv-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  box-shadow: var(--shadow-md);
}
.pv-placeholder {
  display: flex; flex-direction: column; align-items: center; gap: 10px;
  text-align: center; max-width: 300px;
}
.pv-ico {
  width: 56px; height: 56px; border-radius: 14px;
  justify-content: center;
  font-weight: 700; font-size: 14px;
}
.pv-ico.pdf { background: var(--c-danger-light); color: var(--c-danger); }
.pv-ico.doc { background: var(--c-primary-light); color: var(--c-primary); }
.pv-ico.zip { background: var(--c-warning-light); color: var(--c-warning); }
.pv-ico.other { background: var(--gray-100); color: var(--gray-500); }
.pv-name { font-size: 12.5px; font-weight: 600; color: var(--c-text); }
.pv-size { font-size: 11px; color: var(--c-text-secondary); font-family: var(--font-mono); }
.pv-note { font-size: 11.5px; color: var(--c-text-secondary); }

.pp-info {
  flex-shrink: 0; border-top: 1px solid var(--c-border);
  padding: 12px 16px;
  background: var(--c-surface);
}
.pi-row {
  height: 26px; font-size: 11.5px;
}
.pi-row .k { width: 76px; color: var(--c-text-secondary); flex-shrink: 0; font-size: 10.5px; }
.pi-row .v {
  color: var(--c-text-regular); font-family: var(--font-mono); font-size: 10.5px;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.pp-footer {
  padding: 10px 16px; border-top: 1px solid var(--c-border); flex-shrink: 0;
}

@media (prefers-reduced-motion: reduce) {
  .preview-panel { transition: none; }
  .pp-mask { transition: none; }
}
</style>
