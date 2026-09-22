<script setup lang="ts">
import { ref, shallowRef, watch, onBeforeUnmount, nextTick } from 'vue';
import { tauriCallSafe } from '../../core/tauriBridge';
import type { DocumentLayout, DocumentPreview, RichTextDocument } from '../../core/services/docs';

const props=defineProps<{ document:RichTextDocument|null; layout:DocumentLayout; activeBlock:number }>();
const emit=defineEmits<{ 'update:layout':[layout:DocumentLayout]; 'select-block':[block:number] }>();
const result=shallowRef<DocumentPreview|null>(null), urls=ref<string[]>([]);
const stale=ref(true), busy=ref(false), error=ref(''), scroller=ref<HTMLElement>();
let revision=0, running=false, disposed=false, timer:ReturnType<typeof setTimeout>|undefined;
function release(){ urls.value.forEach(url=>URL.revokeObjectURL(url)); urls.value=[]; }
function schedule(){
  revision++; stale.value=true; error.value='';
  if(timer)clearTimeout(timer);
  timer=setTimeout(()=>{timer=undefined; void compile();},550);
}
async function compile(){
  if(running || disposed || !props.document)return;
  running=true; busy.value=true;
  const token=revision;
  try {
    const reply=await tauriCallSafe('preview_editor_document',{
      document:JSON.parse(JSON.stringify(props.document)),layout:{...props.layout},
    });
    if(disposed || token!==revision)return;
    if(!reply.ok || !reply.data)throw new Error(reply.error || '排版没有返回页面');
    release(); result.value=reply.data;
    urls.value=reply.data.pages.map(page=>URL.createObjectURL(new Blob([page.svg],{type:'image/svg+xml'})));
    stale.value=false;
    await nextTick(); revealBlock();
  } catch(e){ if(!disposed && token===revision)error.value=String(e); }
  finally {
    running=false; busy.value=false;
    // Coalesce all changes while compiling into one latest snapshot.
    if(!disposed && token!==revision && !timer)void compile();
  }
}
function revealBlock(){
  if(stale.value)return;
  const target=scroller.value?.querySelector<HTMLElement>(`[data-block="${props.activeBlock}"]`);
  const root=scroller.value;
  if(target && root){
    const box=target.getBoundingClientRect(), frame=root.getBoundingClientRect();
    if(box.top<frame.top+30 || box.bottom>frame.bottom-30)root.scrollTop+=box.top-frame.top-frame.height/3;
  }
}
function setOption(key:keyof DocumentLayout,value:unknown){ emit('update:layout',{...props.layout,[key]:value}); }
watch(()=>[props.document,props.layout],schedule,{immediate:true});
watch(()=>props.activeBlock,revealBlock);
onBeforeUnmount(()=>{ disposed=true;revision++;if(timer)clearTimeout(timer);release(); });
</script>

<template>
  <aside class="typeset-preview" aria-label="A4 排版预览">
    <div class="typeset-controls">
      <strong>A4 排版</strong>
      <span role="status">{{ busy ? '正在排版…' : stale ? '预览待更新' : `${result?.pages.length || 0} 页` }}</span>
      <details>
        <summary>版式设置</summary>
        <div class="layout-options">
          <label>页边距（mm）<input aria-label="页边距" type="number" min="10" max="40" :value="layout.marginMm" @change="setOption('marginMm',Number(($event.target as HTMLInputElement).value))"></label>
          <label>装订边（mm）<input aria-label="装订边" type="number" min="0" max="15" :value="layout.bindingMm" @change="setOption('bindingMm',Number(($event.target as HTMLInputElement).value))"></label>
          <label><input type="checkbox" :checked="layout.firstLineIndent" @change="setOption('firstLineIndent',($event.target as HTMLInputElement).checked)">首行缩进两字</label>
          <label><input type="checkbox" :checked="layout.header" @change="setOption('header',($event.target as HTMLInputElement).checked)">标题与案号页眉</label>
          <label><input type="checkbox" :checked="layout.skipFirstHeader" @change="setOption('skipFirstHeader',($event.target as HTMLInputElement).checked)">首页隐藏页眉</label>
        </div>
      </details>
    </div>
    <p v-if="error" class="typeset-error" role="alert">{{ error }} <button @click="schedule">重试</button></p>
    <p v-if="stale && result" class="typeset-notice">以下为上次排版，更新完成后可定位正文。</p>
    <p v-if="result?.warnings.length" class="typeset-notice">{{ result.warnings.join('；') }}</p>
    <div ref="scroller" class="typeset-pages" :aria-busy="busy">
      <p v-if="!result && !error">正在准备纸张预览…</p>
      <figure v-for="(page,index) in result?.pages || []" :key="index" class="typeset-page" :class="{stale}" :style="{aspectRatio:`${page.width}/${page.height}`}">
        <img :src="urls[index]" :alt="`文书第 ${index+1} 页`">
        <button v-for="anchor in result?.anchors.filter(a=>a.page===index+1) || []" :key="anchor.block"
          :data-block="anchor.block" class="typeset-anchor" :class="{active:anchor.block===activeBlock}" :disabled="stale"
          :style="{top:`${anchor.y/page.height*100}%`,left:`${Math.max(0,anchor.x/page.width*100-4)}%` }"
          :aria-label="`定位第 ${anchor.block+1} 个正文块`" :title="`返回第 ${anchor.block+1} 个正文块`" @click="emit('select-block',anchor.block)">↗</button>
        <figcaption>{{ index+1 }}</figcaption>
      </figure>
    </div>
  </aside>
</template>

<style scoped>
.typeset-preview{display:flex;flex-direction:column;min-width:0;min-height:0;background:var(--c-bg-subtle,#eee);border-left:1px solid var(--c-border);}
.typeset-controls{padding:12px 16px;display:flex;flex-wrap:wrap;align-items:center;gap:10px;background:var(--c-bg-card);border-bottom:1px solid var(--c-border);font-size:12px;}
.typeset-controls>span{margin-left:auto;color:var(--c-text-secondary)}
details{width:100%}summary{cursor:pointer}.layout-options{display:flex;flex-wrap:wrap;gap:10px;padding-top:10px}.layout-options label{display:flex;gap:6px;align-items:center}.layout-options input[type=number]{width:55px;padding:3px;background:var(--c-bg-card);color:var(--c-text);border:1px solid var(--c-border)}
.typeset-pages{overflow:auto;padding:20px;flex:1;min-height:0;overflow-anchor:none;}
.typeset-page{position:relative;margin:0 0 30px;background:white;box-shadow:0 2px 10px #0002;width:100%;}.typeset-page.stale{opacity:.55}.typeset-page img{display:block;width:100%;height:100%}.typeset-page figcaption{position:absolute;bottom:-22px;left:0;right:0;text-align:center;color:var(--c-text-secondary);font-size:11px}
.typeset-anchor{position:absolute;transform:translateY(-25%);width:22px;height:22px;border:0;border-radius:4px;color:#607080;background:#edf2f7;cursor:pointer;opacity:.4;padding:0}.typeset-anchor:hover,.typeset-anchor:focus-visible,.typeset-anchor.active{opacity:1;background:#dbeafe;color:#155ab6}.typeset-anchor:disabled{pointer-events:none}
.typeset-error,.typeset-notice{font-size:12px;padding:8px 14px;margin:0;line-height:1.6}.typeset-error{color:var(--c-danger,#b42318)}
</style>
