<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRouter } from 'vue-router'
import { createElement } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import type { ExcalidrawImperativeAPI, ExcalidrawProps } from '@excalidraw/excalidraw/types'
import '@excalidraw/excalidraw/index.css'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Link, Edit, Delete, Download, Aim, Document, Close } from '../../../shared/icons'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { casyContext } from '../../../core/plugin/context'
import type { CanvasFact, WhiteboardSceneDto, WhiteboardSource } from '../../../types/bindings'
import WhiteboardSourcePicker from './WhiteboardSourcePicker.vue'
import { appendEdge, cardFor, collect, defaults, hydrate, revise, snapshot, sourceLabel, factLink, type Element, type Scene } from '../lib/document'
import { normalizeFacts } from '../lib/document'
import { blobData, exportWhiteboard, type ExportFormat } from '../lib/export'
import EmptyState from '../../../shared/components/EmptyState.vue'
const props=defineProps<{caseId:string;boardId:string;boardName?:string}>()
const emit=defineEmits<{change:[number];ready:[boolean];history:[]}>()
const target={caseId:props.caseId,whiteboardId:props.boardId}
const router=useRouter(),host=ref<HTMLElement>()
const status=ref('正在读取白板…'),ready=ref(false),failed=ref(false),dirty=ref(false),sidebar=ref(true)
const facts=ref<CanvasFact[]>([]),selectedId=ref(''),filter=ref(''),edgeTarget=ref('')
const selected=computed(()=>facts.value.find(f=>f.id===selectedId.value))
const filtered=computed(()=>facts.value.filter(f=>`${f.excerpt} ${f.sourceTitle} ${f.note || ''}`.toLowerCase().includes(filter.value.toLowerCase())))
const editOpen=ref(false),sourceOpen=ref(false),draft=ref<CanvasFact>(),submitting=ref(false)
const exportOpen=ref(false),exportFormat=ref<ExportFormat>('png'),exporting=ref(false),exportPath=ref('')
const historyOpen=ref(false),history=ref<WhiteboardSceneDto[]>([])
let api:ExcalidrawImperativeAPI|undefined,root:Root|undefined,editor:typeof import('@excalidraw/excalidraw')
let factOwners=new Map<string,string>()
let revision=0,factsHash='',saved='',pending='',disposed=false,saving:Promise<boolean>|undefined,timer:ReturnType<typeof setTimeout>|undefined,observer:MutationObserver|undefined
function elements():Element[]{return [...(api?.getSceneElementsIncludingDeleted() || [])]}
function current():Scene {return api?snapshot(elements(),api.getAppState(),api.getFiles()):JSON.parse(pending || saved)}
function setElements(items:Element[],selection?:string){
 if(!api)return
 api.updateScene({elements:items as any,captureUpdate:editor.CaptureUpdateAction.IMMEDIATELY,...(selection?{appState:{selectedElementIds:{[selection]:true}}}:{})})
 // updateScene and onChange are scheduled independently; update our pending document immediately.
 changed(items as any,api.getAppState(),api.getFiles())
}
const changed:NonNullable<ExcalidrawProps['onChange']>=(items,state,files)=>{
 const normalized=normalizeFacts(items,!state.editingTextElement,factOwners)
 factOwners=new Map(normalized.filter(e=>e.customData?.casy?.fact).map(e=>[e.customData.casy.fact.id,e.id]))
 if(normalized.some((e,i)=>e!==items[i]) && api){api.updateScene({elements:normalized as any,captureUpdate:editor.CaptureUpdateAction.NEVER})}
 pending=JSON.stringify(snapshot(normalized,state,files))
 const next=collect(normalized)
 facts.value=next.facts;emit('change',next.facts.length)
 const selectedCard=items.find(e=>state.selectedElementIds?.[e.id] && (e.customData?.casy as any)?.fact)
 if(selectedCard)selectedId.value=(selectedCard.customData!.casy as any).fact.id
 if(!ready.value || pending===saved)return
 dirty.value=true;status.value='尚未保存';clearTimeout(timer);timer=setTimeout(()=>void flush(),900)
}
async function flush(withPreview=false):Promise<boolean>{
 clearTimeout(timer)
 // Single-flight: concurrent callers (debounce + saveNow + leave) await one drain loop.
 while(saving){ if(!await saving)return false }
 if(!ready.value || !dirty.value || pending===saved)return !failed.value
 const payload=pending
 saving=(async()=>{
  status.value='正在保存…'
  try{
   const scene=JSON.parse(payload) as Scene
   // sceneJson 与 facts 必须共用同一套 normalize 后的 ID，否则后端校验「事实缺少对应画布卡片」。
   const normalized=normalizeFacts(scene.elements,false,factOwners)
   const data=collect(normalized)
   const sceneJson=JSON.stringify({...scene,elements:normalized})
   let preview:string|null=null
   if(withPreview && normalized.some(e=>!e.isDeleted)){
    try{preview=await blobData(await editor.exportToBlob({...scene,elements:normalized as any,appState:{...scene.appState,exportBackground:true,exportWithDarkMode:false},mimeType:'image/png',maxWidthOrHeight:640} as any))}catch{ /* Drawing and facts remain saveable if a font/image preview fails. */ }
   }
   const result=await tauriCallSafe('save_whiteboard_document',{input:{...target,sceneJson,preview,revision,factsHash,...data}})
   if(!result.ok || !result.data)throw new Error(result.error || '保存失败')
   revision=result.data.scene.revision;factsHash=result.data.factsHash;saved=payload;dirty.value=pending!==saved;failed.value=false
   status.value=dirty.value?'尚未保存':`已保存 · 版本 ${revision}`
   return true
  }catch(e){failed.value=true;status.value=String(e);return false}
 })()
 const ok=await saving;saving=undefined
 if(ok && pending!==saved && !failed.value)return flush(withPreview)
 return ok
}
async function saveNow(){if(!await flush(true))ElMessage.error(status.value)}
function beforeUnload(e:BeforeUnloadEvent){ if(!dirty.value)return; e.preventDefault(); e.returnValue='' }
onBeforeRouteLeave(()=>flush())
onBeforeRouteUpdate(()=>flush())
function theme(){return ['dark','solarized-dark'].includes(document.documentElement.dataset.theme || '')?'dark':'light'}
onMounted(async()=>{
 try{
  ;(window as any).EXCALIDRAW_ASSET_PATH=new URL('/excalidraw-assets/',window.location.href).href
  const result=await tauriCallSafe('get_whiteboard_document',target)
  if(!result.ok || !result.data)throw new Error(result.error || '无法读取白板')
  const document=result.data;revision=document.scene.revision;factsHash=document.factsHash
  editor=await import('@excalidraw/excalidraw')
  if(disposed || !host.value)return
  const initialized=hydrate(document,editor.convertToExcalidrawElements)
  saved=document.scene.sceneJson;pending=JSON.stringify(initialized.scene)
  factOwners=new Map(initialized.scene.elements.filter(e=>e.customData?.casy?.fact).map(e=>[e.customData.casy.fact.id,e.id]))
  root=createRoot(host.value)
  root.render(createElement(editor.Excalidraw,{
   initialData:{...initialized.scene,appState:{...initialized.scene.appState,theme:theme()},scrollToContent:true} as any,
   name:props.boardName || '事实白板',langCode:'zh-CN',aiEnabled:false,validateEmbeddable:false,handleKeyboardGlobally:false,autoFocus:false,
   UIOptions:{canvasActions:{loadScene:false,export:false,saveAsImage:false,toggleTheme:false}},
   excalidrawAPI:(instance)=>{api=instance},
   onChange:changed,
   onLinkOpen:(element,event)=>{event.preventDefault();void openLink(element.link || '')},
   onDuplicate:(next,previous)=>{
    const previousIds=new Set(previous.map(e=>e.id)), remap=new Map<string,string>()
    for(const e of next)if(!previousIds.has(e.id) && (e.customData?.casy as any)?.fact)remap.set((e.customData!.casy as any).fact.id,crypto.randomUUID())
    return next.map(e=>{
     if(previousIds.has(e.id))return e
     const data=e.customData?.casy as any
     if(data?.fact)return {...e,customData:{...e.customData,casy:{fact:{...data.fact,id:remap.get(data.fact.id)}}}}
     if(data?.captionFor)return {...e,customData:{...e.customData,casy:{captionFor:remap.get(data.captionFor)||data.captionFor}}}
     if(data?.edgeId)return {...e,customData:{...e.customData,casy:{edgeId:crypto.randomUUID()}}}
     return e
    })
   },
  },createElement(editor.MainMenu,{},
   createElement(editor.MainMenu.Item,{onSelect:()=>fit(),children:'适合视图'}),
   createElement(editor.MainMenu.Item,{onSelect:()=>{exportOpen.value=true},children:'导出画布'}),
   createElement(editor.MainMenu.Item,{onSelect:()=>void showVersions(),children:'恢复最近版本'}))))
  // Wait for the actual API and scene initialization, not two guessed animation frames.
  const initialize=()=>{
   if(disposed)return
   if(!api || !host.value?.querySelector('canvas')){requestAnimationFrame(initialize);return}
   const scene=current();pending=JSON.stringify(scene);ready.value=true;emit('ready',true)
   dirty.value=initialized.migrated
   if(!dirty.value)saved=pending
   status.value=dirty.value?'正在接入原有事实…':`已保存 · 版本 ${revision}`
   changed(elements() as any,api.getAppState(),api.getFiles())
   if(dirty.value)void flush()
  }
  requestAnimationFrame(initialize)
  observer=new MutationObserver(()=>api?.updateScene({appState:{theme:theme()},captureUpdate:editor.CaptureUpdateAction.NEVER}))
  observer.observe(window.document.documentElement,{attributes:true,attributeFilter:['data-theme']})
 }catch(e){failed.value=true;status.value=String(e)}
 window.addEventListener('beforeunload',beforeUnload)
})
onBeforeUnmount(()=>{disposed=true;clearTimeout(timer);observer?.disconnect();window.removeEventListener('beforeunload',beforeUnload);void flush();root?.unmount()})
function fit(){api?.scrollToContent(undefined,{fitToViewport:true,maxZoom:1})}
function focus(id:string){
 selectedId.value=id;sidebar.value=true
 const card=elements().find(e=>!e.isDeleted && e.customData?.casy?.fact?.id===id)
 if(card && api){api.updateScene({appState:{selectedElementIds:{[card.id]:true}}});api.scrollToContent([card as any],{fitToViewport:false,animate:false})}
}
function openFact(reference=false){
 if(!ready.value || !api)return
 const state=api.getAppState()
 const x=-state.scrollX+(state.width/2-170)/state.zoom.value,y=-state.scrollY+(state.height/2-80)/state.zoom.value
 draft.value={id:crypto.randomUUID(),fileId:null,knowledgeId:null,sourceCaseId:null,sourceTitle:'手动事实',page:null,excerpt:'',note:null,x,y};editOpen.value=true
 if(reference)sourceOpen.value=true
}
function editFact(fact:CanvasFact){draft.value={...fact};editOpen.value=true}
function chooseSource(source:WhiteboardSource){if(!draft.value)return;draft.value={...draft.value,fileId:source.kind==='file'?source.id:null,knowledgeId:source.kind==='knowledge'?source.id:null,sourceCaseId:source.caseId,sourceTitle:source.title,page:null,excerpt:draft.value.excerpt || (source.kind==='knowledge'?source.preview:'')}}
function clearSource(){if(draft.value)Object.assign(draft.value,{fileId:null,knowledgeId:null,sourceCaseId:null,sourceTitle:'手动事实',page:null})}
async function applyFact(){
 if(!draft.value || !draft.value.excerpt.trim())return ElMessage.warning('请填写事实摘录')
 submitting.value=true
 try{
  const fact={...draft.value,excerpt:draft.value.excerpt.trim(),note:draft.value.note?.trim() || null}
  setElements(revise(elements(),fact,editor.convertToExcalidrawElements));focus(fact.id)
  if(await flush()){editOpen.value=false;ElMessage.success('事实与画布已保存')}else ElMessage.error(status.value)
 }finally{submitting.value=false}
}
function removeFact(fact:CanvasFact){
 const items=elements(),card=items.find(e=>e.customData?.casy?.fact?.id===fact.id)
 if(!card)return
 setElements(items.map(e=>e.id===card.id || e.containerId===card.id || e.customData?.casy?.captionFor===fact.id || e.startBinding?.elementId===card.id || e.endBinding?.elementId===card.id?{...e,isDeleted:true,version:e.version+1,versionNonce:Math.floor(Math.random()*2**31)}:e))
 selectedId.value='';void saveNow()
}
function connect(){if(!selected.value || !edgeTarget.value)return;setElements(appendEdge(elements(),{id:crypto.randomUUID(),sourceNodeId:selected.value.id,targetNodeId:edgeTarget.value},editor.convertToExcalidrawElements));edgeTarget.value='';void saveNow()}
async function openLink(link:string){
 const match=link.match(/^casy-ref:(file|knowledge):([^:]+)(?::([^:]*))?(?::(\d*))?$/)
 if(!match)return ElMessage.warning('此图形未关联可定位的案件来源')
 if(!await flush())return ElMessage.error(status.value)
 try{
  if(match[1]==='knowledge')await router.push({name:'knowledge',query:{select:decodeURIComponent(match[2]!)}})
  else await router.push({name:'files',params:{caseId:decodeURIComponent(match[3] || target.caseId)},query:{select:decodeURIComponent(match[2]!),...(match[4]?{anchor:`page:${match[4]}`}:{})}})
 }catch(e){ElMessage.error(`无法打开来源：${e}`)}
}
async function showVersions(){
 if(!await flush())return ElMessage.error(status.value)
 const result=await tauriCallSafe('list_whiteboard_scene_history',target)
 if(result.ok && result.data){history.value=result.data;historyOpen.value=true}else ElMessage.error(result.error || '无法读取画布历史')
}
async function restore(version:WhiteboardSceneDto){
 try{await ElMessageBox.confirm(`恢复版本 ${version.revision}？当前版本会保留在历史中。`,'恢复白板')}catch{return}
 if(!api || !await flush())return
 const scene=JSON.parse(version.sceneJson) as Scene
 if(scene.elements.some(e=>!e.isDeleted && (e.customData?.factId || (e.type==='rectangle' && String(e.link || '').startsWith('casy-ref:') && !e.customData?.casy?.fact)))) {
  ElMessage.warning('此版本是旧版独立画布，缺少完整事实记录，不能安全覆盖当前白板。当前内容已保留。');return
 }
 api.addFiles(Object.values(scene.files) as any)
 setElements(scene.elements)
 api.updateScene({appState:{...scene.appState,theme:theme()},captureUpdate:editor.CaptureUpdateAction.NEVER})
 if(await flush()){historyOpen.value=false;fit()}else ElMessage.error(status.value)
}
async function doExport(){
 if(exporting.value || !api)return
 exporting.value=true;exportPath.value=''
 try{const path=await exportWhiteboard(current(),exportFormat.value,props.boardName || '事实白板',target,editor);if(path){exportPath.value=path;ElMessage.success('画布已导出')}}catch(e){ElMessage.error(`导出失败：${e}`)}finally{exporting.value=false}
}
defineExpose({flush,openFact})
</script>
<template>
 <section class="rich-canvas">
  <div class="canvas-tools"><el-button :icon="Link" :disabled="!ready" @click="openFact(true)">添加引用</el-button><el-button :icon="Aim" :disabled="!ready" @click="fit">适合视图</el-button><el-button :icon="Download" :disabled="!ready" @click="exportOpen=true;exportPath=''">导出画布</el-button><el-button :disabled="!ready" @click="showVersions">版本历史</el-button><el-button @click="sidebar=!sidebar">{{sidebar?'收起事实面板':'显示事实面板'}}</el-button><div class="tools-spacer"/><span :class="{error:failed}" role="status">{{status}}</span><el-button :disabled="!ready" @click="saveNow">{{failed?'重试保存':'保存'}}</el-button></div>
  <el-alert v-if="failed" :title="status" type="error" :closable="false" description="当前改动尚未保存。可重试保存，或导出可编辑文件保留改动。" />
  <div class="canvas-workspace">
   <div ref="host" class="excalidraw-host" />
   <aside v-if="sidebar" class="fact-panel" aria-label="白板事实">
    <div class="fact-panel-heading"><strong>事实 · {{facts.length}}</strong><el-button text size="small" @click="emit('history')">修订记录</el-button></div>
    <el-input v-model="filter" placeholder="搜索事实与来源" clearable aria-label="搜索白板事实" />
    <div class="fact-list"><p v-if="!facts.length" class="fact-empty ui-text--caption">添加事实或引用后会立即出现在画布中。双击卡片文字即可修订摘录，截图可直接粘贴。</p><button v-for="fact in filtered" :key="fact.id" :class="['fact-list-item',{selected:selectedId===fact.id}]" @click="focus(fact.id)"><span>{{fact.excerpt}}</span><small>{{sourceLabel(fact)}}</small></button></div>
    <section v-if="selected" class="fact-detail"><div class="fact-panel-heading"><strong>选中事实</strong><el-button text :icon="Edit" @click="editFact(selected)">修订</el-button></div><p class="fact-body">{{selected.excerpt}}</p><p v-if="selected.note" class="fact-note">批注：{{selected.note}}</p><p class="fact-source">{{sourceLabel(selected)}}</p><el-button v-if="factLink(selected)" size="small" :icon="Link" @click="openLink(factLink(selected)!)">打开来源</el-button><div class="fact-connect"><el-select v-model="edgeTarget" placeholder="关联另一事实" aria-label="关联另一事实"><el-option v-for="fact in facts.filter(f=>f.id!==selectedId)" :key="fact.id" :value="fact.id" :label="fact.excerpt.slice(0,50)" /></el-select><el-button :disabled="!edgeTarget" @click="connect">连线</el-button></div><el-button text type="danger" :icon="Delete" @click="removeFact(selected)">移出白板（可撤销）</el-button></section>
   </aside>
  </div>
  <div class="canvas-help">事实卡片、来源与连线自动保存；双击卡片编辑文字，拖动调整位置。自由绘图用于分析，不会改写来源文件。⌘ / Ctrl + Z 撤销。</div>
  <el-dialog v-model="editOpen" :title="facts.some(f=>f.id===draft?.id)?'修订事实':'添加事实'" width="660px" class="whiteboard-fact-dialog" append-to-body :close-on-click-modal="false" :show-close="!submitting">
   <el-form v-if="draft" label-position="top" @submit.prevent="applyFact">
    <el-form-item label="引用来源"><div class="fact-source-field"><div><strong>{{draft.sourceTitle}}</strong><small>{{draft.fileId?'案件卷宗':draft.knowledgeId?'知识库条目':'独立事实，不绑定来源文件'}}</small></div><el-button :icon="Link" :disabled="submitting" @click="sourceOpen=true">选择来源</el-button><el-button v-if="draft.fileId || draft.knowledgeId" text :icon="Close" aria-label="移除来源关联" :disabled="submitting" @click="clearSource" /></div></el-form-item>
    <el-form-item v-if="draft.fileId" label="来源页码（可选）"><el-input-number v-model="draft.page" :min="1" :precision="0" controls-position="right" placeholder="未指定" :disabled="submitting" /></el-form-item>
    <el-form-item label="事实摘录" required><el-input v-model="draft.excerpt" type="textarea" :rows="6" :maxlength="20000" show-word-limit :disabled="submitting" placeholder="输入或粘贴事实；这段文字会显示在画布卡片上" /></el-form-item>
    <el-form-item label="分析批注（可选）"><el-input v-model="draft.note" type="textarea" :rows="3" :maxlength="20000" :disabled="submitting" placeholder="你的分析、疑点或待核实事项" /></el-form-item>
   </el-form>
   <template #footer><el-button :disabled="submitting" @click="editOpen=false">取消</el-button><el-button type="primary" :loading="submitting" @click="applyFact">保存并显示在画布</el-button></template>
  </el-dialog>
  <WhiteboardSourcePicker v-model="sourceOpen" :case-id="target.caseId" @select="chooseSource" />
  <el-dialog v-model="exportOpen" title="导出画布" width="540px" class="whiteboard-fact-dialog" append-to-body :close-on-click-modal="!exporting">
   <el-form label-position="top"><el-form-item label="文件格式"><el-select v-model="exportFormat" :disabled="exporting" aria-label="导出格式"><el-option value="png" label="PNG 图片 · 查看与分享"/><el-option value="svg" label="SVG 矢量图 · 放大与打印"/><el-option value="excalidraw" label="可编辑画布 · 保留图形、截图和引用"/></el-select></el-form-item></el-form>
   <p class="export-help">导出当前画布内容，保留原白板。PNG 最长边不超过 6000 像素，SVG 可无损缩放；可编辑文件包含嵌入截图和引用标识。</p><p v-if="exportPath" class="export-result">已保存：{{exportPath}}</p><el-button v-if="exportPath" text @click="casyContext.files.reveal(exportPath)">在文件夹中显示</el-button>
   <template #footer><el-button :disabled="exporting" @click="exportOpen=false">关闭</el-button><el-button type="primary" :loading="exporting" @click="doExport">选择位置并导出</el-button></template>
  </el-dialog>
  <el-dialog v-model="historyOpen" title="最近 5 次内容版本" width="660px" class="whiteboard-fact-dialog" append-to-body><EmptyState v-if="!history.length" type="custom" compact hide-action title="暂无历史版本"/><article v-for="h in history" :key="h.revision" class="scene-history"><img v-if="h.preview" :src="h.preview" alt="历史白板预览"/><span>版本 {{h.revision}}</span><el-button @click="restore(h)">恢复</el-button></article></el-dialog>
 </section>
