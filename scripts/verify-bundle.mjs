import { readFile,stat } from 'node:fs/promises'
import { createReadStream } from 'node:fs'
import { createHash } from 'node:crypto'
import { join,resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { sourceArchiveRequired } from './license-policy.mjs'
import { assertBinaryTarget } from './binary-architecture.mjs'

const app=resolve(process.argv[2]||'src-tauri/target/release/bundle/macos/Casy.app')
const windows=process.platform==='win32'
const runtime=join(app,windows?'runtime':'Contents/Resources/runtime')
const executable=join(app,windows?'casy.exe':'Contents/MacOS/casy')
const engine=join(runtime,windows?'bin/casy-doc-engine.exe':'bin/casy-doc-engine')
if(!windows) execFileSync('codesign',['--verify','--deep','--strict',app],{stdio:'pipe'})
const manifest=JSON.parse(await readFile(join(runtime,'manifest.json'),'utf8'))
assert.equal(manifest.platform, process.platform)
await assertBinaryTarget(executable, manifest.platform, manifest.arch)
for(const file of manifest.files){
  const absolute=join(runtime,file.path)
  assert.equal((await stat(absolute)).size,file.bytes,file.path)
  const hash=createHash('sha256')
  for await(const chunk of createReadStream(absolute))hash.update(chunk)
  assert.equal(hash.digest('hex'),file.sha256,file.path)
  if(windows && /^zvec\/.*\.dll$/.test(file.path)) {
    const installedHash=createHash('sha256').update(await readFile(join(app,file.path.slice(5)))).digest('hex')
    assert.equal(installedHash,file.sha256,`Loader copy: ${file.path}`)
  }
  // licenses/ 下是依赖许可证与配方脚本（如 PDF 渲染器 recipes 附带的
  // tls12-download-arm64.exe 辅助工具），属于溯源产物而非运行时二进制，不参与架构校验。
  if(!file.path.startsWith('licenses/')&&(/\.(dll|exe|dylib)$/.test(file.path)||['bin/pdftoppm','bin/casy-doc-engine'].includes(file.path))) await assertBinaryTarget(absolute,manifest.platform,manifest.arch)
  if(file.path.endsWith('.dylib')||file.path==='bin/pdftoppm'){
    const libraries=execFileSync('otool',['-L',absolute],{encoding:'utf8'}).split('\n').slice(file.path.endsWith('.dylib')?2:1).map(line=>line.trim().split(' (')[0]).filter(Boolean)
    assert(libraries.every(library=>library.startsWith('/usr/lib/')||library.startsWith('/System/')||library.startsWith('@loader_path/')),`External library dependency: ${file.path}`)
  }
}
assert.equal(manifest.delivery, 'full')
assert.equal(manifest.schemaVersion, 2)
const componentFiles = manifest.components.flatMap(component => component.files)
assert.equal(new Set(componentFiles).size, manifest.files.length)
assert.deepEqual([...componentFiles].sort(), manifest.files.map(file => file.path).sort())
for (const component of manifest.components) {
  assert.equal(component.delivery, 'bundled')
  assert.equal(component.bytes, manifest.files.filter(file => component.files.includes(file.path)).reduce((sum, file) => sum + file.bytes, 0))
}
// 默认档位：OCR small（models/ocr/small）+ 向量 e5-small；medium/base 为可选下载档
for (const id of ['embedding-e5-small','ocr-small','ocr-korean-ppocrv5-mobile','layout']) assert(manifest.components.some(component => component.id === id && component.bytes > 0), `缺少运行时组件: ${id}`)
assert(!manifest.files.some(file => /^zvec\/sdk-.*\.tar\.gz$/.test(file.path)))
const licenseRoot=join(runtime,'licenses/packages')
const dependencies=JSON.parse(await readFile(join(licenseRoot,'index.json'),'utf8'))
assert(dependencies.length>0)
for(const dependency of dependencies){
  for(const notice of dependency.notices) assert((await stat(join(licenseRoot,notice))).isFile(),notice)
  const mustBundle=sourceArchiveRequired(dependency.license)||dependency.notices.length===0
  assert.equal(dependency.sourceDelivery,mustBundle?'bundled':'build-cache',dependency.name)
  if(mustBundle){
    assert(dependency.source,dependency.name)
    const digest=createHash('sha256')
    for await(const chunk of createReadStream(join(licenseRoot,dependency.source)))digest.update(chunk)
    assert.equal(digest.digest('hex'),dependency.sourceSha256,dependency.name)
  }else assert.equal(dependency.source,null,dependency.name)
}
const env={...process.env,PATH:windows?`${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`:'/usr/bin:/bin'}
for(const key of Object.keys(env))if(key.startsWith('CASY_'))delete env[key]
const probe=JSON.parse(execFileSync(engine,['probe'],{env,encoding:'utf8'}))
assert(probe.available,JSON.stringify(probe))
const embeddings=JSON.parse(execFileSync(engine,['embed'],{env:{...env,CASY_EMBEDDING_MODEL_DIR:join(runtime,'models/embedding-e5-small')},input:JSON.stringify({inputs:['专利侵权赔偿','Prüfung français 日本語'],query:true})+'\n',encoding:'utf8',timeout:30000}))
assert.equal(embeddings.embeddings?.length,2,JSON.stringify(embeddings))
// 默认向量模型 multilingual-e5-small 为 384 维（e5-base 可选档为 768）
const EMBEDDING_DIMENSIONS=384
assert(embeddings.embeddings.every(vector=>vector.length===EMBEDDING_DIMENSIONS&&vector.every(Number.isFinite)))
const vectors=JSON.parse(execFileSync(executable,['--verify-vector-index'],{env,encoding:'utf8',timeout:30000}))
assert(vectors.available && vectors.engine==='zvec',JSON.stringify(vectors))
console.log(JSON.stringify({app,platform:manifest.platform,architecture:manifest.arch,nativeArchitecturesVerified:true,files:manifest.files.length,runtimeBytes:manifest.files.reduce((sum,file)=>sum+file.bytes,0),signature:windows?'unsigned':'ad-hoc verified',probe,embeddingDimensions:EMBEDDING_DIMENSIONS,vectors}))
