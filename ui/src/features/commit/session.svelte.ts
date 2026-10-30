import { invoke } from '@tauri-apps/api/core';
import { commitText as t } from '../../i18n/commit';
import { emptySelection, type Selection, type CommitResult, feedback, remember } from './model';
export class CommitSession {
  selection = $state<Selection>(emptySelection());
  message = $state(''); draft = $state(''); amend = $state(false);
  history = $state<string[]>([]);
  count = $state(0); busy = $state(false); task = $state('');
  result = $state<CommitResult | null>(null);
  restoreUndoMessage(message: string) {
    this.history = remember(remember(this.history, this.draft), this.message);
    this.draft = this.message; this.message = message; this.amend = false; this.result = null;
    this.selection = emptySelection();
  }
  async submit(generation: number, expected: string) {
    this.busy=true;this.task=t('running');this.result=null;
    try {
      const result=await invoke<CommitResult>('commit',{generation,expected,selection:$state.snapshot(this.selection),message:this.message,amend:this.amend});
      this.finish(result);
    } finally {this.busy=false;}
  }
  private finish(result: CommitResult) {
    const next = feedback(result, this.message);
    this.history = remember(this.history, this.message);
    this.message = next.message; this.draft = ''; this.amend = false; this.result = result;
    this.selection = { ...this.selection, hunks: [] };
  }
}
