import { test } from 'node:test'
import assert from 'node:assert/strict'
import { componentInventory } from './runtime-components.mjs'
test('full distribution assigns every file once and retains all model groups', () => {
  const files = [{path:'models/embedding-e5-small/model_int8.onnx',bytes:10},{path:'models/embedding-e5-small/tokenizer.json',bytes:2},{path:'models/ocr/small/rec.onnx',bytes:3},{path:'models/korean-ppocrv5-mobile/rec.onnx',bytes:3},{path:'models/layout/model.onnx',bytes:4},{path:'licenses/model.txt',bytes:1},{path:'bin/engine',bytes:5}]
  const groups = componentInventory(files)
  assert.equal(groups.reduce((sum,g)=>sum+g.bytes,0),28)
  assert.deepEqual(groups.flatMap(g=>g.files).sort(),files.map(f=>f.path).sort())
  assert(groups.every(g=>g.delivery==='bundled'))
  assert.equal(groups.find(g=>g.id==='embedding-e5-small').bytes,12)
  assert.equal(groups.find(g=>g.id==='ocr-small').bytes,3)
})
test('legacy medium layout still maps to its own component', () => {
  const groups = componentInventory([{path:'models/ocr/medium/det.onnx',bytes:7},{path:'models/ppocrv6-medium/rec.onnx',bytes:3}])
  assert.equal(groups.find(g=>g.id==='ocr-medium').bytes,7)
  // 旧布局（无 tier 目录）归入 document-runtime，不会被误判为某个档位
  assert.equal(groups.find(g=>g.id==='document-runtime').bytes,3)
})
