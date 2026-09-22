// @vitest-environment jsdom
import {beforeAll,describe,it,expect,vi} from 'vitest'
import {cardFor,collect,appendEdge,hydrate,revise,snapshot,type Converter} from '../../src/modules/whiteboard/lib/document'
import type {CanvasFact} from '../../src/types/bindings'
let convert:Converter
beforeAll(async()=>{
 vi.stubGlobal('FontFace',class {family:string;status='loaded';constructor(family:string){this.family=family}load(){return Promise.resolve(this)}})
 Object.defineProperty(document,'fonts',{value:{check:()=>true,add:()=>{},load:()=>Promise.resolve([]),ready:Promise.resolve()},configurable:true})
 HTMLCanvasElement.prototype.getContext=vi.fn(()=>({measureText:(t:string)=>({width:t.length*9,actualBoundingBoxAscent:14,actualBoundingBoxDescent:4}),font:''})) as any
 convert=(await import('@excalidraw/excalidraw')).convertToExcalidrawElements
})
const fact=(id='a'):CanvasFact=>({id,fileId:null,knowledgeId:null,sourceCaseId:null,sourceTitle:'手动事实',page:null,excerpt:'期限从转文日起算',note:null,x:id==='a'?0:500,y:0})
describe('canonical whiteboard with real Excalidraw conversion',()=>{
 it('converts readable cards and bound arrows into canonical facts and relations',()=>{
  let elements=[...cardFor(fact(),convert),...cardFor(fact('b'),convert)]
  elements=appendEdge(elements,{id:'ab',sourceNodeId:'a',targetNodeId:'b'},convert)
  expect(collect(elements)).toEqual({facts:[fact(),fact('b')],edges:[{id:'ab',sourceNodeId:'a',targetNodeId:'b'}]})
 })
 it('collects in-canvas edits and moves; deletion and undo include relationships',()=>{
  let elements=appendEdge([...cardFor(fact(),convert),...cardFor(fact('b'),convert)],{id:'ab',sourceNodeId:'a',targetNodeId:'b'},convert)
  const text=elements.find(e=>e.containerId==='fact-a')!;text.originalText='修改后的摘录';elements.find(e=>e.id==='fact-a')!.x=45
  expect(collect(elements).facts[0]).toMatchObject({excerpt:'修改后的摘录',x:45})
  expect(collect(elements.map(e=>e.id==='fact-a'?{...e,isDeleted:true}:e))).toMatchObject({facts:[fact('b')],edges:[]})
  expect(collect(elements).edges).toHaveLength(1)
 })
 it('hydrates legacy facts and canonical external edits without losing screenshots',()=>{
  const doc:any={scene:{sceneJson:JSON.stringify({elements:[{id:'img',type:'image',fileId:'p'}],appState:{},files:{p:{dataURL:'data:image/png;base64,AA=='}}})},facts:[fact()],edges:[]}
  const initial=hydrate(doc,convert);expect(initial.migrated).toBe(true);expect(collect(initial.scene.elements).facts).toEqual([fact()]);expect(initial.scene.files.p).toBeDefined()
  doc.scene.sceneJson=JSON.stringify(initial.scene);doc.facts[0]={...fact(),x:90,excerpt:'外部更新',sourceTitle:'新来源'}
  const loaded=hydrate(doc,convert);expect(collect(loaded.scene.elements).facts).toEqual(doc.facts)
  expect(loaded.scene.elements.find(e=>e.customData?.casy?.captionFor)?.text).toBe('新来源')
 })
 it('preserves resized card width on sidebar edit and keeps image data for export',()=>{
  const elements=cardFor(fact(),convert,600);const edited=revise(elements,{...fact(),note:'核实',excerpt:'新的内容'},convert)
  expect(edited.find(e=>e.id==='fact-a')?.width).toBe(600)
  expect(collect(edited).facts[0]).toMatchObject({note:'核实',excerpt:'新的内容'})
  const scene=snapshot([...edited,{type:'image',fileId:'p'}],{scrollX:1,selectedElementIds:{a:true}},{p:{dataURL:'x'},unused:{dataURL:'y'}})
  expect(scene.files).toEqual({p:{dataURL:'x'}});expect(scene.appState.selectedElementIds).toBeUndefined()
 })
})

import {normalizeFacts} from '../../src/modules/whiteboard/lib/document'
it('repairs pasted fact identity, caption ownership and blank cards without blocking the board',()=>{
 const original=cardFor(fact(),convert)
 const copies=original.map(e=>({...e,id:'copy-'+e.id,containerId:e.containerId?'copy-'+e.containerId:null,groupIds:e.groupIds.map((g:string)=>'copy-'+g),boundElements:e.boundElements?.map((b:any)=>({...b,id:'copy-'+b.id}))}))
 const fixed=normalizeFacts([...original,...copies])
 const facts=collect(fixed).facts
 expect(facts).toHaveLength(2);expect(new Set(facts.map(f=>f.id)).size).toBe(2)
 const caption=fixed.find(e=>e.id.startsWith('copy-source-'))!
 expect(caption.customData.casy.captionFor).toBe(facts[1]!.id)
 expect(normalizeFacts(fixed)).toEqual(fixed)
 const blank=fixed.map(e=>e.containerId==='fact-a'?{...e,text:'',originalText:''}:e)
 expect(collect(normalizeFacts(blank)).facts).toHaveLength(1)
 expect(normalizeFacts(blank).find(e=>e.id==='fact-a')!.isDeleted).not.toBe(true)
})
it('collect keeps unique facts after normalize and never emits empty excerpts',()=>{
 const original=cardFor(fact(),convert)
 const copies=original.map(e=>({...e,id:'pasted-'+e.id,containerId:e.containerId?'pasted-'+(e.containerId==='fact-a'?'fact-a':e.containerId):undefined,boundElements:undefined,groupIds:(e.groupIds||[]).map((g:string)=>'pasted-'+g)}))
 for(const e of copies) if(e.customData?.casy?.fact) e.customData={...e.customData,casy:{...e.customData.casy,fact:{...e.customData.casy.fact}}}
 const fixed=normalizeFacts([...original,...copies],false)
 const result=collect(fixed)
 expect(result.facts).toHaveLength(2)
 expect(new Set(result.facts.map(f=>f.id)).size).toBe(2)
 // scene customData ids must match collected fact ids (backend save contract)
 for(const f of result.facts){
  const card=fixed.find(e=>e.type==='rectangle' && e.customData?.casy?.fact?.id===f.id)
  expect(card).toBeTruthy()
 }
 const emptied=fixed.map(e=>e.containerId==='fact-a'||e.containerId==='pasted-fact-a'?{...e,text:'',originalText:''}:e)
 const blank=collect(emptied)
 expect(blank.facts.every(f=>f.excerpt.trim().length>0)).toBe(true)
})
