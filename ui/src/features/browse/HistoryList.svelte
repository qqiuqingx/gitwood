<script lang="ts">
  import List from '../../components/List.svelte';
  import Highlight from '../../components/Highlight.svelte';
  import GraphCanvas from './GraphCanvas.svelte';
  import Filters from './Filters.svelte';
  import { formatDate } from './model';
  import { browseText as t, prettyDateWords } from '../../i18n/browse';
  import type { RepositoryBrowser } from './browser.svelte';
  let { browser }: { browser: RepositoryBrowser } = $props();
  let scrollLeft = $state(0);
  const rows = $derived([...browser.rows.values()]);
  const graphWidth = $derived(Math.max(58, ...rows.map(row => Math.max(row.column, ...row.segments.flatMap(segment => [segment[1], segment[2]])) * 16 + 32)));
  const selected = $derived(new Set(browser.selected ? [browser.selected] : []));
  const refsByHash = $derived.by(() => {
    const map = new Map<string, { name: string; kind: string }[]>();
    if (!browser.refs) return map;
    const { local, remotes, tags, head } = browser.refs;
    for (const [refs, kind] of [[local, 'branch-local'], [Object.values(remotes).flat(), 'branch-remote'], [tags, 'tag']] as const) {
      for (const ref of refs) { const hash = ref.peeled ?? ref.hash; const labels = map.get(hash) ?? []; labels.push({ name: ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''), kind }); map.set(hash, labels); }
    }
    if (head.oid) { const labels = map.get(head.oid) ?? []; labels.unshift({ name: 'HEAD', kind: 'head' }); map.set(head.oid, labels); }
    return map;
  });
</script>
<section aria-label={t('log')}>
  <Filters value={browser.filter} refs={browser.refs} apply={value => { void browser.applyFilter(value); }} refresh={() => { void browser.refresh({ status: false, refs: false, history: true }); }} />
  <div class="column-clip"><div class="columns" style:transform={`translateX(${-scrollLeft}px)`} style:min-width={`${graphWidth + 430}px`} style:grid-template-columns={`${graphWidth}px minmax(0,1fr) 100px 130px`}><span></span><span>{t('subject')}</span><span>{t('author')}</span><span>{t('date')}</span></div></div>
  <div class="list" aria-busy={browser.loading}>
    <List label={t('log')} rowCount={browser.total} minRowWidth={graphWidth + 430} search={false} selectedIds={selected} activeIndex={browser.activeIndex}
      bind:scrollTop={browser.scrollTop} getRow={index => ({ id: browser.rows.get(index)?.commit.hash ?? `row:${index}`, label: browser.rows.get(index)?.commit.subject ?? t('loading') })}
      onviewportchange={range => { scrollLeft = range.scrollLeft; void browser.viewport(range); }} onselectionchange={(_, index) => { void browser.selectIndex(index); }}>
      {#snippet row(_, index, query)}
        {@const commit = browser.rows.get(index)?.commit}
        <div class="commit-row" style:grid-template-columns={`${graphWidth}px minmax(0,1fr) 100px 130px`}>
          <span></span><span class="subject">
            {#if commit}{#each refsByHash.get(commit.hash) ?? [] as ref}<span class="ref" style:color={`var(--label-${ref.kind})`}>{ref.name}</span>{/each}{/if}
            <Highlight text={commit?.subject ?? t('loading')} {query} />
          </span>
          <span class="secondary" title={commit?.email}>{commit?.author ?? ''}</span>
          <span class="secondary" title={commit ? new Date(commit.author_time * 1000).toLocaleString() : ''}>{commit ? formatDate(commit.author_time, navigator.language, new Date(), prettyDateWords()) : ''}</span>
        </div>
      {/snippet}
      {#snippet overlay(viewport)}<GraphCanvas {rows} {...viewport} width={Math.max(0, Math.min(graphWidth - viewport.scrollLeft, viewport.width))} head={browser.refs?.head.oid ?? null} />{/snippet}
    </List>
    {#if !browser.total}<div class="empty">{browser.loading ? t('loading') : t('noCommits')}</div>{/if}
  </div>
</section>
<style>
  section { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .list { position: relative; flex: 1; min-height: 0; }
  .column-clip { overflow: hidden; flex: none; }
  .columns, .commit-row { display: grid; align-items: center; gap: var(--space-inline); width: 100%; min-width: 0; }
  .columns { color: var(--text-secondary); border-bottom: var(--border-width) solid var(--separator); font-size: var(--font-small-size); padding-inline: var(--space-inline); }
  .commit-row { margin-inline-start: calc(-1 * var(--space-inline)); }
  .subject { display: flex; align-items: center; gap: calc(var(--space-inline) / 2); min-width: 0; overflow: hidden; }
  .ref { flex: none; border: var(--border-width) solid currentColor; border-radius: calc(var(--space-inline) / 3); font-size: var(--font-small-size); padding-inline: calc(var(--space-inline) / 2); }
  .secondary { color: var(--text-secondary); font-size: var(--font-small-size); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { position: absolute; inset: 25% 0 auto; text-align: center; color: var(--text-secondary); pointer-events: none; }
</style>
