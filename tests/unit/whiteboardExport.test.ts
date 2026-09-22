// @vitest-environment jsdom
import {beforeEach,describe,it,expect,vi} from 'vitest'
import {exportWhiteboard} from '../../src/modules/whiteboard/lib/export'
const mocks=vi.hoisted(()=>({save:vi.fn(),call:vi.fn()}))
vi.mock('@tauri-apps/plugin-dialog',()=>({save:mocks.save}))
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:mocks.call}))
vi.mock('../../src/core/mockData',()=>({isTauriRuntime:()=>true}))
const scene={elements:[{id:'image',type:'image',fileId:'pic'}],appState:{},files:{pic:{dataURL:'data:image/png;base64,AA=='}}}
const target={caseId:'a',whiteboardId:'w'}
beforeEach(()=>vi.resetAllMocks())
describe('native whiteboard export',()=>{
 it('cancellation writes nothing and does not render a large image',async()=>{
  mocks.save.mockResolvedValue(null);const editor={exportToBlob:vi.fn()} as any
  expect(await exportWhiteboard(scene,'png','画布',target,editor)).toBeNull();expect(mocks.call).not.toHaveBeenCalled();expect(editor.exportToBlob).not.toHaveBeenCalled()
 })
 it('editable export carries screenshots and citations through native writer',async()=>{
  mocks.save.mockResolvedValue('/tmp/画布.excalidraw');mocks.call.mockResolvedValue({ok:true,data:'/tmp/画布.excalidraw'})
  expect(await exportWhiteboard(scene,'excalidraw','画布',target,{} as any)).toBe('/tmp/画布.excalidraw')
  const [cmd,args]=mocks.call.mock.calls[0]!;expect(cmd).toBe('write_whiteboard_export');expect(args).toMatchObject({...target,format:'excalidraw'})
  expect(JSON.parse(atob(args.dataBase64)).files).toEqual(scene.files)
 })
 it('native write failures remain visible to the caller',async()=>{
  mocks.save.mockResolvedValue('/tmp/board.png');mocks.call.mockResolvedValue({ok:false,error:'磁盘空间不足'})
  await expect(exportWhiteboard(scene,'png','画布',target,{exportToBlob:async()=>new Blob(['PNG'])} as any)).rejects.toThrow('磁盘空间不足')
 })
})
