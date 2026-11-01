import { expect, it } from 'vitest';
import { render } from 'svelte/server';
import CheckBox from '../src/components/CheckBox.svelte';
import CommitPanel from '../src/features/browse/CommitPanel.svelte';
import { RepositoryBrowser } from '../src/features/browse/browser.svelte';
import FileTreeRow from '../src/features/browse/FileTreeRow.svelte';
import type { FileRow } from '../src/features/browse/PagedFileTree.svelte';
import { fileCount, filePathParts, fileRowLabel, fileTypeIcon } from '../src/features/browse/fileTree';
it('renders checked, mixed and unchecked inclusion without making mixed look fully checked',()=>{
 for(const [state,aria,checked] of [['all','true',true],['some','mixed',false],['none','false',false]] as const){
  const {body}=render(CheckBox,{props:{state,label:'<path>',onchange:()=>{}}});
  expect(body).toContain(`aria-checked="${aria}"`);expect(/\schecked(?:=|\s|>)/.test(body)).toBe(checked);
 }
});
it('orders Changes actions like IDEA, keeps Shelf disabled and exposes the preview and View Options states', () => {
 Object.defineProperty(globalThis.navigator,'language',{configurable:true,value:'en-US'});
 const body=render(CommitPanel,{props:{browser:new RepositoryBrowser(),refresh:()=>{}}}).body;
 const toolbar=body.slice(body.indexOf('role="toolbar"'),body.indexOf('class="files'));
 const labels=[...toolbar.matchAll(/<button[^>]*aria-label="([^"]+)"/g)].map(match=>match[1]);
 expect(labels).toEqual(['Refresh','Rollback…','Shelve Silently','Show Diff Preview','Expand All','Collapse All','View Options']);
 expect(toolbar.match(/<button[^>]*aria-label="Shelve Silently"[^>]*>/)?.[0]).toContain('disabled');
 expect(toolbar.match(/<button[^>]*aria-label="Show Diff Preview"[^>]*>/)?.[0]).toContain('aria-pressed="true"');
 expect(toolbar.match(/<button[^>]*aria-label="View Options"[^>]*>/)?.[0]).toContain('aria-haspopup="dialog"');
 expect(toolbar).not.toContain('Group by Directory');
});
const row: FileRow = { id:'changes/file/test', label:'src/<unsafe>.ts', path:{display:'src/<unsafe>.ts',bytes:[]},
 status:'.M',staged:true,unstaged:true,conflicted:false,note:null,count:0,hasChildren:false,parentIndex:0,level:2 };
it('renders flat filenames, muted directories and type icons with staging information only in tooltips', () => {
 const body=render(FileTreeRow,{props:{file:row,flat:true,changes:true,query:'',ontoggle:()=>{}}}).body;
 expect(body).toContain('&lt;unsafe>.ts'); expect(body).not.toContain('<unsafe>');
 expect(body).toContain('var(--status-modified)'); expect(body).toContain('typeScript.svg');
 expect(body).toMatch(/class="note directory[^>]*>.*src/s);
 expect(body).toContain('Staged · Unstaged');
 expect(body.replace(/title="[^"]*"/g,'')).not.toMatch(/Staged|Unstaged/);
 const root={...row,path:{display:'README.md',bytes:[]},label:'README.md'};
 expect(render(FileTreeRow,{props:{file:root,flat:true,changes:true,query:''}}).body).not.toContain('note directory');
 const conflict=render(FileTreeRow,{props:{file:{...row,conflicted:true},flat:false,changes:true,query:''}}).body;
 expect(conflict).toMatch(/class="conflicted[^>]*>Conflicted/);
 expect(conflict).not.toContain('note directory');
});
it('renders group counts without parentheses and uses the same singular/plural text for paged search', () => {
 Object.defineProperty(globalThis.navigator,'language',{configurable:true,value:'en-US'});
 for(const [id,count,label] of [['changes',1,'Changes 1 file'],['changes',20,'Changes 20 files'],['unversioned',58,'Unversioned Files 58 files']] as const){
  const file={...row,id,count,label:'',path:null};
  expect(fileRowLabel(file,true)).toBe(label);
  const body=render(FileTreeRow,{props:{file,flat:true,changes:true,query:''}}).body;
  expect(body).toContain('<strong'); expect(body).toContain(fileCount(count)); expect(body).not.toContain(`(${count})`);
 }
 expect(fileRowLabel(row,true)).toBe('<unsafe>.ts src');
 expect(filePathParts('src/sub/file.rs')).toEqual({name:'file.rs',directory:'src/sub'});
 expect(filePathParts('README.md')).toEqual({name:'README.md',directory:''});
 expect(fileTypeIcon('Test.java')).toBe('class');
 expect(fileTypeIcon('calendar.vue')).toBe('vue');
 expect(fileTypeIcon('test.TS')).toBe('typeScript');
 expect(fileTypeIcon('Dockerfile')).toBe('docker');
 for(const name of ['README.md','unknown.bin','.env','Makefile'])expect(fileTypeIcon(name)).toBe('text');
 const folder=render(FileTreeRow,{props:{file:{...row,path:null,label:'src',hasChildren:true},flat:false,changes:true,query:''}}).body;
 expect(folder).toContain('folder');
});
it('renders compact directory paths and their file counts as escaped text with inclusion controls', () => {
 Object.defineProperty(globalThis.navigator,'language',{configurable:true,value:'en-US'});
 const directory: FileRow={...row,id:'changes/dir/7372632f',path:null,label:'src/main/java/<pkg>',hasChildren:true,count:3,staged:false,unstaged:false};
 const body=render(FileTreeRow,{props:{file:directory,flat:false,changes:true,query:'',state:'some',ontoggle:()=>{}}}).body;
 expect(body).toContain('src/main/java/&lt;pkg>'); expect(body).not.toContain('<pkg>');
 expect(body).toContain('3 files'); expect(body).toContain('aria-checked="mixed"');
 expect(body).toContain('folder'); expect(body).not.toContain('note directory');
 const history=render(FileTreeRow,{props:{file:{...directory,count:1},flat:false,changes:false,query:''}}).body;
 expect(history).toContain('1 file'); expect(history).not.toContain('role="checkbox"');
});
it('renders a real editable multiline draft, enables commit and push, and disables empty-message commit',()=>{
 Object.defineProperty(globalThis.navigator,'language',{configurable:true,value:'en-US'});
 const browser=new RepositoryBrowser();browser.commit.message='draft\n<script>unsafe</script>';browser.commit.count=1;
 const {body}=render(CommitPanel,{props:{browser,refresh:()=>{}}});expect(body).toContain('draft\n&lt;script>unsafe&lt;/script>');expect(body).toContain('Commit and Push…');expect(body).not.toContain('<script>unsafe</script>');
 browser.commit.message='';const empty=render(CommitPanel,{props:{browser,refresh:()=>{}}}).body;const buttons=[...empty.matchAll(/<button[^>]*>Commit<\/button>/g)].map(([html])=>html).filter(html=>!html.includes('role="tab"'));expect(buttons).toHaveLength(1);expect(buttons[0]).toContain('disabled');
});
