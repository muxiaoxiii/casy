import type { CanvasFact, CanvasEdge, WhiteboardDocument } from '../../../types/bindings'
// Serialized editor elements are normalized by Excalidraw at the rendering boundary.
export type Element = Record<string, any>
export type Scene = { elements: Element[]; appState: Record<string, any>; files: Record<string, any> }
export type Converter = (elements: any[], options?: {regenerateIds: boolean}) => readonly any[]
export const defaults = {currentItemFontFamily:2,currentItemFontSize:18,currentItemRoughness:0,currentItemStrokeWidth:1,currentItemFillStyle:'solid',currentItemStrokeColor:'#475569',currentItemBackgroundColor:'transparent',currentItemTextAlign:'left',currentItemRoundness:'round',viewBackgroundColor:'#ffffff',gridSize:20,gridModeEnabled:false,objectsSnapModeEnabled:true,exportBackground:true,exportWithDarkMode:false}
export function factLink(f:CanvasFact) {
  if(f.fileId && f.sourceCaseId)return `casy-ref:file:${encodeURIComponent(f.fileId)}:${encodeURIComponent(f.sourceCaseId)}:${f.page || ''}`
  if(f.knowledgeId)return `casy-ref:knowledge:${encodeURIComponent(f.knowledgeId)}`
  return null
}
export function sourceLabel(f:CanvasFact) {return `${f.sourceTitle || '手动事实'}${f.page ? ` · 第 ${f.page} 页` : ''}`}
export function cardFor(f:CanvasFact,convert:Converter,width=340):Element[] {
  const id=`fact-${f.id}`
  const items=convert([{id,type:'rectangle',x:f.x,y:f.y,width,height:160,roughness:0,strokeWidth:1,strokeColor:'#475569',backgroundColor:f.knowledgeId?'#eef4ff':'#fff8e7',fillStyle:'solid',roundness:{type:3},groupIds:[id],link:factLink(f),customData:{casy:{fact:f}},label:{text:f.excerpt,fontSize:18,fontFamily:2,textAlign:'left',verticalAlign:'middle'}}],{regenerateIds:false}) as Element[]
  const caption=convert([{id:`source-${f.id}`,type:'text',x:f.x,y:f.y-25,text:sourceLabel(f).slice(0,65),fontSize:13,fontFamily:2,strokeColor:'#64748b',groupIds:[id],link:factLink(f),customData:{casy:{captionFor:f.id}}}],{regenerateIds:false}) as Element[]
  return [...items,...caption].map(e=>({...e,groupIds:[id]}))
}
export function collect(elements:readonly Element[]):{facts:CanvasFact[];edges:CanvasEdge[]} {
  const active=elements.filter(e=>!e.isDeleted)
  const seen=new Set<string>()
  const factIdByElement=new Map<string,string>()
  const facts:CanvasFact[]=[]
  for(const e of active){
    if(e.type!=='rectangle' || !e.customData?.casy?.fact)continue
    const label=active.find(t=>t.type==='text' && t.containerId===e.id)
    let id=String(e.customData.casy.fact.id || '')
    // 场景与 facts 必须共用同一 ID（后端 save 校验画布卡片匹配）。
    // 重复 ID 由 normalizeFacts 在写回场景时重分配；这里只兜底空摘录。
    if(!id) id=`fact-${e.id}`
    if(seen.has(id)) continue
    seen.add(id)
    factIdByElement.set(e.id,id)
    const raw=label?.originalText ?? label?.text ?? ''
    facts.push({...e.customData.casy.fact,id,excerpt:raw.trim()?raw:'（空白事实）',x:e.x,y:e.y} as CanvasFact)
  }
  const edges=active.filter(e=>e.type==='arrow' && factIdByElement.has(e.startBinding?.elementId) && factIdByElement.has(e.endBinding?.elementId)).map(e=>({id:e.customData?.casy?.edgeId || e.id,sourceNodeId:factIdByElement.get(e.startBinding.elementId)!,targetNodeId:factIdByElement.get(e.endBinding.elementId)!}))
  return {facts,edges}
}
export function appendEdge(elements:Element[],edge:CanvasEdge,convert:Converter):Element[] {
  const from=elements.find(e=>!e.isDeleted && e.customData?.casy?.fact?.id===edge.sourceNodeId)
  const to=elements.find(e=>!e.isDeleted && e.customData?.casy?.fact?.id===edge.targetNodeId)
  if(!from || !to)return elements
  const id=`edge-${edge.id}`, x=from.x+from.width+4,y=from.y+from.height/2
  const points=[[0,0],[to.x-4-x,to.y+to.height/2-y]]
  const arrow=convert([{id,type:'arrow',x,y,points,roughness:0,strokeWidth:1.5,strokeColor:'#64748b',endArrowhead:'arrow',startBinding:{elementId:from.id,focus:0,gap:4,fixedPoint:null},endBinding:{elementId:to.id,focus:0,gap:4,fixedPoint:null},customData:{casy:{edgeId:edge.id}}}],{regenerateIds:false}) as Element[]
  for(const item of arrow)if(item.type==='arrow'){item.startBinding={elementId:from.id,focus:0,gap:4,fixedPoint:null};item.endBinding={elementId:to.id,focus:0,gap:4,fixedPoint:null}}
  return [...elements.map(e=>e.id===from.id || e.id===to.id?{...e,boundElements:[...(e.boundElements || []),{id,type:'arrow'}]}:e),...arrow]
}
export function hydrate(document:WhiteboardDocument,convert:Converter):{scene:Scene;migrated:boolean} {
  const original=JSON.parse(document.scene.sceneJson) as Scene
  let elements=[...original.elements]
  let migrated=false
  const canonical=new Map(document.facts.map(f=>[f.id,f]))
  // Upgrade the old one-way fact snapshots in place. Preserve nonstandard authored text as a note.
  for(const f of document.facts) {
    const card=elements.find(e=>!e.isDeleted && (e.customData?.casy?.fact?.id===f.id || e.customData?.factId===f.id))
    if(!card){elements.push(...cardFor(f,convert));migrated=true;continue}
    const text=elements.find(e=>!e.isDeleted && e.type==='text' && e.containerId===card.id)
    const previous=card.customData?.casy?.fact
    if(!previous){
      const expected=`${f.excerpt}\n${f.note || ''}\n${f.sourceTitle === '手动事实' ? '手动事实' : f.sourceTitle}${f.page?` · P${f.page}`:''}`
      const oldText=text?.originalText ?? text?.text ?? ''
      if(oldText && oldText!==expected && oldText!==f.excerpt){
        const copy=convert([{type:'text',x:card.x,y:card.y+card.height+30,text:`原画布内容（保留）\n${oldText}`,fontFamily:2,fontSize:16}],{regenerateIds:false})
        elements.push(...copy)
      }
      migrated=true
    }
    const x=previous?f.x:card.x,y=previous?f.y:card.y
    const replacement=cardFor({...f,x,y},convert,card.width)
    const bound=replacement.find(e=>e.type==='text' && e.containerId)
    elements=elements.map(e=>{
      if(e.id===card.id)return {...e,x,y,groupIds:[card.id],customData:{...e.customData,casy:{fact:{...f,x,y}}},link:factLink(f)}
      if(e.id===text?.id && bound)return {...e,x:e.x+(x-card.x),y:e.y+(y-card.y),groupIds:[card.id],...((e.originalText ?? e.text)===f.excerpt?{}:{text:bound.text,originalText:f.excerpt,width:bound.width,height:bound.height})}
      if(e.customData?.casy?.captionFor===f.id){const caption=replacement.find(c=>c.customData?.casy?.captionFor)!;return {...e,x:e.x+(x-card.x),y:e.y+(y-card.y),text:caption.text,originalText:caption.originalText,width:caption.width,groupIds:[card.id],link:factLink(f)}}
      return e
    })
    if(!text){elements=elements.map(e=>e.id===card.id?{...e,boundElements:[...(e.boundElements||[]),{id:bound!.id,type:'text'}]}:e);elements.push({...bound!,containerId:card.id})}
    // Captions are linked to the source, not an editable copy of the fact body.
    if(!elements.some(e=>!e.isDeleted && e.customData?.casy?.captionFor===f.id))elements.push({...replacement.find(e=>e.customData?.casy?.captionFor)!,groupIds:[card.id]})
  }
  // Old reference-only rectangles become real facts without dropping their authored text.
  for(const card of elements.filter(e=>!e.isDeleted && e.type==='rectangle' && !e.customData?.casy?.fact && !e.customData?.factId)) {
    const match=String(card.link || '').match(/^casy-ref:(file|knowledge):([^:]+)(?::([^:]*))?(?::(\d*))?$/)
    const text=elements.find(e=>!e.isDeleted && e.type==='text' && e.containerId===card.id)
    if(!match || !text)continue
    const excerpt=text.originalText ?? text.text ?? ''
    if(!excerpt.trim())continue
    const fact:CanvasFact={id:crypto.randomUUID(),fileId:match[1]==='file'?decodeURIComponent(match[2]!):null,knowledgeId:match[1]==='knowledge'?decodeURIComponent(match[2]!):null,sourceCaseId:match[1]==='file'?decodeURIComponent(match[3] || ''):null,sourceTitle:excerpt.split('\n')[0] || '引用来源',page:match[4]?Number(match[4]):null,excerpt,note:null,x:card.x,y:card.y}
    canonical.set(fact.id,fact)
    elements=elements.map(e=>e.id===card.id?{...e,customData:{...e.customData,casy:{fact}},link:factLink(fact)}:e)
    migrated=true
  }
  // A fact deleted through legacy tools must not reappear from an old saved card.
  elements=elements.map(e=>{
    const id=e.customData?.casy?.fact?.id || e.customData?.factId || e.customData?.casy?.captionFor
    const container=elements.find(c=>c.id===e.containerId)
    const owner=id || container?.customData?.casy?.fact?.id || container?.customData?.factId
    return owner && !canonical.has(owner)?{...e,isDeleted:true}:e
  })
  const existing=collect(elements).edges
  for(const edge of document.edges)if(!existing.some(e=>e.id===edge.id)){elements=appendEdge(elements,edge,convert);migrated=true}
  return {scene:{...original,elements,appState:{...defaults,...original.appState,activeTool:{type:'selection'}}},migrated:migrated || JSON.stringify(elements)!==JSON.stringify(original.elements)}
}
export function snapshot(elements:readonly Element[],state:Record<string,any>,files:Record<string,any>):Scene {
  const used=new Set(elements.filter(e=>!e.isDeleted && e.type==='image').map(e=>e.fileId))
  const keys=[...Object.keys(defaults),'scrollX','scrollY','zoom','currentItemStrokeStyle','currentItemStartArrowhead','currentItemEndArrowhead']
  return {elements:[...elements],appState:Object.fromEntries(keys.filter(k=>state[k]!==undefined).map(k=>[k,state[k]])),files:Object.fromEntries(Object.entries(files).filter(([id])=>used.has(id)))}
}
export function revise(elements:Element[],fact:CanvasFact,convert:Converter):Element[] {
  const card=elements.find(e=>!e.isDeleted && e.customData?.casy?.fact?.id===fact.id)
  if(!card)return [...elements,...cardFor(fact,convert)]
  const replacement=cardFor({...fact,x:card.x,y:card.y},convert,card.width)
  const bound=replacement.find(e=>e.type==='text' && e.containerId)!
  const caption=replacement.find(e=>e.customData?.casy?.captionFor)!
  return elements.map(e=>{
    if(e.id===card.id)return {...e,customData:{...e.customData,casy:{fact}},link:factLink(fact),height:Math.max(card.height,bound.height+20),version:e.version+1,versionNonce:Math.floor(Math.random()*2**31)}
    if(e.containerId===card.id && e.type==='text')return {...e,text:bound.text,originalText:fact.excerpt,width:bound.width,height:bound.height,version:e.version+1,versionNonce:Math.floor(Math.random()*2**31)}
    if(e.customData?.casy?.captionFor===fact.id)return {...e,text:caption.text,originalText:caption.originalText,width:caption.width,link:factLink(fact),version:e.version+1,versionNonce:Math.floor(Math.random()*2**31)}
    return e
  })
}

