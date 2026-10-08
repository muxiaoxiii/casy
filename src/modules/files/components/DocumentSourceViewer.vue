<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { ArrowLeft, ArrowRight, ZoomIn, ZoomOut, Refresh, EditPen, Check, Close } from '../../../shared/icons'
import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../../../core/tauriBridge'
import DocumentMarkdown from '../../../shared/components/DocumentMarkdown.vue'
import type { DocumentPageView, SourceLocation } from '../../../types/documentRetrieval'

const props = defineProps<{ modelValue:boolean; fileId:string; jobId:string; initialPage?:number; locations?:SourceLocation[] }>()
const emit=defineEmits<{(e:'update:modelValue',value:boolean):void}>()
const view=ref<DocumentPageView|null>(null), page=ref(1), busy=ref(false), error=ref(''), zoom=ref(100)
const mode=ref('preview'), selected=ref<number|null>(null)
const currentJob = ref(props.jobId), correcting=ref(false), correction=ref(''), savingCorrection=ref(false)
let revision=0
const marked=computed(()=> new Set<number>((props.locations||[]).flatMap(l=>l.pageNumber===page.value && l.regionIndex!==null?[l.regionIndex]:[])))
const hitPages=computed(()=> [...new Set((props.locations||[]).map(l=>l.pageNumber))])
watch(()=>[props.modelValue,props.fileId,props.jobId,props.initialPage],()=>{
  revision++; view.value=null; error.value=''; busy.value=false; selected.value=null; zoom.value=100
  currentJob.value=props.jobId; correcting.value=false
  if(props.modelValue) {page.value=props.initialPage||1; void load()}
},{immediate:true})
onUnmounted(()=>revision++)
async function load() {
  const current=++revision
  busy.value=true; error.value=''; view.value=null; selected.value=null
  try {
    const result=await tauriCallSafe('get_document_page',{fileId:props.fileId,jobId:currentJob.value,pageNumber:page.value})
    if(current!==revision)return
    if(!result.ok||!result.data)throw new Error(result.error||'无法读取页面')
    view.value=result.data
    await nextTick()
    const first=marked.value.values().next().value
    if(first!==undefined)focusRegion(first)
  }catch(e){if(current===revision)error.value=e instanceof Error?e.message:String(e)}
  finally{if(current===revision)busy.value=false}
}
function go(number:number){if(savingCorrection.value||correcting.value)return;page.value=number; void load()}
function editRegion(){if(selected.value===null||!view.value)return;correction.value=view.value.regions[selected.value].text;correcting.value=true}
async function saveCorrection(){
  if(selected.value===null||!view.value)return
  savingCorrection.value=true
  try {
    const result=await tauriCallSafe('correct_document_region',{fileId:props.fileId,jobId:currentJob.value,pageNumber:page.value,regionIndex:selected.value,expectedText:view.value.regions[selected.value].text,text:correction.value})
    if(!result.ok||!result.data){ElMessage.error(result.error||'校订失败');return}
    currentJob.value=result.data;correcting.value=false
    await load();ElMessage.success('校订版本已保存，检索将使用新版本')
  }finally{savingCorrection.value=false}
}
function changePage(value:number|undefined){if(value)go(value)}
function regionStyle(bbox:number[]){
  const width=view.value?.width||1,height=view.value?.height||1
  return {left:`${bbox[0]/width*100}%`,top:`${bbox[1]/height*100}%`,width:`${(bbox[2]-bbox[0])/width*100}%`,height:`${(bbox[3]-bbox[1])/height*100}%`}
}
function focusRegion(index:number){
  if(correcting.value||savingCorrection.value)return
  selected.value=index
  document.getElementById(`source-region-${index}`)?.scrollIntoView({block:'center',inline:'nearest',behavior:'smooth'})
}
</script>

