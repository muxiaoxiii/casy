// @vitest-environment jsdom
import {describe,it,expect,vi,beforeEach} from 'vitest'
import {exportDocument} from '../../src/shared/editor/exportDocument'
const mock=vi.hoisted(()=>({call:vi.fn(),save:vi.fn()}))
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:mock.call}))
vi.mock('@tauri-apps/plugin-dialog',()=>({save:mock.save}))
beforeEach(()=>vi.clearAllMocks())
describe('统一当前文档导出',()=>{
 it('源码模式直接生成导出树，保存取消不触发写入',async()=>{mock.save.mockResolvedValue(null);expect(await exportDocument({content:'- [x] 核对完成',contentFormat:'markdown',title:'测试',format:'pdf'})).toBeNull();expect(mock.call).not.toHaveBeenCalled()})
 it('MD、PDF、Word 使用相同结构化正文且保留待办',async()=>{mock.save.mockImplementation(async({defaultPath}:any)=>'/tmp/'+defaultPath);mock.call.mockImplementation(async(_name:string,args:any)=>({ok:true,data:{outputPath:args.outputPath}}));for(const format of ['md','pdf','docx'] as const){await exportDocument({content:'- [x] 核对完成',contentFormat:'markdown',title:'统一样例',format});const [command,args]=mock.call.mock.calls.at(-1)!;expect(command).toBe('export_editor_document');expect(args.document.content[0]).toMatchObject({type:'taskList',content:[{type:'taskItem',attrs:{checked:true}}]});expect(args.markdown).toBe('- [x] 核对完成')}})
 it('原生写入错误不显示成功',async()=>{mock.save.mockResolvedValue('/tmp/example.pdf');mock.call.mockResolvedValue({ok:false,error:'无法保存文件'});await expect(exportDocument({content:'正文',contentFormat:'markdown',title:'测试',format:'pdf'})).rejects.toThrow('无法保存文件')})
})
