import { detectLocale } from './index';
const messages = {
  en: {
    undoBusy: 'A repository write is running', undoLoading: 'Waiting for repository and containing branch information', undoHead: 'Only the last commit in the branch can be undone', undoMerge: 'Merge commits cannot be undone', undoRoot: 'Root commits cannot be undone with soft reset', undoProtected: 'Commit is pushed to a protected remote branch (main/master)', undoState: 'Cannot undo during rebase, merge, cherry-pick or revert', revertMerge: 'Merge commits cannot be reverted', historyStagedChanges: 'Commit or unstage existing staged changes before cherry-picking or reverting', writeDetached: 'Check out a local branch before changing history', writeState: 'Finish the current Git operation first', writeBusy: 'A repository write is running',
    menu: 'Commit', copy: 'Copy Revision Number', patch: 'Create Patch…', checkout: 'Checkout', browse: 'Show Repository at Revision',
    compare: 'Compare Versions', localDiff: 'Show Diff with Local', reset: 'Reset Current Branch to Here…', revert: 'Revert Commit', undo: 'Undo Commit…',
    edit: 'Edit Commit Message…', fixup: 'Fixup…', squashInto: 'Squash Into…', drop: 'Drop Commits', squash: 'Squash Commits…',
    rebase: 'Interactive Rebase from Here…', push: 'Push All up to Here…', branch: 'Branch', newBranch: 'New Branch…', tag: 'New Tag…',
    child: 'Go to Child Commit', parent: 'Go to Parent Commit', revision: "Checkout Revision '{hash}'", cherryPick: 'Cherry-Pick', pushTag: "Push Tag '{name}'", deleteTag: "Delete Tag '{name}'", deleteRemoteTag: "Delete Remote Tag '{name}'",
    resetTitle: 'Reset Current Branch to Here', soft: 'Soft — move HEAD; keep index and working tree', mixed: 'Mixed — move HEAD and reset index; keep working tree', hard: 'Hard — discard index and working tree changes', keep: 'Keep — move HEAD and keep local changes where possible', resetHardWarning: 'Hard reset discards staged and unstaged changes in affected paths.', resetButton: 'Reset', cancel: 'Cancel', newTagTitle: 'New Tag', tagName: 'Tag name', tagMessage: 'Annotation message (optional)', lightweight: 'Lightweight tag', annotated: 'Annotated tag', checkoutMode: 'Local changes', normalCheckout: 'Checkout', smartCheckout: 'Smart Checkout (stash, checkout, restore)', forceCheckout: 'Force Checkout (discard local changes)', deleteRemoteTagConfirm: "Delete remote tag '{name}' from '{remote}'? This removes that tag reference from the remote.",
    preferences: 'Git Preferences', historyPreferences: 'History Operations', addSuffixToCherryPicks: 'Add the source suffix when cherry-picking published commits', emptyCherryPick: 'When a cherry-pick is empty', skipEmptyCherryPick: 'Skip the empty commit', createEmptyCherryPick: 'Create an empty commit', save: 'Save', saveFailed: 'Could not save preferences', chooseChild: 'Select a child commit to navigate to', commitSubject: 'Commit', commitAuthor: 'Author',
  },
  'zh-CN': {
    undoBusy: '仓库写操作正在执行', undoLoading: '正在等待仓库和包含分支信息', undoHead: '只能撤销当前分支的最后一个提交', undoMerge: '无法撤销合并提交', undoRoot: '根提交无法通过 soft reset 撤销', undoProtected: '提交已推送至受保护的远程分支（main/master）', undoState: '变基、合并、拣选或还原进行中，无法撤销提交', writeDetached: '请先检出本地分支，再修改历史', writeState: '请先完成当前 Git 操作', writeBusy: '仓库写操作正在执行', revertMerge: '不能还原合并提交', historyStagedChanges: '请先提交或取消暂存已有内容，再拣选或还原提交',
    menu: '提交', copy: '复制修订号', patch: '创建补丁…', checkout: '检出', browse: '显示该修订的仓库',
    compare: '比较版本', localDiff: '与本地比较差异', reset: '将当前分支重置到此处…', revert: '还原提交', undo: '撤销提交…',
    edit: '编辑提交消息…', fixup: '修补…', squashInto: '压缩到…', drop: '丢弃提交', squash: '压缩提交…',
    rebase: '从此处交互式变基…', push: '推送到此处的所有提交…', branch: '分支', newBranch: '新建分支…', tag: '新建标签…',
    child: '转到子提交', parent: '转到父提交', revision: "检出修订 '{hash}'", cherryPick: '拣选提交', pushTag: "推送标签 '{name}'", deleteTag: "删除标签 '{name}'", deleteRemoteTag: "删除远程标签 '{name}'",
    resetTitle: '将当前分支重置到此处', soft: 'Soft — 移动 HEAD；保留暂存区和工作区', mixed: 'Mixed — 移动 HEAD 并重置暂存区；保留工作区', hard: 'Hard — 丢弃暂存区和工作区改动', keep: 'Keep — 尽量保留本地改动并移动 HEAD', resetHardWarning: 'Hard reset 会丢弃受影响路径中的已暂存和未暂存改动。', resetButton: '重置', cancel: '取消', newTagTitle: '新建标签', tagName: '标签名', tagMessage: '附注内容（可选）', lightweight: '轻量标签', annotated: '附注标签', checkoutMode: '本地修改', normalCheckout: '检出', smartCheckout: '智能检出（暂存、检出并恢复）', forceCheckout: '强制检出（丢弃本地修改）', deleteRemoteTagConfirm: "从远程 '{remote}' 删除标签 '{name}'？这会移除远程标签引用。",
    preferences: 'Git 首选项', historyPreferences: '历史操作', addSuffixToCherryPicks: '拣选已发布提交时附加来源说明', emptyCherryPick: '拣选结果为空时', skipEmptyCherryPick: '跳过空提交', createEmptyCherryPick: '创建空提交', save: '保存', saveFailed: '无法保存首选项', chooseChild: '选择要跳转到的子提交', commitSubject: '提交', commitAuthor: '作者',
  },
};
export type HistoryMenuMessage = keyof typeof messages.en;
export function historyMenuText(key: HistoryMenuMessage): string { return messages[detectLocale(navigator.language)][key]; }