</template>
<style scoped>
.rich-canvas{display:flex;flex-direction:column;flex:1;min-width:0;min-height:0;overflow:hidden}.canvas-tools{display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding:10px 16px;border-bottom:1px solid var(--c-border)}.canvas-tools .el-button{margin:0}.canvas-tools>span{font-size:12px;color:var(--c-text-secondary);max-width:280px;overflow-wrap:anywhere}.canvas-tools>.error{color:var(--el-color-danger)}.tools-spacer{flex:1}.canvas-workspace{display:flex;flex:1;min-height:0;min-width:0}.excalidraw-host{flex:1;min-width:0;min-height:320px;position:relative}.excalidraw-host :deep(.library-button){display:none}.fact-panel{width:290px;flex-shrink:0;border-left:1px solid var(--c-border);background:var(--c-bg-card,var(--el-bg-color));padding:14px;display:flex;flex-direction:column;gap:12px;overflow:auto}.fact-panel-heading{display:flex;align-items:center;justify-content:space-between;gap:8px;font-size:13px}.fact-panel-heading .el-button{padding:0}.fact-list{min-height:70px;max-height:35%;overflow:auto;flex-shrink:0}.fact-empty{line-height:1.8}.fact-list-item{display:grid;gap:7px;width:100%;padding:10px;border:1px solid transparent;background:transparent;border-radius:6px;text-align:left;color:var(--c-text);cursor:pointer}.fact-list-item:hover{background:var(--el-fill-color-light)}.fact-list-item.selected{background:var(--el-color-primary-light-9);border-color:var(--el-color-primary-light-5)}.fact-list-item span{font-size:13px;line-height:1.65;display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;overflow-wrap:anywhere}.fact-list-item small,.fact-source{font-size:11px;color:var(--c-text-secondary);overflow-wrap:anywhere}.fact-detail{border-top:1px solid var(--c-border);padding-top:12px;font-size:12px}.fact-body,.fact-note{line-height:1.8;white-space:pre-wrap;overflow-wrap:anywhere}.fact-note{padding:8px;background:var(--el-fill-color-light);border-radius:4px}.fact-connect{display:flex;gap:6px;margin:14px 0}.fact-connect .el-select{flex:1;min-width:0}.canvas-help{font-size:11px;color:var(--c-text-secondary);padding:8px 16px;border-top:1px solid var(--c-border)}.fact-source-field{width:100%;display:flex;align-items:center;gap:10px;padding:12px;background:var(--el-fill-color-light);border:1px solid var(--c-border);border-radius:6px}.fact-source-field>div{display:grid;gap:4px;flex:1;min-width:0;overflow-wrap:anywhere}.fact-source-field strong{font-size:13px;font-weight:500}.fact-source-field small{font-size:11px;color:var(--c-text-secondary)}.fact-source-field .el-button{margin:0}.export-help,.export-result{font-size:13px;line-height:1.8;overflow-wrap:anywhere}.export-help{color:var(--c-text-secondary)}.scene-history{display:flex;align-items:center;gap:20px;padding:16px 0;border-bottom:1px solid var(--c-border)}.scene-history img{width:220px;max-height:150px;object-fit:contain}@media(max-width: 1100px){.fact-panel{width:240px}.canvas-tools{gap:6px}}

</style>
<style>.el-dialog.whiteboard-fact-dialog{max-width:calc(100vw - 24px)}.whiteboard-fact-dialog .el-select{width:100%}.whiteboard-fact-dialog .el-form-item{margin-bottom:20px}</style>
