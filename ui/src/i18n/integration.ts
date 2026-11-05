import { detectLocale } from './index';
const messages = {
  en: {
    merge: 'Merge…', rebase: 'Rebase…', pull: 'Pull…', mergeRun: 'Merge', rebaseRun: 'Rebase', pullRun: 'Pull',
    localBranch: 'New local branch name (optional)', workingTree: 'Working Tree', target: 'Branch or commit', branch: 'Branch', remote: 'Remote', branches: 'Branches', current: 'Current', selected: 'Selected',
    noFf: 'Create a merge commit even if it can be fast-forwarded', ffOnly: 'Merge only if it can be fast-forwarded',
    squash: 'Create a single commit for all merged changes', noCommit: 'Do not commit the merge result', message: 'Specify a message for the merge commit',
    onto: 'Specify a new base (--onto)', newBase: 'New base (branch or hash)', oldBase: 'Old base (branch or hash)',
    interactive: 'Interactive rebase — available in v0.3', protect: 'Tracked local changes are saved and restored with stash. Untracked files are kept; Git refuses overwrites.',
    multiRebase: 'Rebase requires exactly one branch.', cancel: 'Cancel', working: 'Working…', restoreSaved: 'Restore saved local changes', completed: 'Operation completed',
    failed: 'Operation failed', paused: 'Merge is ready to commit', conflicts: 'Operation stopped with conflicts', interrupted: 'Operation interrupted',
    retained: 'Saved changes retained in stash {stash}.', recover: 'Working tree recovery is required',
    setUpstream: 'Set Upstream…', unsetUpstream: 'Unset Upstream', upstream: 'Upstream branch', upstreamRequired: 'No upstream is configured.', set: 'Set Upstream',
    mergeCurrent: 'Merge into Current', rebaseCurrent: 'Rebase Current onto Selected', checkoutRebase: 'Checkout and Rebase onto Current',
    compare: 'Compare with Current', diff: 'Show Diff with Working Tree', files: 'Files', commits: 'Commits', empty: 'No differences',
    comparison: 'Branch Comparison', loading: 'Loading…', browse: 'Choose branch', close: 'Close',
    checkoutHint: 'The selected local branch will be checked out and rebased onto the current branch.',
    noCommitHint: '--no-commit does not stop a fast-forward. Select --no-ff to always stop before committing.',
    squashHint: 'Squash leaves staged changes for a regular commit; Git does not create MERGE_HEAD.',
    invalidTarget: 'Choose an existing branch or commit; names beginning with a dash are not allowed.', incompatibleOptions: 'These options cannot be used together.', messageRequired: 'Enter a commit message.', branchRequired: 'Choose an existing local or remote branch.', currentBranch: 'Choose a branch other than the current branch.', localBranchExists: 'The local branch already exists. Enter a different name.', fetchChanged: 'The fetched branches changed. Fetch again and retry.', operationInProgress: 'Finish or abort the operation in progress first.', stashRecoveryRequired: 'Restore the saved local changes first.', concurrentChange: 'The repository changed externally. Review the refreshed state and retry.', indexLocked: 'Another Git operation holds the index lock.',
    invalid: 'Choose a target and compatible options before continuing.',
  },
  'zh-CN': {
    merge: '合并…', rebase: '变基…', pull: '拉取…', mergeRun: '合并', rebaseRun: '变基', pullRun: '拉取',
    localBranch: '新本地分支名称（可选）', workingTree: '工作树', target: '分支或提交', branch: '分支', remote: '远端', branches: '分支', current: '当前', selected: '所选',
    noFf: '即使可以快进也创建合并提交', ffOnly: '仅允许快进合并', squash: '将全部合并变更合为一次普通提交', noCommit: '不提交合并结果', message: '指定合并提交说明',
    onto: '指定新的基点（--onto）', newBase: '新基点（分支或哈希）', oldBase: '旧基点（分支或哈希）',
    interactive: '交互式变基 — 将在 v0.3 提供', protect: '本地已跟踪修改通过 stash 保存并恢复。未跟踪文件保留，Git 会拒绝覆盖。',
    multiRebase: '变基必须且只能选择一个分支。', cancel: '取消', working: '正在执行…', restoreSaved: '恢复已保存的本地修改', completed: '操作已完成',
    failed: '操作失败', paused: '合并结果已就绪，请提交', conflicts: '操作因冲突停止', interrupted: '操作已中断',
    retained: '本地修改保留在 stash {stash}。', recover: '需要恢复工作区',
    setUpstream: '设置上游…', unsetUpstream: '取消上游', upstream: '上游分支', upstreamRequired: '尚未配置上游分支。', set: '设置上游',
    mergeCurrent: '合并到当前分支', rebaseCurrent: '将当前分支变基到所选分支', checkoutRebase: '检出并变基到当前分支',
    compare: '与当前分支比较', diff: '显示与工作树的差异', files: '文件', commits: '提交', empty: '没有差异',
    comparison: '分支比较', loading: '正在加载…', browse: '选择分支', close: '关闭',
    checkoutHint: '将检出所选本地分支，再变基到当前分支。',
    noCommitHint: '--no-commit 不会停止快进。若需要始终在提交前停止，请同时选择 --no-ff。',
    squashHint: '压缩合并会留下已暂存变更以供普通提交，Git 不会创建 MERGE_HEAD。',
    invalidTarget: '请选择已存在的分支或提交，不允许以短横线开头的名称。', incompatibleOptions: '所选选项不能同时使用。', messageRequired: '请填写提交说明。', branchRequired: '请选择已存在的本地或远程分支。', currentBranch: '请选择当前分支以外的分支。', localBranchExists: '本地分支已存在，请填写其他名称。', fetchChanged: '获取的分支已变化，请重新获取后重试。', operationInProgress: '请先完成或中止正在进行的操作。', stashRecoveryRequired: '请先恢复已保存的本地修改。', concurrentChange: '仓库已被外部修改，请检查刷新后的状态再重试。', indexLocked: '其他 Git 操作正在占用索引锁。',
    invalid: '请先选择目标及兼容的选项。',
  },
};
export type IntegrationMessage = keyof typeof messages.en;
export function integrationText(key: IntegrationMessage, values: Record<string, string> = {}): string {
  return Object.entries(values).reduce((text, [key, value]) => text.replaceAll(`{${key}}`, value), messages[detectLocale(navigator.language)][key]);
}
