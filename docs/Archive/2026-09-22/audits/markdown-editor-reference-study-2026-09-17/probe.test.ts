// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import { documentExtensions } from '../../../src/shared/editor/schema';
import { mdToHtml, htmlToMd } from '../../../src/shared/markdown/mdBridge';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import DocumentEditor from '../../../src/shared/editor/DocumentEditor.vue';

// Research observations, not a claim that all tested syntax is supported.
const samples = [
  { id: 'h5-h6', source: '##### 五级标题\n\n###### 六级标题\n\n正文' },
  { id: 'footnote', source: '主张依据[^law]。\n\n[^law]: 第六十五条。' },
  { id: 'inline-math', source: '损失为 $x_i + y_i$，以及 $a*b*c$。' },
  { id: 'mermaid', source: '```mermaid\ngraph LR\n  A[立案] --> B[开庭]\n```' },
  { id: 'table-alignment', source: '| 名称 | 金额 |\n| :--- | ---: |\n| 费用 | 100 |' },
  { id: 'reference-link', source: '参见[原件][evidence]。\n\n[evidence]: https://example.com/evidence "证据标题"' },
  { id: 'image-title', source: '![证据](asset://local/evidence.png "原件")\n\n正文' },
  { id: 'merged-table', source: '<table><tr><td colspan="2"><p>合并内容</p></td></tr></table>\n\n正文' },
  { id: 'ordered-start', source: '3. 第三项\n4. 第四项' },
  { id: 'unknown-html', source: '<details><summary>详情</summary>证据补充</details>\n\n正文' },
  { id: 'unicode', source: '𠮷野 e\u0301 👩🏽‍⚖️ 原件' },
];

describe('Casy editor research observations', () => {
  it('records actual Markdown → editor → edit → Markdown → reopen behavior', async () => {
    const results = samples.map(({id,source}) => {
      const editor = new Editor({ extensions: documentExtensions(), content: mdToHtml(source) });
      try {
        const initial = editor.getJSON();
        editor.commands.insertContentAt(editor.state.doc.content.size, {type:'paragraph',content:[{type:'text',text:'追加核查'}]});
        const html = editor.getHTML();
        const saved = htmlToMd(html);
        const reopened = new Editor({extensions:documentExtensions(),content:mdToHtml(saved)});
        try {
          expect(reopened.getText()).toContain('追加核查');
          return { id, source, initial, html, saved, reopened:reopened.getJSON(), text:reopened.getText() };
        } finally { reopened.destroy(); }
      } finally { editor.destroy(); }
    });
    const outputDir = process.env.CASY_EDITOR_STUDY_DIR!;
    writeFileSync(join(outputDir, 'observations.json'), JSON.stringify(results,null,2));
    setActivePinia(createPinia());
    const componentResults = [];
    for (const observation of results.filter(r => ['h5-h6','footnote','table-alignment','inline-math'].includes(r.id))) {
      const wrapper = mount(DocumentEditor, { attachTo:document.body, props:{modelValue:observation.source} });
      try {
        const api = wrapper.vm as any;
        for(let i=0;i<20 && !api.getEditor();i++) await new Promise(resolve=>setTimeout(resolve,0));
        const editor = api.getEditor();
        expect(editor).toBeTruthy();
        expect(api.flushAndGetMarkdown()).toBe(observation.source);
        editor.commands.insertContentAt(editor.state.doc.content.size,{type:'paragraph',content:[{type:'text',text:'追加核查'}]});
        const saved = api.flushAndGetMarkdown();
        // Checks both entry points agree; this does not assert semantic fidelity.
        expect(saved).toBe(observation.saved);
        componentResults.push({id:observation.id, unchangedFlushPreserved:true, afterUnrelatedEditMatchesProbe:true, saved});
      } finally { wrapper.unmount(); }
    }
    writeFileSync(join(outputDir, 'component-observations.json'), JSON.stringify(componentResults,null,2));
  });
});
