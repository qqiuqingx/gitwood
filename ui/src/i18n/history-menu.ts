import { detectLocale } from './index';
const messages = {
  en: {
    undoBusy: 'A repository write is running', undoLoading: 'Waiting for repository and containing branch information', undoHead: 'Only the last commit in the branch can be undone', undoMerge: 'Merge commits cannot be undone', undoRoot: 'Root commits cannot be undone with soft reset', undoProtected: 'Commit is pushed to a protected remote branch (main/master)', undoState: 'Cannot undo during rebase, merge, cherry-pick or revert',
    menu: 'Commit', copy: 'Copy Revision Number', patch: 'Create Patch…', checkout: 'Checkout', browse: 'Show Repository at Revision',
    compare: 'Compare Versions', localDiff: 'Show Diff with Local', reset: 'Reset Current Branch to Here…', revert: 'Revert Commit', undo: 'Undo Commit…',
    edit: 'Edit Commit Message…', fixup: 'Fixup…', squashInto: 'Squash Into…', drop: 'Drop Commits', squash: 'Squash Commits…',
    rebase: 'Interactive Rebase from Here…', push: 'Push All up to Here…', branch: 'Branch', newBranch: 'New Branch…', tag: 'New Tag…',
    child: 'Go to Child Commit', parent: 'Go to Parent Commit', revision: "Checkout Revision '{hash}'",
  },
  'zh-CN': {
    undoBusy: '仓库写操作正在执行', undoLoading: '正在等待仓库和包含分支信息', undoHead: '只能撤销当前分支的最后一个提交', undoMerge: '无法撤销合并提交', undoRoot: '根提交无法通过 soft reset 撤销', undoProtected: '提交已推送至受保护的远程分支（main/master）', undoState: '变基、合并、拣选或还原进行中，无法撤销提交',
    menu: '提交', copy: '复制修订号', patch: '创建补丁…', checkout: '检出', browse: '显示该修订的仓库',
    compare: '比较版本', localDiff: '与本地比较差异', reset: '将当前分支重置到此处…', revert: '还原提交', undo: '撤销提交…',
    edit: '编辑提交消息…', fixup: '修补…', squashInto: '压缩到…', drop: '丢弃提交', squash: '压缩提交…',
    rebase: '从此处交互式变基…', push: '推送到此处的所有提交…', branch: '分支', newBranch: '新建分支…', tag: '新建标签…',
    child: '转到子提交', parent: '转到父提交', revision: "检出修订 '{hash}'",
  },
};
export type HistoryMenuMessage = keyof typeof messages.en;
export function historyMenuText(key: HistoryMenuMessage): string { return messages[detectLocale(navigator.language)][key]; }
