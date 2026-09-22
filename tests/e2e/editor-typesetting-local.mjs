// Real Vue editor + native Typst command bridge, isolated from the user's documents.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawn} from 'node:child_process';
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright');
const profile=fs.mkdtempSync(path.join(os.tmpdir(),'casy-typeset-'));
const output=path.resolve(process.env.CASY_QA_DIR || profile);fs.mkdirSync(output,{recursive:true});
const binary=path.resolve('src-tauri/target/debug/examples/knowledge_local_bridge');
const calls=[];
function native(command,args){
  const started=Date.now();
  return new Promise((resolve,reject)=>{
    const child=spawn(binary,[],{env:{...process.env,CASY_TEST_DATA_DIR:profile},stdio:['pipe','pipe','pipe']});
    let stdout='',stderr='';child.stdout.on('data',v=>stdout+=v);child.stderr.on('data',v=>stderr+=v);child.on('error',reject);
    child.on('close',code=>{try{if(code)throw new Error(stderr);const reply=JSON.parse(stdout);if(!reply.ok)throw new Error(reply.error);if(reply.data?.pages)fs.writeFileSync(path.join(output,'page-1.svg'),reply.data.pages[0].svg);calls.push({command,wallMs:Date.now()-started,elapsedMs:reply.data?.elapsedMs,pages:reply.data?.pages?.length});resolve(reply.data);}catch(e){reject(e);}});
    child.stdin.end(JSON.stringify({command,args}));
  });
}
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1600,height:1000}}),errors=[];
page.on('pageerror',e=>errors.push(e.message));
await page.exposeFunction('__typesetNative',native);
try {
  await page.goto((process.env.CASY_QA_URL || 'http://127.0.0.1:1430/')+'#/cases');
  const skip=page.getByRole('button',{name:'暂时跳过'});await skip.waitFor();await skip.click();
  await page.evaluate(async()=>{
    const {tryMockCommand}=await import('/src/core/mockData.ts');
    const {mdToHtml}=await import('/src/shared/markdown/mdBridge.ts');
    const markdown='# 民事案件事实与证据\n\n依据[^law]，核对公式 $x_i + y_i$。\n\n```mermaid\nflowchart LR\nA[事实] --> B[证据]\n```\n\n| 证据名称 | 证明事项 |\n| :--- | ---: |\n| 合同原件 | 约定履行期限 |\n\n'+Array.from({length:70},(_,i)=>`第${i}项事实：核对当事人陈述与送达时间。`).join('\n\n')+'\n\n[^law]: 第六十五条。';
    const draft={id:'typeset-qa',title:'民事案件事实与证据',content:mdToHtml(markdown),caseId:'typeset-case',status:'draft',version:1};
    window.__typesetDraft=draft;
    window.__TAURI_INTERNALS__={invoke:async(command,args={})=>{
      if(command==='preview_editor_document'||command==='export_editor_document')return window.__typesetNative(command,args);
      if(command==='list_drafts')return [{...draft}];
      if(command==='get_draft')return {...draft};
      if(command==='update_draft'){Object.assign(draft,args,{version:draft.version+1});return {...draft};}
      if(command==='list_cases')return {items:[{id:'typeset-case',caseName:'合同纠纷',caseNo:'（2026）测试民初1号'}],total:1};
      if(command==='get_settings')return {};
      if(command==='get_lawyer_profile')return {onboarding_completed:true,name:'本地验收'};
      return tryMockCommand(command,args)??null;
    },transformCallback:()=>0,unregisterCallback:()=>{}};
    location.hash='/docs';
  });
  await page.locator('.tiptap h1').waitFor();
  await page.getByRole('button',{name:'A4 预览',exact:true}).click();
  await page.locator('.typeset-page:not(.stale)').first().waitFor({timeout:90000});
  const pages=await page.locator('.typeset-page').count();assert(pages>=3);
  await page.waitForFunction(()=>document.querySelector('[data-source-node=mermaid] svg')?.textContent.includes('事实'));
  await page.waitForFunction(()=>Array.from(document.querySelectorAll('.typeset-page img')).every(img=>img.complete&&img.naturalWidth>0));
  await page.screenshot({path:path.join(output,'a4-desktop.png')});
  const math=page.locator('[data-source-node="mathInline"]');await math.getByRole('button',{name:'编辑源码'}).click();
  await math.locator('textarea').fill('a*b*c');await math.getByRole('button',{name:'应用',exact:true}).click();
  await page.waitForFunction(()=>window.__typesetDraft.content.includes('a*b*c'));
  await page.locator('.typeset-page:not(.stale)').first().waitFor({timeout:90000});
  // Page anchor -> actual document selection; selection -> active page anchor.
  await page.locator('.typeset-anchor[data-block="20"]').click();
  assert(await page.evaluate(()=>document.getSelection()?.anchorNode?.parentElement?.closest('.tiptap')!==null));
  await page.locator('.typeset-anchor[data-block="20"].active').waitFor();
  await page.locator('.tiptap h1').click();await page.locator('.typeset-anchor[data-block="0"].active').waitFor();
  await page.locator('.typeset-controls summary').click();
  await page.getByRole('spinbutton',{name:'页边距',exact:true}).fill('25');
  await page.getByRole('spinbutton',{name:'页边距',exact:true}).press('Tab');
  await page.locator('.typeset-page:not(.stale)').first().waitFor({timeout:90000});
  const documentJson=await page.evaluate(async()=>{
    const {documentFromContent}=await import('/src/shared/editor/schema.ts');return documentFromContent(window.__typesetDraft.content,'html');
  });
  const pdfPath=path.join(output,'typeset-sample.pdf');
  await native('export_editor_document',{document:documentJson,markdown:'',title:'民事案件事实与证据',format:'pdf',outputPath:pdfPath,layout:{title:'民事案件事实与证据',caseNo:'（2026）测试民初1号',marginMm:25,bindingMm:5,firstLineIndent:true,header:true,skipFirstHeader:true}});
  assert(fs.readFileSync(pdfPath).subarray(0,5).toString()==='%PDF-');
  await page.setViewportSize({width:800,height:1000});await page.screenshot({path:path.join(output,'a4-narrow.png')});
  assert.equal(errors.length,0,errors.join('\n'));
  fs.writeFileSync(path.join(output,'results.json'),JSON.stringify({pages,calls,errors,checks:['native SVG pages loaded','local math edit autosaved','page-to-block focus','block-to-page highlight','layout update','native PDF export','narrow layout']},null,2));
  console.log(JSON.stringify({output,pages,calls,errors},null,2));
} catch(error){await page.screenshot({path:path.join(output,'failure.png')});fs.writeFileSync(path.join(output,'failure.json'),JSON.stringify({errors,calls,url:page.url()},null,2));throw error;}
finally{await browser.close();}
