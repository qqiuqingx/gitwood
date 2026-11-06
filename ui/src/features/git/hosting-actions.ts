import { invoke } from '@tauri-apps/api/core';
import type { RepositoryBrowser } from '../browse/browser.svelte';
import type { HostingTarget } from './hosting-menu';

export function hostingHandlers(getBrowser: () => RepositoryBrowser) {
  const open = (remote: string, target: HostingTarget) => {
    const browser = getBrowser();
    void invoke('open_hosting_link', { request: { generation: browser.generation, remote, target } }).catch(error => browser.report(error));
  };
  const copy = (remote: string, target: HostingTarget) => {
    const browser = getBrowser();
    void invoke<string>('create_hosting_link', { request: { generation: browser.generation, remote, target } })
      .then(url => navigator.clipboard.writeText(url)).catch(error => browser.report(error));
  };
  return { open, copy };
}

export function publishedHostingRemotes(browser: RepositoryBrowser) {
  const head = browser.status?.branch.oid;
  const branch = browser.status?.branch.head;
  if (!head || head === '(initial)' || !branch || branch === '(detached)') return [];
  return browser.hostedRemotes.filter(remote => (browser.refs?.remotes[remote.name] ?? []).some(ref =>
    ref.name === `refs/remotes/${remote.name}/${branch}` && ref.hash === head));
}

export function hostingRequestRemotes(browser: RepositoryBrowser) {
  const upstream = browser.status?.branch.upstream;
  const base = upstream?.replace(/^refs\/remotes\/[^/]+\//, '');
  if (!base || base === upstream) return [];
  return publishedHostingRemotes(browser).filter(remote =>
    (browser.refs?.remotes[remote.name] ?? []).some(ref => ref.name === `refs/remotes/${remote.name}/${base}`));
}
