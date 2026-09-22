import test from 'node:test'
import assert from 'node:assert/strict'
import {sourceArchiveRequired} from './license-policy.mjs'
test('only recognized permissive expressions may omit a bundled source archive',()=>{
  for(const value of ['MIT','(MIT OR Apache-2.0)','MIT/Apache-2.0','BSD-3-Clause AND ISC'])assert.equal(sourceArchiveRequired(value),false)
  for(const value of ['GPL-3.0-only','MPL-2.0','MIT OR GPL-2.0-only','LGPL-2.1-only WITH custom-exception','SEE LICENSE IN terms.txt','MIT OR','(MIT','MIT Apache-2.0','MIT))',null,{},''])assert.equal(sourceArchiveRequired(value),true)
})
