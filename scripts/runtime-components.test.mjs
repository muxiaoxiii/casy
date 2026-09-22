import { test } from 'node:test'
import assert from 'node:assert/strict'
import { componentInventory } from './runtime-components.mjs'
test('full distribution assigns every file once and retains all model groups', () => {
  const files = [{path:'models/embedding-e5-base/model_int8.onnx',bytes:10},{path:'models/embedding-e5-base/tokenizer.json',bytes:2},{path:'models/ppocrv6-medium/rec.onnx',bytes:3},{path:'models/korean-ppocrv5-mobile/rec.onnx',bytes:3},{path:'models/layout/model.onnx',bytes:4},{path:'licenses/model.txt',bytes:1},{path:'bin/engine',bytes:5}]
  const groups = componentInventory(files)
  assert.equal(groups.reduce((sum,g)=>sum+g.bytes,0),28)
  assert.deepEqual(groups.flatMap(g=>g.files).sort(),files.map(f=>f.path).sort())
  assert(groups.every(g=>g.delivery==='bundled'))
  assert.equal(groups.find(g=>g.id==='embedding-e5-base').bytes,12)
})
