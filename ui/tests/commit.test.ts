import { expect, it, vi } from 'vitest';
const ipc=vi.hoisted(()=>vi.fn());
vi.mock('@tauri-apps/api/core',()=>({invoke:ipc}));
import { amendMessage, blockSelection, canCommit, emptySelection, feedback, included, remember, toggleBlock, toggleRule } from '../src/features/commit/model';
import { CommitSession } from '../src/features/commit/session.svelte';
import { RepositoryBrowser } from '../src/features/browse/browser.svelte';
const path = { bytes:[100,105,114,47,102], display:'dir/f' };
it('tracked defaults included, untracked defaults excluded, directory/file rules survive refresh by bytes',()=>{
 let selection=emptySelection(); expect(included(selection,path)).toBe(true);expect(included(selection,path,true)).toBe(false);
 selection=toggleRule(selection,'changes/dir/6469722f','all');expect(included(selection,{...path,display:'new display'})).toBe(false);
 selection=toggleRule(selection,'change/6469722f66','none');expect(included(selection,path)).toBe(true);
});
it('partial blocks create mixed state, full toggles remove hunk overrides, stale previews never retarget indices',()=>{
 let selection=toggleBlock(emptySelection(),path,'v1',3,1);expect(blockSelection(selection,path,'v1',3)).toEqual([0,2]);expect(selection.hunks.length).toBe(1);
 expect(blockSelection(selection,path,'v2',3)).toEqual([]);
 selection=toggleRule(selection,'changes','some');expect(selection.hunks).toHaveLength(0);expect(included(selection,path)).toBe(true);
 selection=toggleBlock(selection,path,'v1',3,0);selection=toggleBlock(selection,path,'v1',3,0);expect(selection.hunks).toHaveLength(0);
});
it('amend loads the previous message and cancellation restores the original multiline draft',()=>{
 const enabled=amendMessage('draft\nbody','',true,'last\nbody');expect(enabled).toEqual({message:'last\nbody',draft:'draft\nbody'});
 expect(amendMessage('edited amend',enabled.draft,false,'').message).toBe('draft\nbody');
});
it('button requires message and selection or amend and refuses while running',()=>{
 expect(canCommit(' \n',1,false,false)).toBe(false);expect(canCommit('message',0,false,false)).toBe(false);expect(canCommit('message',0,true,false)).toBe(true);expect(canCommit('message',1,false,true)).toBe(false);
});
it('history is multiline, newest first, deduplicated and bounded',()=>{
 expect(remember(['old','message\nbody'],'message\nbody')).toEqual(['message\nbody','old']);expect(remember(Array.from({length:30},(_,i)=>String(i)),'new')).toHaveLength(25);expect(remember(['old'],' ')).toEqual(['old']);
});
it('three result states distinguish ordinary success, recovery and failure without losing failure draft or selection',async()=>{
 const state=new CommitSession();state.message='draft';state.selection=toggleBlock(emptySelection(),path,'v1',3,1);const chosen=state.selection;
 const failure=feedback(null,state.message);expect(failure).toEqual({message:'draft',success:false,recovery:null});expect(state.selection).toBe(chosen);
 const recovery={kind:'recovery' as const,hash:'abc',directory:'/recovery',instructions:'Do not retry',reason:'index conflict'};
 expect(feedback(recovery,state.message)).toEqual({message:'',success:false,recovery});ipc.mockResolvedValueOnce(recovery);await state.submit(1,'v1');expect(state.result?.kind).toBe('recovery');expect(state.message).toBe('');expect(state.history).toEqual(['draft']);
 expect(feedback({kind:'committed',hash:'abc'},'next')).toEqual({message:'',success:true,recovery:null});
});
it('refresh does not replace inclusion rules or commit message, writes defer read invalidation',async()=>{
 const browser=new RepositoryBrowser();browser.project={root:'/repo',linked:false};browser.generation=2;browser.commit.message='draft';browser.commit.selection=toggleRule(emptySelection(),'changes','all');const selection=browser.commit.selection;browser.commit.busy=true;
 await browser.refresh({status:true,refs:true,history:true},2);expect(browser.generation).toBe(2);expect(browser.commit.selection).toBe(selection);expect(browser.commit.message).toBe('draft');
});

it('hook/signing/lock IPC failures preserve the actual controller message and hunk inclusion and never show success',async()=>{
 for(const detail of ['hook rejected','signing failed','index locked']){
  const state=new CommitSession();state.message='draft\nbody';state.selection=toggleBlock(emptySelection(),path,'v1',3,1);const selected=JSON.stringify(state.selection);ipc.mockRejectedValueOnce({detail});
  await expect(state.submit(1,'v1')).rejects.toEqual({detail});expect(state.busy).toBe(false);expect(state.result).toBeNull();expect(state.message).toBe('draft\nbody');expect(JSON.stringify(state.selection)).toBe(selected);
 }
});
it('successful IPC clears message and amend while retaining uncommitted inclusion rules',async()=>{
 const state=new CommitSession();state.message='success';state.amend=true;state.draft='draft';state.selection=toggleRule(emptySelection(),'changes','all');const rules=JSON.stringify(state.selection.rules);
 ipc.mockResolvedValueOnce({kind:'committed',hash:'abc'});await state.submit(3,'v3');expect(state.message).toBe('');expect(state.amend).toBe(false);expect(state.result).toEqual({kind:'committed',hash:'abc'});expect(JSON.stringify(state.selection.rules)).toBe(rules);
});
it('the sole untracked-file block follows whole-file inclusion and starts unchecked',()=>{
 const initial=emptySelection();expect(blockSelection(initial,path,'v1',1,true)).toEqual([]);
 const selected=toggleRule(initial,'untracked/6469722f66','none');expect(blockSelection(selected,path,'v1',1,true)).toEqual([0]);expect(selected.hunks).toEqual([]);
});
