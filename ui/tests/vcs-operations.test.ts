import { describe, expect, it, vi } from 'vitest';
import { quickSelectIndex } from '../src/components/popup';
import { selectable } from '../src/components/list';
import { nativeMenuLabels, nativeMenuText } from '../src/i18n/native-menu';
import { vcsOperationActionIds, vcsOperationsItems } from '../src/features/git/vcs-operations';

describe('VCS Operations Popup', () => {
  it('uses the implemented IDEA VCS and Git action subset in source order', () => {
    const items = vcsOperationsItems(Object.fromEntries(vcsOperationActionIds.map(id => [id, true])), () => {}, key => nativeMenuText(key, 'en'));
    expect(items.map(item => item.id)).toEqual([
      'vcs-operations-git-heading',
      'CheckinProject', 'ChangesView.ToggleCommitUi', 'ChangesView.CommitFile', 'ChangesView.Revert',
      'vcs-operations-separator-4',
      'Git.CurrentFile.ShowHistory', 'Git.CurrentFile.Annotate', 'Git.CurrentFile.CompareRevision',
      'vcs-operations-separator-7',
      'Git.Branches', 'Vcs.Push', 'Git.StashChanges', 'Git.UnstashChanges',
      'vcs-operations-separator-11', 'ChangesView.AddUnversioned', 'Git.ResolveConflicts',
    ]);
    expect(items.filter(item => item.shortcut).map(item => item.shortcut)).toEqual(['1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd']);
    expect(items[1].label).toBe(nativeMenuText('commitAction', 'en'));
  });

  it('mirrors the shared CheckinProject enabled state for clean repositories', () => {
    const available = vcsOperationsItems({ CheckinProject: true }, () => {});
    const unavailable = vcsOperationsItems({ CheckinProject: false }, () => {});
    expect(available.find(item => item.id === 'CheckinProject')?.disabled).toBe(false);
    expect(unavailable.find(item => item.id === 'CheckinProject')?.disabled).toBe(true);
  });

  it('keeps unavailable rows visible and prevents quick-select from activating them', () => {
    const run = vi.fn();
    const items = vcsOperationsItems({ 'CheckinProject': true, 'Git.CurrentFile.Annotate': false }, run, key => nativeMenuText(key, 'zh-CN'));
    const annotateIndex = quickSelectIndex(items, '6');
    expect(annotateIndex).toBeGreaterThanOrEqual(0);
    expect(items[annotateIndex].id).toBe('Git.CurrentFile.Annotate');
    expect(items[annotateIndex].disabled).toBe(true);
    expect(selectable(items[annotateIndex])).toBe(false);
    if (selectable(items[annotateIndex])) items[annotateIndex].run?.();
    expect(run).not.toHaveBeenCalled();
    expect(items[annotateIndex].label).toBe(nativeMenuText('annotate', 'zh-CN'));
  });

  it('provides deterministic Chinese labels for native menu and quick-list assertions', () => {
    const labels = nativeMenuLabels('zh-CN');
    expect(labels.commitAction).toBe('提交');
    expect(labels.pull).toBe('拉取…');
    expect(labels.vcsOperations).toBe('VCS 操作弹窗');
    expect(nativeMenuText('currentFile', 'zh-CN')).toBe('当前文件');
  });
});