<template>
  <el-dialog :model-value="modelValue" @update:model-value="emit('update:modelValue',$event)" :close-on-click-modal="!correcting&&!savingCorrection" :close-on-press-escape="!correcting&&!savingCorrection" :show-close="!correcting&&!savingCorrection" :title="view?.fileName||'文档对照'" width="min(1240px, calc(100vw - 24px))" top="3vh" append-to-body class="document-source-dialog" destroy-on-close>
    <div class="source-toolbar">
      <div class="source-navigation">
      <el-button :icon="ArrowLeft" aria-label="上一页" title="上一页" :disabled="busy||correcting||page<=1" @click="go(page-1)" />
      <el-input-number :model-value="page" :min="1" :max="view?.totalPages||page" :disabled="busy||correcting||!view" :controls="false" aria-label="页码" @change="changePage" />
      <span>/ {{ view?.totalPages||'…' }}</span>
      <el-button :icon="ArrowRight" aria-label="下一页" title="下一页" :disabled="busy||correcting||!view||page>=view.totalPages" @click="go(page+1)" />
      </div>
      <div class="source-zoom">
      <el-button :icon="ZoomOut" aria-label="缩小" title="缩小" :disabled="zoom<=50" @click="zoom-=25" />
      <span class="zoom-value">{{ zoom }}%</span>
      <el-button :icon="ZoomIn" aria-label="放大" title="放大" :disabled="zoom>=250" @click="zoom+=25" />
      </div>
      <div v-if="hitPages.length>1" class="hit-pages"><el-button v-for="number in hitPages" :key="number" link :type="number===page?'primary':undefined" :disabled="busy" @click="go(number)">第 {{ number }} 页</el-button></div>
    </div>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <el-button v-if="error" :icon="Refresh" @click="load">重试</el-button>
    <div v-if="busy" class="source-loading" role="status">正在读取第 {{ page }} 页…</div>
    <div v-else-if="view" class="source-panes" :class="{'text-only':!view.imageData}">
      <div v-if="view.imageData" class="source-image-pane">
        <div class="source-page-image" :style="{width:`${zoom}%`}">
          <img :src="view.imageData" :alt="`${view.fileName} 第 ${page} 页`" />
          <button v-for="(region,index) in view.regions" :id="`source-region-${index}`" :key="index" class="source-region" :class="{marked:marked.has(index),selected:selected===index}" :style="regionStyle(region.bbox)" :aria-label="region.text" :title="region.text" @click="focusRegion(index)" />
        </div>
      </div>
      <div class="source-text-pane">
        <el-radio-group v-model="mode" size="small"><el-radio-button value="preview">排版预览</el-radio-button><el-radio-button value="text">识别文字</el-radio-button><el-radio-button value="markdown">Markdown 源码</el-radio-button></el-radio-group>
        <el-button v-if="selected!==null&&view.regions.length&&!correcting" :icon="EditPen" title="校订选中区域" aria-label="校订选中区域" @click="editRegion" />
        <div v-if="correcting" class="source-correction">
          <el-input v-model="correction" type="textarea" :rows="5" :disabled="savingCorrection" aria-label="校订文字" />
          <el-button :icon="Check" :loading="savingCorrection" @click="saveCorrection">保存校订</el-button>
          <el-button :icon="Close" :disabled="savingCorrection" @click="correcting=false">取消</el-button>
        </div>
        <div v-if="mode==='text'&&view.regions.length" class="source-text-lines">
          <button v-for="(region,index) in view.regions" :key="index" :class="{marked:marked.has(index),selected:selected===index}" @click="focusRegion(index)">{{ region.text }}</button>
        </div>
        <DocumentMarkdown v-else-if="mode==='preview'" class="source-markdown" :markdown="view.markdown" :file-id="view.fileId" :job-id="view.jobId" />
        <pre v-else>{{ view.markdown }}</pre>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.source-toolbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:12px}.source-toolbar .el-button+.el-button{margin-left:0}.source-toolbar .el-input-number{width:64px}.zoom-value{width:42px;text-align:center}.hit-pages{display:flex;gap:8px;margin-left:auto}
.source-navigation,.source-zoom{display:flex;align-items:center;gap:8px;flex-shrink:0}
.source-correction{margin:12px 0}.source-correction .el-textarea{margin-bottom:8px}
.source-panes{display:grid;grid-template-columns:minmax(0,1.4fr) minmax(260px,1fr);height:72vh;min-height:0;border-top:1px solid var(--c-border)}.source-panes.text-only{grid-template-columns:1fr}.source-image-pane{overflow:auto;background:#e5e7eb;padding:16px}.source-page-image{position:relative;margin:0 auto;min-width:50%;line-height:0}.source-page-image img{width:100%;height:auto;display:block}.source-region{position:absolute;border:1px solid transparent;background:transparent;cursor:pointer;padding:0}.source-region:hover,.source-region:focus-visible{border-color:#0284c7;background:#0ea5e922}.source-region.marked{background:#eab30844;border-color:#ca8a04}.source-region.selected{background:#0ea5e944;border:2px solid #0284c7}
.source-text-pane{overflow:auto;padding:16px;min-width:0}.source-text-lines{display:flex;flex-direction:column;gap:4px;margin-top:14px}.source-text-lines button{text-align:left;padding:6px 8px;background:transparent;border:0;border-left:3px solid transparent;color:var(--c-text);font:inherit;line-height:1.65;overflow-wrap:anywhere;cursor:pointer}.source-text-lines button.marked{background:#eab30822;border-left-color:#ca8a04}.source-text-lines button.selected{background:#0ea5e922;border-left-color:#0284c7}.source-text-pane pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.7}.source-loading{height:72vh;display:grid;place-items:center;color:var(--c-text-secondary)}
.source-markdown{line-height:1.7;overflow-x:auto;margin-top:14px}.source-markdown :deep(table){border-collapse:collapse;width:100%;margin:12px 0;font-variant-numeric:tabular-nums}.source-markdown :deep(td),.source-markdown :deep(th){border:1px solid var(--c-border,#cbd5e1);padding:7px 10px;min-width:65px;vertical-align:middle}.source-markdown :deep(blockquote){margin:12px 0;padding:8px 12px;border-left:3px solid #d97706;background:#f59e0b12}.source-markdown :deep(img){max-width:100%}


</style>
