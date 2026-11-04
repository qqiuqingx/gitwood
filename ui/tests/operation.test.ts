import { describe, expect, it, vi, afterEach, beforeAll, afterAll } from 'vitest';
import { abortConfirmation, operationActions, operationLabel, sequenceErrorText, sequenceFeedback } from '../src/features/git/operation';
import { addNotice, AUTO_HIDE_MS, MAX_NOTICES, removeNotice } from '../src/features/notifications/model';
import { NotificationCenter } from '../src/features/notifications/center.svelte';
import { historyEditReason } from '../src/features/browse/history-edit';
import { forSelection } from '../src/features/browse/history-menu';
import { rowRanges } from '../src/features/browse/browser.svelte';
import type { Commit, Operation, References } from '../src/features/browse/types';

const rebase: Operation = { kind: 'rebase', branch: 'topic', step: 2, total: 5 };

describe('in-progress operation', () => {
  beforeAll(() => { vi.stubGlobal('navigator', { language: 'en-US' }); });
  afterAll(() => { vi.unstubAllGlobals(); });
  it('labels the branch widget like IDEA GitBranchUtil', () => {
    expect(operationLabel(rebase, 'HEAD')).toBe('Rebasing topic 2/5');
    expect(operationLabel({ kind: 'merge', branch: null, step: null, total: null }, 'main')).toBe('Merging main');
    expect(operationLabel({ kind: 'cherryPick', branch: null, step: null, total: null }, 'main')).toBe('Cherry-picking in main');
    expect(operationLabel({ kind: 'revert', branch: null, step: null, total: null }, null)).toBe('Reverting in HEAD');
  });
  it('offers the IDEA ongoing actions per operation', () => {
    expect(operationActions(rebase).map(item => [item.action, item.label, item.tone])).toEqual([
      ['continue', 'Continue Rebase', 'resolve'], ['abort', 'Abort Rebase', 'abort'], ['skip', 'Skip Commit', 'plain']]);
    expect(operationActions({ ...rebase, kind: 'merge' }).map(item => item.label)).toEqual(['Commit Merge', 'Abort Merge']);
    expect(operationActions({ ...rebase, kind: 'cherryPick' }).map(item => item.label)).toEqual(['Continue Cherry-Pick', 'Abort Cherry-Pick']);
    expect(operationActions({ ...rebase, kind: 'revert' }).map(item => item.label)).toEqual(['Abort Revert']);
    expect(operationActions({ ...rebase, kind: 'bisect' })).toEqual([]);
    expect(operationActions(null)).toEqual([]);
  });
  it('confirms abort and reports outcomes without fake success', () => {
    expect(abortConfirmation('cherryPick', 'demo')).toEqual({ title: 'Abort Cherry-Pick', message: 'Abort cherry-pick in demo?' });
    expect(sequenceFeedback('merge', 'abort', { kind: 'completed' })).toMatchObject({ kind: 'success', title: 'Merge abort succeeded' });
    const conflicts = sequenceFeedback('rebase', 'continue', { kind: 'conflicts', operation: rebase, files: [{ bytes: [65], display: 'A' }] });
    expect(conflicts).toMatchObject({ kind: 'warning', title: 'Rebase stopped with conflicts', files: ['A'], offerActions: true });
    expect(sequenceFeedback('cherryPick', 'continue', { kind: 'stopped', operation: { ...rebase, kind: 'cherryPick' }, detail: 'now empty' }))
      .toMatchObject({ kind: 'warning', detail: 'now empty' });
    expect(sequenceErrorText('rebase', { key: 'sequence.unresolved', detail: 'x' }).detail).toContain('Resolve all conflicts');
    expect(sequenceErrorText('rebase', { key: 'sequence.failed', detail: 'hook said no' }).detail).toBe('hook said no');
  });
});

describe('notifications', () => {
  afterEach(() => { vi.useRealTimers(); });
  it('replaces a group, keeps errors sticky and bounds the list', () => {
    let list = addNotice([], { kind: 'success', title: 'one', group: 'git' }, 1);
    list = addNotice(list, { kind: 'error', title: 'two', group: 'git' }, 2);
    expect(list.map(notice => [notice.title, notice.sticky])).toEqual([['two', true]]);
    for (let id = 3; id < 10; id++) list = addNotice(list, { kind: 'info', title: String(id) }, id);
    expect(list).toHaveLength(MAX_NOTICES);
    expect(removeNotice(list, 9).map(notice => notice.id)).not.toContain(9);
  });
  it('hides transient notices after a timeout and keeps warnings', () => {
    vi.useFakeTimers();
    const center = new NotificationCenter();
    center.notify({ kind: 'success', title: 'done' });
    center.notify({ kind: 'warning', title: 'conflicts' });
    vi.advanceTimersByTime(AUTO_HIDE_MS);
    expect(center.items.map(notice => notice.title)).toEqual(['conflicts']);
    center.clear();
    expect(center.items).toEqual([]);
  });
});

describe('log selection and edit checks', () => {
  const commit: Commit = { hash: 'b', parents: ['a'], author: '', email: '', author_time: 0, commit_time: 0, subject: '', message: '', refs: '' };
  const refs = { local: [], remotes: {}, tags: [], remote_heads: {}, head: { oid: 'b', head: 'main', upstream: null, ahead: 0, behind: 0 } } as References;
  const info = { branches: [], total: 0, protected: false, operation: null };
  it('turns sorted unloaded rows into half-open ranges', () => {
    expect(rowRanges([0, 1, 2, 5, 6, 9])).toEqual([[0, 3], [5, 7], [9, 10]]);
    expect(rowRanges([])).toEqual([]);
  });
  it('applies only the requested history-edit requirements', () => {
    expect(historyEditReason(commit, refs, info, { head: true })).toBeNull();
    expect(historyEditReason({ ...commit, hash: 'c' }, refs, info, { head: true })).toBe('head');
    expect(historyEditReason({ ...commit, hash: 'c' }, refs, info, { nonMerge: true })).toBeNull();
    expect(historyEditReason({ ...commit, parents: ['a', 'x'] }, refs, info, { nonMerge: true })).toBe('merge');
    expect(historyEditReason(commit, refs, { ...info, operation: rebase }, {})).toBe('state');
    expect(historyEditReason(commit, refs, { ...info, operation: { ...rebase, kind: 'bisect' } }, {})).toBeNull();
    expect(historyEditReason(commit, refs, { ...info, branches: ['refs/remotes/origin/master'] }, { unprotected: true })).toBe('protected');
  });
  it('keeps only multi-commit actions enabled for several selected commits', () => {
    const items = [{ id: 'copy', label: 'Copy', run: () => {} }, { id: 'sep', label: '', kind: 'separator' as const }, { id: 'undo', label: 'Undo', run: () => {} }];
    expect(forSelection(items, 1)).toBe(items);
    expect(forSelection(items, 2).map(item => [item.id, !!item.disabled])).toEqual([['copy', false], ['sep', false], ['undo', true]]);
  });
});
