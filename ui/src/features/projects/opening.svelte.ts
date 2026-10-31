import { invoke } from '@tauri-apps/api/core';
import type { RepositoryBrowser } from '../browse/browser.svelte';

export type OpenTarget = 'current' | 'new';
type ProjectAction = { kind: 'choose' | 'current'; path: string } | { kind: 'focused' | 'created' };

/** All user-facing open entries share the same choice and window routing. */
export class ProjectOpening {
  pending = $state<string | null>(null);
  busy = $state(false);
  constructor(private browser: RepositoryBrowser) {}

  async request(path: string, target?: OpenTarget) {
    if (this.busy || this.pending || this.browser.opening || this.browser.commit.busy) return;
    this.busy = true;
    try {
      const action = await invoke<ProjectAction>('open_project', { path, target: target ?? null });
      if (action.kind === 'choose') this.pending = action.path;
      else if (action.kind === 'current') await this.browser.open(action.path);
    } catch (error) {
      this.browser.report(error);
      void this.browser.recentProjects().catch(error => this.browser.report(error));
    }
    finally { this.busy = false; }
  }

  async choose(target: OpenTarget) {
    const path = this.pending;
    this.pending = null;
    if (path) await this.request(path, target);
  }

  cancel() { this.pending = null; }
}
