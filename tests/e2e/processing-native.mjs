// Native commands only; every operation uses a fresh isolated test profile.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import assert from 'node:assert/strict'
const profile=fs.mkdtempSync(path.join(os.tmpdir(),'casy-processing-native-'))
const source=path.join(profile,'证据.md'),invalid=path.join(profile,'不支持.foo'),output=path.join(profile,'output')
fs.mkdirSync(output)
const original='# 证据\n\n| 项目 | 数值 |\n|---|---|\n| 样品 | 100 |\n'
fs.writeFileSync(source,original);fs.writeFileSync(invalid,'invalid')
function invoke(command,args={}) {
 const child=spawnSync('src-tauri/target/debug/examples/knowledge_local_bridge',[],{env:{...process.env,CASY_TEST_DATA_DIR:profile},input:JSON.stringify({command,args}),encoding:'utf8'})
 assert.equal(child.status,0,child.stderr)
 return JSON.parse(child.stdout)
}
function call(command,args) { const result=invoke(command,args);assert.equal(result.ok,true,result.error);return result.data }
const ids=call('register_conversion_batch',{sourcePaths:[source,invalid,source]})
assert.equal(new Set(ids).size,3)
assert.equal(call('get_processing_center',{filter:'active'}).active,3)
const converted=call('convert_file_to_markdown',{sourcePath:source,outputDir:output,jobId:ids[0]})
assert.equal(fs.readFileSync(source,'utf8'),original)
assert.equal(fs.readFileSync(converted.outputPath,'utf8'),original)
assert.equal(invoke('convert_file_to_markdown',{sourcePath:invalid,outputDir:output,jobId:ids[1]}).ok,false)
call('cancel_queued_conversions',{jobIds:[ids[2]]})
assert.equal(invoke('convert_file_to_markdown',{sourcePath:source,outputDir:output,jobId:ids[2]}).ok,false)
const history=call('get_processing_center',{filter:'all',limit:30})
assert.equal(history.total,3);assert.equal(history.active,0);assert.equal(history.failed,1)
assert.equal(history.jobs.find(j=>j.id===ids[0]).outputPath,converted.outputPath)
assert.equal(history.jobs.find(j=>j.id===ids[2]).status,'cancelled')
assert.ok(history.jobs.find(j=>j.id===ids[1]).error.includes('不支持'))
console.log(JSON.stringify({profile,passed:true,queued:3,completed:1,failed:1,cancelled:1,originalUnchanged:true}))
