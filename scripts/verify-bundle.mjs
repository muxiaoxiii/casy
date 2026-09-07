import { readFile,stat } from 'node:fs/promises'
import { createReadStream } from 'node:fs'
import { createHash } from 'node:crypto'
import { join,resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'

const app=resolve(process.argv[2]||'src-tauri/target/release/bundle/macos/Casy.app')
const runtime=join(app,'Contents/Resources/runtime')
execFileSync('codesign',['--verify','--deep','--strict',app],{stdio:'pipe'})
const manifest=JSON.parse(await readFile(join(runtime,'manifest.json'),'utf8'))
for(const file of manifest.files){
  const absolute=join(runtime,file.path)
  assert.equal((await stat(absolute)).size,file.bytes,file.path)
  const hash=createHash('sha256')
  for await(const chunk of createReadStream(absolute))hash.update(chunk)
  assert.equal(hash.digest('hex'),file.sha256,file.path)
  if(file.path.endsWith('.dylib')||file.path==='bin/pdftoppm'){
    const libraries=execFileSync('otool',['-L',absolute],{encoding:'utf8'}).split('\n').slice(1).map(line=>line.trim().split(' (')[0]).filter(Boolean)
    assert(libraries.every(library=>library.startsWith('/usr/lib/')||library.startsWith('/System/')||library.startsWith('@loader_path/')),`External library dependency: ${file.path}`)
  }
}
const env={...process.env,PATH:'/usr/bin:/bin'}
for(const key of Object.keys(env))if(key.startsWith('CASY_'))delete env[key]
const probe=JSON.parse(execFileSync(join(runtime,'bin/casy-doc-engine'),['probe'],{env,encoding:'utf8'}))
assert(probe.available,JSON.stringify(probe))
const embeddings=JSON.parse(execFileSync(join(runtime,'bin/casy-doc-engine'),['embed'],{env:{...env,CASY_EMBEDDING_MODEL_DIR:join(runtime,'models/embedding-e5-base')},input:JSON.stringify({inputs:['专利侵权赔偿','Prüfung français 日本語'],query:true})+'\n',encoding:'utf8',timeout:30000}))
assert.equal(embeddings.embeddings?.length,2,JSON.stringify(embeddings))
assert(embeddings.embeddings.every(vector=>vector.length===768&&vector.every(Number.isFinite)))
console.log(JSON.stringify({app,files:manifest.files.length,runtimeBytes:manifest.files.reduce((sum,file)=>sum+file.bytes,0),signature:'ad-hoc verified',probe,embeddingDimensions:768}))
