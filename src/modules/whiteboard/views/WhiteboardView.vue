<script setup lang="ts">
import {ref,computed,onMounted,watch} from 'vue'
import {useRoute,useRouter} from 'vue-router'
import {ElMessage,ElMessageBox} from 'element-plus'
import {ArrowLeft,Plus,Edit,Delete,Refresh} from '../../../shared/icons'
import {tauriCallSafe} from '../../../core/tauriBridge'
import {useCasesStore} from '../../../stores/cases'
import RichCanvas from '../components/RichCanvas.vue'
import EmptyState from '../../../shared/components/EmptyState.vue'
import type {WhiteboardDto,FactHistoryDto} from '../../../types/bindings'
const route=useRoute(),router=useRouter(),cases=useCasesStore()
const caseId=computed(()=>String(route.params.caseId || ''))
const caseName=ref(''),boards=ref<WhiteboardDto[]>([]),currentId=ref(''),loading=ref(false),error=ref(''),ready=ref(false),reloadKey=ref(0)
const canvas=ref<InstanceType<typeof RichCanvas>>()
const current=computed(()=>boards.value.find(b=>b.id===currentId.value))
const historyOpen=ref(false),historyLoading=ref(false),history=ref<(FactHistoryDto & {detail:any})[]>([]),historyMore=ref(false)
let loadSequence=0
async function load(preferred?:string){
 const seq=++loadSequence,id=caseId.value;loading.value=true;error.value=''
 const [caseResult,boardResult]=await Promise.all([cases.loadCase(id),tauriCallSafe('list_whiteboards',{caseId:id})])
 if(seq!==loadSequence || id!==caseId.value)return
 loading.value=false
 caseName.value=caseResult.ok?caseResult.data?.caseName || '':''
 if(!boardResult.ok || !boardResult.data){error.value=boardResult.error || '无法读取白板';boards.value=[];currentId.value='';return}
 boards.value=boardResult.data
 const requested=preferred || (typeof route.query.board==='string'?route.query.board:'')
 currentId.value=requested?boards.value.find(b=>b.id===requested)?.id || '':boards.value[0]?.id || ''
 if(requested && !currentId.value)error.value='指定白板不存在或不属于当前案件，请选择其他白板'
 if(currentId.value && route.query.board!==currentId.value)await router.replace({query:{...route.query,board:currentId.value}})
}
async function flush(){if(canvas.value && !await canvas.value.flush()){ElMessage.error('画布尚未保存，请先处理保存错误或导出保留改动');return false}return true}
async function select(id:string){if(id===currentId.value || !await flush())return;ready.value=false;currentId.value=id;historyOpen.value=false;await router.replace({query:{...route.query,board:id}})}
async function create(){
 if(!await flush())return
 const id=caseId.value
 try{
  const result=await ElMessageBox.prompt('请输入白板名称','新建白板',{inputPlaceholder:'例如：争议焦点与证据关系',inputValidator:(v:string)=>!!v?.trim() || '名称不能为空',confirmButtonText:'创建',cancelButtonText:'取消'})
  if(id!==caseId.value)return
  const added=await tauriCallSafe('create_whiteboard',{caseId:id,name:result.value.trim()})
  if(!added.ok || !added.data)return ElMessage.error(added.error || '创建失败')
  ready.value=false;await load(added.data)
 }catch{/* cancelled */}
}
async function rename(){
 const board=current.value;if(!board)return
 try{
  const result=await ElMessageBox.prompt('请输入白板名称','重命名',{inputValue:board.name,inputValidator:(v:string)=>!!v?.trim() || '名称不能为空'})
  const renamed=await tauriCallSafe('rename_whiteboard',{id:board.id,name:result.value.trim()})
  if(renamed.ok)board.name=result.value.trim();else ElMessage.error(renamed.error || '重命名失败')
 }catch{/* cancelled */}
}
async function remove(){
 const board=current.value;if(!board || !await flush())return
 try{await ElMessageBox.confirm(`删除「${board.name}」及其中的事实和画布？来源文件和知识笔记会保留。`,'删除白板',{type:'warning'})}catch{return}
 const result=await tauriCallSafe('delete_whiteboard',{id:board.id})
 if(!result.ok)return ElMessage.error(result.error || '删除失败')
 currentId.value='';await router.replace({query:{...route.query,board:undefined}});await load()
}
async function refresh(){if(!await flush())return;ready.value=false;await load(currentId.value);reloadKey.value++}
function changed(count:number){if(current.value)current.value.nodeCount=count}
async function showHistory(append=false){
 if(historyLoading.value || !currentId.value || !await flush())return
 const board=currentId.value
 historyOpen.value=true;historyLoading.value=true
 if(!append)history.value=[]
 const result=await tauriCallSafe('list_fact_history',{whiteboardId:board,beforeSequence:append?history.value[history.value.length-1]?.sequence || null:null})
 if(board!==currentId.value){historyLoading.value=false;return}
 try{
  if(!result.ok || !result.data)throw new Error(result.error || '读取失败')
  history.value.push(...result.data.map(row=>({...row,detail:JSON.parse(row.payload)})));historyMore.value=result.data.length===100
 }catch(e){ElMessage.error(`无法读取事实历史：${e}`)}finally{historyLoading.value=false}
}
watch(caseId,()=>{currentId.value='';boards.value=[];ready.value=false;historyOpen.value=false;void load()})
watch(()=>route.query.board,id=>{if(loading.value || typeof id!=='string' || id===currentId.value)return;if(boards.value.some(b=>b.id===id)){ready.value=false;currentId.value=id}else error.value='此白板不属于当前案件'})
onMounted(()=>void load())
</script>
<template>
 <div class="whiteboard-view">
  <header class="whiteboard-topbar"><el-button text :icon="ArrowLeft" @click="router.push(`/cases/${caseId}`)">{{caseName || '案件详情'}}</el-button><el-divider direction="vertical"/><el-select :model-value="currentId" placeholder="选择事实白板" class="board-select" :loading="loading" @change="select"><el-option v-for="board in boards" :key="board.id" :label="board.name" :value="board.id"/></el-select><el-button :icon="Plus" @click="create">新建白板</el-button><el-button text :icon="Edit" :disabled="!current" @click="rename">重命名</el-button><el-button text :icon="Delete" :disabled="!current" @click="remove">删除</el-button><div class="topbar-spacer"/><span v-if="current" class="node-count">{{current.nodeCount}} 条事实</span><el-button text :icon="Refresh" :disabled="loading" aria-label="刷新白板" @click="refresh"/><el-button type="primary" :icon="Plus" :disabled="!current || !ready" @click="canvas?.openFact()">添加事实节点</el-button></header>
  <el-alert v-if="error" :title="error" type="error" :closable="false"><el-button text @click="load(currentId || undefined)">重试</el-button></el-alert>
  <div v-if="loading && !current" class="whiteboard-empty">正在读取白板…</div>
  <div v-else-if="!boards.length && !error" class="whiteboard-empty"><EmptyState title="还没有事实白板" description="把案件事实、证据引用和分析图形放在同一块白板中。" action-text="新建白板" @action="create"/></div>
  <RichCanvas v-if="current" :key="`${current.id}:${reloadKey}`" ref="canvas" :case-id="caseId" :board-id="current.id" :board-name="current.name" @change="changed" @ready="ready=$event" @history="showHistory()"/>
  <el-dialog v-model="historyOpen" title="事实修订记录" width="700px" class="whiteboard-history-dialog" append-to-body><p class="history-intro ui-text ui-text--secondary">画布和事实面板共用同一份记录。删除与撤销恢复均会留痕，来源文件和知识正文不会被修改。</p><EmptyState v-if="!historyLoading && !history.length" type="custom" compact hide-action title="暂无修订记录"/><article v-for="entry in history" :key="entry.id" class="history-entry"><strong>{{({fact_created:'新增事实',fact_updated:'修订事实',fact_deleted:'移除事实'} as Record<string,string>)[entry.eventType]}}</strong><time>{{entry.createdAt}}</time><div v-for="side in (['before','after'] as const)" :key="side"><template v-if="entry.detail[side]"><small>{{side==='before'?'修改前':'修改后'}}</small><p>{{entry.detail[side].excerpt}}</p><p v-if="entry.detail[side].note">批注：{{entry.detail[side].note}}</p><small>来源：{{entry.detail[side].sourceTitle || entry.detail[side].fileName || '手动事实'}} · 页码 {{entry.detail[side].page || '未指定'}}</small></template></div></article><el-button v-if="historyMore" :loading="historyLoading" @click="showHistory(true)">加载更早记录</el-button></el-dialog>
 </div>
</template>
<style scoped>
.whiteboard-view{display:flex;flex-direction:column;height:100%;min-height:0;min-width:0;overflow:hidden;background:var(--c-bg-page);color:var(--c-text)}.whiteboard-topbar{display:flex;flex-wrap:wrap;align-items:center;gap:8px;padding:10px 16px;border-bottom:1px solid var(--c-border);background:var(--c-bg-topbar)}.whiteboard-topbar .el-button{margin:0}.board-select{width:210px}.topbar-spacer{flex:1}.node-count{font-size:12px;color:var(--c-text-secondary)}.whiteboard-empty{flex:1;display:grid;place-content:center}.history-intro{line-height:1.8}.history-entry{border-bottom:1px solid var(--c-border);padding:18px 0}.history-entry time{font-size:11px;color:var(--c-text-secondary);margin-left:12px}.history-entry div{margin-top:12px}.history-entry p{white-space:pre-wrap;overflow-wrap:anywhere;font-size:13px;line-height:1.8}.history-entry small{color:var(--c-text-secondary)}
</style>
<style>.el-dialog.whiteboard-history-dialog{max-width:calc(100vw - 24px)}</style>
