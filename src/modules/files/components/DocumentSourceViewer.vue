<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { ArrowLeft, ArrowRight, ZoomIn, ZoomOut, Refresh } from '@element-plus/icons-vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import type { DocumentPageView, SourceLocation } from '../../../types/documentRetrieval'

const props = defineProps<{ modelValue:boolean; fileId:string; jobId:string; initialPage?:number; locations?:SourceLocation[] }>()
const emit=defineEmits<{(e:'update:modelValue',value:boolean):void}>()
const view=ref<DocumentPageView|null>(null), page=ref(1), busy=ref(false), error=ref(''), zoom=ref(100)
const mode=ref('text'), selected=ref<number|null>(null)
let revision=0
const marked=computed(()=> new Set<number>((props.locations||[]).flatMap(l=>l.pageNumber===page.value && l.regionIndex!==null?[l.regionIndex]:[])))
const hitPages=computed(()=> [...new Set((props.locations||[]).map(l=>l.pageNumber))])
watch(()=>[props.modelValue,props.fileId,props.jobId,props.initialPage],()=>{
  revision++; view.value=null; error.value=''; busy.value=false; selected.value=null; zoom.value=100
  if(props.modelValue) {page.value=props.initialPage||1; void load()}
},{immediate:true})
onUnmounted(()=>revision++)
async function load() {
  const current=++revision
  busy.value=true; error.value=''; view.value=null; selected.value=null
  try {
    const result=await tauriCallSafe('get_document_page',{fileId:props.fileId,jobId:props.jobId,pageNumber:page.value})
    if(current!==revision)return
    if(!result.ok||!result.data)throw new Error(result.error||'无法读取页面')
    view.value=result.data
    await nextTick()
    const first=marked.value.values().next().value
    if(first!==undefined)focusRegion(first)
  }catch(e){if(current===revision)error.value=e instanceof Error?e.message:String(e)}
  finally{if(current===revision)busy.value=false}
}
function go(number:number){page.value=number; void load()}
function changePage(value:number|undefined){if(value)go(value)}
function regionStyle(bbox:number[]){
  const width=view.value?.width||1,height=view.value?.height||1
  return {left:`${bbox[0]/width*100}%`,top:`${bbox[1]/height*100}%`,width:`${(bbox[2]-bbox[0])/width*100}%`,height:`${(bbox[3]-bbox[1])/height*100}%`}
}
function focusRegion(index:number){
  selected.value=index
  document.getElementById(`source-region-${index}`)?.scrollIntoView({block:'center',inline:'nearest',behavior:'smooth'})
}
</script>

<template>
  <el-dialog :model-value="modelValue" @update:model-value="emit('update:modelValue',$event)" :title="view?.fileName||'文档对照'" width="min(1240px, calc(100vw - 24px))" top="3vh" append-to-body class="document-source-dialog" destroy-on-close>
    <div class="source-toolbar">
      <div class="source-navigation">
      <el-button :icon="ArrowLeft" aria-label="上一页" title="上一页" :disabled="busy||page<=1" @click="go(page-1)" />
      <el-input-number :model-value="page" :min="1" :max="view?.totalPages||page" :disabled="busy||!view" :controls="false" aria-label="页码" @change="changePage" />
      <span>/ {{ view?.totalPages||'…' }}</span>
      <el-button :icon="ArrowRight" aria-label="下一页" title="下一页" :disabled="busy||!view||page>=view.totalPages" @click="go(page+1)" />
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
          <button v-for="(region,index) in view.regions" :id="`source-region-${index}`" :key="index" class="source-region" :class="{marked:marked.has(index),selected:selected===index}" :style="regionStyle(region.bbox)" :aria-label="region.text" :title="region.text" @click="selected=index" />
        </div>
      </div>
      <div class="source-text-pane">
        <el-radio-group v-model="mode" size="small"><el-radio-button value="text">识别文字</el-radio-button><el-radio-button value="markdown">Markdown</el-radio-button></el-radio-group>
        <div v-if="mode==='text'&&view.regions.length" class="source-text-lines">
          <button v-for="(region,index) in view.regions" :key="index" :class="{marked:marked.has(index),selected:selected===index}" @click="focusRegion(index)">{{ region.text }}</button>
        </div>
        <pre v-else>{{ view.markdown }}</pre>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.source-toolbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:12px}.source-toolbar .el-button+.el-button{margin-left:0}.source-toolbar .el-input-number{width:64px}.zoom-value{width:42px;text-align:center}.hit-pages{display:flex;gap:8px;margin-left:auto}
.source-navigation,.source-zoom{display:flex;align-items:center;gap:8px;flex-shrink:0}
.source-panes{display:grid;grid-template-columns:minmax(0,1.4fr) minmax(260px,1fr);height:72vh;min-height:0;border-top:1px solid var(--c-border)}.source-panes.text-only{grid-template-columns:1fr}.source-image-pane{overflow:auto;background:#e5e7eb;padding:16px}.source-page-image{position:relative;margin:0 auto;min-width:50%;line-height:0}.source-page-image img{width:100%;height:auto;display:block}.source-region{position:absolute;border:1px solid transparent;background:transparent;cursor:pointer;padding:0}.source-region:hover,.source-region:focus-visible{border-color:#0284c7;background:#0ea5e922}.source-region.marked{background:#eab30844;border-color:#ca8a04}.source-region.selected{background:#0ea5e944;border:2px solid #0284c7}
.source-text-pane{overflow:auto;padding:16px;min-width:0}.source-text-lines{display:flex;flex-direction:column;gap:4px;margin-top:14px}.source-text-lines button{text-align:left;padding:6px 8px;background:transparent;border:0;border-left:3px solid transparent;color:var(--c-text);font:inherit;line-height:1.65;overflow-wrap:anywhere;cursor:pointer}.source-text-lines button.marked{background:#eab30822;border-left-color:#ca8a04}.source-text-lines button.selected{background:#0ea5e922;border-left-color:#0284c7}.source-text-pane pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.7}.source-loading{height:72vh;display:grid;place-items:center;color:var(--c-text-secondary)}
@media(max-width:700px){.source-panes{grid-template-columns:1fr;grid-template-rows:minmax(230px,1fr) minmax(170px,0.7fr)}.source-panes.text-only{grid-template-rows:1fr}.source-image-pane,.source-text-pane{padding:8px}.hit-pages{width:100%;margin-left:0}}
</style>
