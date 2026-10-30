import { expect, it } from 'vitest';
import { render } from 'svelte/server';
import CheckBox from '../src/components/CheckBox.svelte';
import CommitPanel from '../src/features/browse/CommitPanel.svelte';
import { RepositoryBrowser } from '../src/features/browse/browser.svelte';
it('renders checked, mixed and unchecked inclusion without making mixed look fully checked',()=>{
 for(const [state,aria,checked] of [['all','true',true],['some','mixed',false],['none','false',false]] as const){
  const {body}=render(CheckBox,{props:{state,label:'<path>',onchange:()=>{}}});
  expect(body).toContain(`aria-checked="${aria}"`);expect(/\schecked(?:=|\s|>)/.test(body)).toBe(checked);
 }
});
it('renders a real editable multiline draft, enables commit and push, and disables empty-message commit',()=>{
 Object.defineProperty(globalThis.navigator,'language',{configurable:true,value:'en-US'});
 const browser=new RepositoryBrowser();browser.commit.message='draft\n<script>unsafe</script>';browser.commit.count=1;
 const {body}=render(CommitPanel,{props:{browser,refresh:()=>{}}});expect(body).toContain('draft\n&lt;script>unsafe&lt;/script>');expect(body).toContain('Commit and Push…');expect(body).not.toContain('<script>unsafe</script>');
 browser.commit.message='';const empty=render(CommitPanel,{props:{browser,refresh:()=>{}}}).body;const buttons=[...empty.matchAll(/<button[^>]*>Commit<\/button>/g)].map(([html])=>html).filter(html=>!html.includes('role="tab"'));expect(buttons).toHaveLength(1);expect(buttons[0]).toContain('disabled');
});
