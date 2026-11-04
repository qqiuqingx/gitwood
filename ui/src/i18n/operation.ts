import { detectLocale } from './index';
// Texts follow IDEA GitBundle: git.status.bar.widget.text.*, action.Git.*.text, abort.operation.*.
const messages = {
  en: {
    merge: 'Merging {branch}', rebase: 'Rebasing {branch}', applyMailbox: 'Applying patches to {branch}',
    cherryPick: 'Cherry-picking in {branch}', revert: 'Reverting in {branch}', bisect: 'Bisecting {branch}',
    step: '{step}/{total}', conflicts: '{count} conflicted file(s)',
    mergeName: 'Merge', rebaseName: 'Rebase', applyMailboxName: 'Apply patches', cherryPickName: 'Cherry-Pick', revertName: 'Revert', bisectName: 'Bisect',
    continueRebase: 'Continue Rebase', abortRebase: 'Abort Rebase', skipCommit: 'Skip Commit',
    commitMerge: 'Commit Merge', abortMerge: 'Abort Merge',
    continueCherryPick: 'Continue Cherry-Pick', abortCherryPick: 'Abort Cherry-Pick', abortRevert: 'Abort Revert',
    continueApply: 'Continue Applying Patches', abortApply: 'Abort Applying Patches', skipPatch: 'Skip Patch',
    abortTitle: 'Abort {name}', abortMessage: 'Abort {name} in {project}?', abort: 'Abort', cancel: 'Cancel',
    completed: '{name} completed', aborted: '{name} abort succeeded', skipped: 'Commit skipped',
    stoppedConflicts: '{name} stopped with conflicts', stopped: '{name} stopped', failed: '{name} failed',
    unresolved: 'Resolve all conflicts and stage the files before continuing.',
    none: 'No merge, rebase, cherry-pick or revert is in progress.',
    unsupported: 'This action is not available for the current operation.',
    stillInProgress: 'Git finished but the operation is still in progress.',
  },
  'zh-CN': {
    merge: '正在合并 {branch}', rebase: '正在变基 {branch}', applyMailbox: '正在向 {branch} 应用补丁',
    cherryPick: '正在 {branch} 中拣选', revert: '正在 {branch} 中还原', bisect: '正在二分查找 {branch}',
    step: '{step}/{total}', conflicts: '{count} 个冲突文件',
    mergeName: '合并', rebaseName: '变基', applyMailboxName: '应用补丁', cherryPickName: '拣选', revertName: '还原', bisectName: '二分查找',
    continueRebase: '继续变基', abortRebase: '中止变基', skipCommit: '跳过提交',
    commitMerge: '提交合并', abortMerge: '中止合并',
    continueCherryPick: '继续拣选', abortCherryPick: '中止拣选', abortRevert: '中止还原',
    continueApply: '继续应用补丁', abortApply: '中止应用补丁', skipPatch: '跳过补丁',
    abortTitle: '中止{name}', abortMessage: '中止 {project} 中的{name}？', abort: '中止', cancel: '取消',
    completed: '{name}已完成', aborted: '{name}已中止', skipped: '已跳过提交',
    stoppedConflicts: '{name}因冲突停止', stopped: '{name}已停止', failed: '{name}失败',
    unresolved: '请先解决所有冲突并暂存文件，再继续。',
    none: '没有正在进行的合并、变基、拣选或还原。',
    unsupported: '当前操作不支持此动作。',
    stillInProgress: 'Git 已结束，但操作仍在进行中。',
  },
};
export type OperationMessage = keyof typeof messages.en;
export function operationText(key: OperationMessage, values: Record<string, string | number> = {}): string {
  return Object.entries(values).reduce((text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    messages[detectLocale(navigator.language)][key]);
}