/** Clipboard duplication copies customData. Give each drawing its own fact/edge identity. */
export function normalizeFacts(elements: readonly Element[], demoteEmpty=true, owners=new Map<string,string>()): Element[] {
  const cards=elements.filter(e=>!e.isDeleted && e.type==='rectangle' && e.customData?.casy?.fact)
  const byId=new Map<string,Element>()
  for(const card of cards){const id=card.customData.casy.fact.id;if(!byId.has(id) || card.id===`fact-${id}`)byId.set(id,card)}
  const changes=new Map<string, string|null>()
  const seen=new Set<string>()
  for(const card of cards){
    const id=card.customData.casy.fact.id
    const text=elements.find(e=>!e.isDeleted && e.type==='text' && e.containerId===card.id)
    if(demoteEmpty && !String(text?.originalText ?? text?.text ?? '').trim())changes.set(card.id,null)
    else if(byId.get(id)!==card || (owners.get(id)!==card.id && card.id!==`fact-${id}` && id!==`copy-${card.id}`))changes.set(card.id,`copy-${card.id}`)
  }
  return elements.map(element=>{
    let e=element
    const card=cards.find(c=>c.id===e.id || (e.customData?.casy?.captionFor===c.customData.casy.fact.id && c.groupIds?.length && e.groupIds?.some((g:string)=>c.groupIds.includes(g))))
    if(card && changes.has(card.id)){
      const id=changes.get(card.id)
      const casy={...e.customData?.casy}
      if(e.id===card.id){if(id)casy.fact={...casy.fact,id};else delete casy.fact}
      else {if(id)casy.captionFor=id;else delete casy.captionFor}
      e={...e,customData:{...e.customData,casy},version:(e.version || 0)+1}
    }
    if(!e.isDeleted && e.type==='arrow' && e.customData?.casy?.edgeId){
      const id=e.customData.casy.edgeId
      if(seen.has(id))e={...e,customData:{...e.customData,casy:{...e.customData.casy,edgeId:`copy-${e.id}`}},version:(e.version || 0)+1}
      seen.add(e.customData.casy.edgeId)
    }
    return e
  })
}
