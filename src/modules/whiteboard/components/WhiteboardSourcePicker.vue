<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { Search, Document, Collection } from '../../../shared/icons'
import { tauriCallSafe } from '../../../core/tauriBridge'
import type { WhiteboardSource } from '../../../types/bindings'
const props=defineProps<{modelValue:boolean;caseId:string}>()
const emit=defineEmits<{'update:modelValue':[boolean];select:[WhiteboardSource]}>()
const scope=ref('case'),query=ref(''),items=ref<WhiteboardSource[]>([]),selected=ref<WhiteboardSource>(),offset=ref(0),total=ref(0),loading=ref(false),error=ref('')
let sequence=0
const preview=computed(()=>selected.value?.preview.replace(/!\[[^\]]*\]\(data:[^)]+\)/g,'[图片]').replace(/<[^>]*>/g,'').trim() || '')
async function search(reset=true){
 if(reset)offset.value=0
 const request=++sequence;loading.value=true;error.value='';selected.value=undefined
 const result=await tauriCallSafe('list_whiteboard_sources',{caseId:props.caseId,scope:scope.value,query:query.value.trim(),offset:offset.value})
 if(request!==sequence || !props.modelValue)return
 loading.value=false
 if(result.ok && result.data){items.value=result.data.items;total.value=result.data.total}
 else{items.value=[];total.value=0;error.value=result.error || '无法读取引用来源'}
}
function page(delta:number){offset.value+=delta*30;void search(false)}
watch(()=>props.modelValue,open=>{if(open){scope.value='case';query.value='';void search()}else{sequence++;loading.value=false}})
</script>
<template>
 <el-dialog :model-value="modelValue" title="选择引用来源" width="850px" class="whiteboard-source-dialog" append-to-body @update:model-value="emit('update:modelValue',$event)">
  <div class="source-search">
   <el-radio-group v-model="scope" aria-label="来源范围" @change="search()"><el-radio-button value="case">本案卷宗</el-radio-button><el-radio-button value="other">其他案件卷宗</el-radio-button><el-radio-button value="knowledge">知识库</el-radio-button></el-radio-group>
   <el-input v-model="query" placeholder="搜索文件名、知识标题或正文" :prefix-icon="Search" clearable @keyup.enter="search()" @clear="search()"><template #append><el-button @click="search()">搜索</el-button></template></el-input>
  </div>
  <el-alert v-if="error" :title="error" type="error" :closable="false" />
  <div class="source-browser" v-loading="loading">
   <section class="source-results" aria-label="引用来源列表">
    <div class="source-count">{{ total }} 个来源</div>
    <el-empty v-if="!loading && !items.length" :description="error?'读取失败，请重试':'没有匹配的来源'" :image-size="55" />
    <button v-for="item in items" :key="`${item.kind}:${item.id}`" type="button" :class="['source-result',{selected:selected?.id===item.id}]" @click="selected=item">
     <el-icon><Collection v-if="item.kind==='knowledge'"/><Document v-else/></el-icon><span><strong>{{item.title}}</strong><small>{{item.caseName || '知识库笔记 / 法条 / 摘录'}}<template v-if="item.totalPages"> · {{item.totalPages}} 页</template></small></span>
    </button>
   </section>
   <section class="source-preview" aria-label="引用来源预览">
    <template v-if="selected"><span class="source-count">来源预览</span><h3>{{selected.title}}</h3><p class="source-context">{{selected.caseName || '知识库'}}</p><p class="source-text" v-if="preview">{{preview}}</p><p class="source-no-text" v-else>此文件尚无可预览正文。可以引用该文件，再填写你要摘录的事实。</p><p class="source-preview-note">预览仅供核对。选择后可编辑摘录、批注和来源页码，原文件或笔记不会被改写。</p></template>
    <p v-else class="source-no-text">选择左侧来源，核对标题、所属案件和内容。</p>
   </section>
  </div>
  <div class="source-pages"><el-button :disabled="offset===0 || loading" @click="page(-1)">上一页</el-button><span>{{ Math.floor(offset/30)+1 }} / {{Math.max(1,Math.ceil(total/30))}}</span><el-button :disabled="offset+30>=total || loading" @click="page(1)">下一页</el-button></div>
  <template #footer><el-button @click="emit('update:modelValue',false)">取消</el-button><el-button type="primary" :disabled="!selected || loading" @click="emit('select',selected!);emit('update:modelValue',false)">使用此来源</el-button></template>
 </el-dialog>
</template>
<style scoped>
.source-search{display:grid;gap:14px;margin-bottom:18px}.source-browser{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);min-height:320px;max-height:48vh;border:1px solid var(--c-border);border-radius:8px;overflow:hidden}.source-results,.source-preview{min-width:0;overflow:auto;padding:14px}.source-results{border-right:1px solid var(--c-border)}.source-count,.source-context,.source-preview-note{font-size:12px;color:var(--c-text-secondary)}.source-count{display:block;margin-bottom:10px}.source-result{width:100%;display:flex;gap:10px;align-items:flex-start;padding:12px 9px;border:1px solid transparent;background:transparent;text-align:left;border-radius:6px;color:var(--c-text);cursor:pointer}.source-result:hover{background:var(--el-fill-color-light)}.source-result.selected{background:var(--el-color-primary-light-9);border-color:var(--el-color-primary-light-5)}.source-result>span{display:grid;gap:5px;min-width:0}.source-result strong{font-size:13px;font-weight:500;overflow-wrap:anywhere}.source-result small{font-size:11px;color:var(--c-text-secondary)}.source-preview h3{font-size:15px;margin:0;overflow-wrap:anywhere}.source-text{font-size:13px;line-height:1.8;white-space:pre-wrap;overflow-wrap:anywhere}.source-no-text{font-size:13px;line-height:1.8;color:var(--c-text-secondary)}.source-preview-note{line-height:1.7;border-top:1px solid var(--c-border);padding-top:12px}.source-pages{display:flex;justify-content:center;gap:14px;align-items:center;margin-top:14px;font-size:12px}@media(max-width:650px){.source-browser{grid-template-columns:1fr;max-height:55vh}.source-preview{border-top:1px solid var(--c-border)}.source-results{max-height:24vh;border-right:0}.source-search :deep(.el-radio-group){display:flex;flex-wrap:wrap}}
</style>
<style>.el-dialog.whiteboard-source-dialog{max-width:calc(100vw - 24px)}</style>
