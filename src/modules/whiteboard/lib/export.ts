import { tauriCallSafe } from '../../../core/tauriBridge'
import { isTauriRuntime } from '../../../core/mockData'
import type { Scene } from './document'
export type ExportFormat='png'|'svg'|'excalidraw'
export async function blobData(blob:Blob):Promise<string>{return new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(new Error('无法读取导出文件'));reader.readAsDataURL(blob)})}
export async function exportWhiteboard(scene:Scene,format:ExportFormat,name:string,target:{caseId:string;whiteboardId:string},editor:typeof import('@excalidraw/excalidraw')):Promise<string|null>{
 if(!isTauriRuntime())throw new Error('请在桌面版 Casy 中导出画布')
 if(!scene.elements.some(e=>!e.isDeleted))throw new Error('画布为空，请先添加事实或图形')
 const {save}=await import('@tauri-apps/plugin-dialog')
 const outputPath=await save({defaultPath:`${name.replace(/[\\/:*?"<>|]/g,'_')}.${format}`,filters:[{name:format==='excalidraw'?'可编辑画布':format.toUpperCase(),extensions:[format]}]})
 if(!outputPath)return null
 let blob:Blob
 const data={...scene,elements:scene.elements.filter(e=>!e.isDeleted),appState:{...scene.appState,exportBackground:true,exportWithDarkMode:false,viewBackgroundColor:'#ffffff',exportEmbedScene:false}}
 if(format==='png')blob=await editor.exportToBlob({...data,mimeType:'image/png',maxWidthOrHeight:6000} as any)
 else if(format==='svg'){
   const svg=await editor.exportToSvg({...data,exportPadding:24} as any)
   blob=new Blob([new XMLSerializer().serializeToString(svg)],{type:'image/svg+xml'})
 }else blob=new Blob([JSON.stringify({type:'excalidraw',version:2,source:'Casy',...scene})],{type:'application/json'})
 const dataBase64=(await blobData(blob)).split(',')[1]!
 const result=await tauriCallSafe('write_whiteboard_export',{...target,outputPath,format,dataBase64})
 if(!result.ok || !result.data)throw new Error(result.error || '文件未能写入')
 return result.data
}
