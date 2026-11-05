import { detectLocale } from './index';

const en = {
  commitAction: 'Commit', push: 'Push', update: 'Update Project', pull: 'Pull…', fetch: 'Fetch',
  merge: 'Merge…', commitMerge: 'Commit Merge', abortMerge: 'Abort Merge', rebase: 'Rebase…',
  continueRebase: 'Continue Rebase', abortRebase: 'Abort Rebase', skipCommit: 'Skip Commit',
  branches: 'Branches…', newBranch: 'New Branch…', newTag: 'New Tag…', resetHead: 'Reset HEAD…',
  showGitLog: 'Show Git Log', uncommittedChanges: 'Uncommitted Changes', stash: 'Stash Changes…',
  unstash: 'Unstash Changes…', rollback: 'Rollback…', currentFile: 'Current File',
  commitFile: 'Commit File…', addToVcs: 'Add to VCS', showHistory: 'Show History',
  annotate: 'Annotate', compareRevision: 'Compare with Revision…', compareBranch: 'Compare with Branch…',
  vcsOperations: 'VCS Operations Popup', vcsOperationsTitle: 'VCS Operations', gitGroup: 'Git',
  resolveConflicts: 'Resolve Conflicts', toggleCommitUi: 'Toggle Commit UI',
} as const;

type Messages = Record<keyof typeof en, string>;
const zh: Messages = {
  commitAction: '提交', push: '推送', update: '更新项目', pull: '拉取…', fetch: '获取',
  merge: '合并…', commitMerge: '提交合并', abortMerge: '中止合并', rebase: '变基…',
  continueRebase: '继续变基', abortRebase: '中止变基', skipCommit: '跳过提交',
  branches: '分支…', newBranch: '新建分支…', newTag: '新建标签…', resetHead: '重置 HEAD…',
  showGitLog: '显示 Git 日志', uncommittedChanges: '未提交的更改', stash: '暂存工作区变更…',
  unstash: '恢复暂存变更…', rollback: '回滚…', currentFile: '当前文件',
  commitFile: '提交文件…', addToVcs: '添加到版本控制', showHistory: '显示历史记录',
  annotate: '注释', compareRevision: '与修订比较…', compareBranch: '与分支比较…',
  vcsOperations: 'VCS 操作弹窗', vcsOperationsTitle: 'VCS 操作', gitGroup: 'Git',
  resolveConflicts: '解决冲突', toggleCommitUi: '切换提交窗口',
};

export type NativeMenuMessage = keyof typeof en;
export type NativeMenuLabels = Record<
  'commitAction' | 'push' | 'update' | 'pull' | 'fetch' | 'merge' | 'commitMerge' | 'abortMerge'
  | 'rebase' | 'continueRebase' | 'abortRebase' | 'skipCommit' | 'branches' | 'newBranch' | 'newTag'
  | 'resetHead' | 'showGitLog' | 'uncommittedChanges' | 'stash' | 'unstash' | 'rollback' | 'currentFile'
  | 'commitFile' | 'addToVcs' | 'showHistory' | 'annotate' | 'compareRevision' | 'compareBranch' | 'vcsOperations', string>;

export function nativeMenuText(key: NativeMenuMessage, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return (detectLocale(language) === 'zh-CN' ? zh : en)[key];
}

export function nativeMenuLabels(language = typeof navigator === 'undefined' ? 'en' : navigator.language): NativeMenuLabels {
  const labels = detectLocale(language) === 'zh-CN' ? zh : en;
  return {
    commitAction: labels.commitAction, push: labels.push, update: labels.update, pull: labels.pull,
    fetch: labels.fetch, merge: labels.merge, commitMerge: labels.commitMerge, abortMerge: labels.abortMerge,
    rebase: labels.rebase, continueRebase: labels.continueRebase, abortRebase: labels.abortRebase,
    skipCommit: labels.skipCommit, branches: labels.branches, newBranch: labels.newBranch, newTag: labels.newTag,
    resetHead: labels.resetHead, showGitLog: labels.showGitLog, uncommittedChanges: labels.uncommittedChanges,
    stash: labels.stash, unstash: labels.unstash, rollback: labels.rollback, currentFile: labels.currentFile,
    commitFile: labels.commitFile, addToVcs: labels.addToVcs, showHistory: labels.showHistory,
    annotate: labels.annotate, compareRevision: labels.compareRevision, compareBranch: labels.compareBranch,
    vcsOperations: labels.vcsOperations,
  };
}
