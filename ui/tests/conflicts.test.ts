import { describe, expect, it } from 'vitest';
import { EditorState } from '@codemirror/state';
import { render } from 'svelte/server';
import FileTreeRow from '../src/features/browse/FileTreeRow.svelte';
import { fileGroup, fileRowLabel } from '../src/features/browse/fileTree';
import { needsCloseConfirmation, replacement, type MergeBlock } from '../src/features/conflicts/model';
import MergeEditor from '../src/features/conflicts/MergeEditor.svelte';
import { conflictText } from '../src/i18n/conflict';

const block: MergeBlock = { from: 7, to: 11, base: 'base', left: 'ours\r\n', right: 'theirs', resolved: false };
describe('three way merge choices', () => {
  for (const [choice, expected] of [['left','prefix ours\r\n end'],['right','prefix theirs end'],['both','prefix ours\r\ntheirs end'],['ignore','prefix base end']] as const) {
    it(`accepts ${choice} and preserves the surrounding manually editable result`, () => {
      const state = EditorState.create({ doc: 'prefix base end', extensions: [EditorState.lineSeparator.of('\n')] });
      const text = replacement(block,choice);
      const result = text === null ? state : state.update({ changes: { from: block.from, to: block.to, insert: text } }).state;
      expect(result.doc.toString()).toBe(expected);
    });
  }
  it('confirms unfinished or edited results before closing', () => {
    expect(needsCloseConfirmation([block],false)).toBe(true);
    expect(needsCloseConfirmation([{ ...block,resolved:true }],true)).toBe(true);
    expect(needsCloseConfirmation([{ ...block,resolved:true }],false)).toBe(false);
  });
  it('keeps save disabled until every conflict is reviewed and offers all four choices', () => {
    const body = render(MergeEditor,{ props: { content: { fingerprint:'f',left:'ours',right:'theirs',result:'base',blocks:[block],readOnly:null },rebase:true,busy:false,onsave:()=>{},onclose:()=>{} } }).body;
    expect(body).toContain(conflictText('rebased'));
    expect(body).toContain(conflictText('upstream'));
    expect(body).toContain(conflictText('both'));
    expect(body).toContain(conflictText('ignore'));
    expect(body).toMatch(new RegExp(`<button[^>]*disabled[^>]*>${conflictText('save')}`));
  });
});
it('renders a separate non-selectable conflict group and escapes repository text', () => {
  expect(fileGroup('conflicts')).toBe(true);
  const row = { id:'conflicts',label:'',path:null,status:null,note:null,staged:false,unstaged:false,conflicted:false,count:2,hasChildren:true,level:0,parentIndex:-1 };
  expect(fileRowLabel(row,false)).toContain(conflictText('title'));
  expect(render(FileTreeRow,{ props:{file:row,flat:true,changes:true,query:''} }).body).toContain(conflictText('title'));
  const file = { ...row,id:'conflict/01',label:'<script>',path:{bytes:[1],display:'<script>'},conflicted:true,hasChildren:false };
  const body = render(FileTreeRow,{ props:{file,flat:true,changes:true,query:'',ontoggle:()=>{}} }).body;
  expect(body).toContain('&lt;script>');
  expect(body).toMatch(/disabled/);
});
