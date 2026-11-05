import { detectLocale } from './index';
const en = {
  stash:'Stash Changes…', unstash:'Unstash Changes…', message:'Message:', keepIndex:'Keep index', includeUntracked:'Include untracked files', create:'Create Stash',
  list:'Stashes', viewDiff:'Show Diff', apply:'Apply', pop:'Pop', drop:'Drop…', clear:'Clear All…', branch:'New Branch…', branchName:'Branch name',
  dropQuestion:'Drop this stash permanently? The saved changes in {selector} will be removed.', clearQuestion:'Clear all {count} stashes permanently? Every saved stash will be removed.',
  cancel:'Cancel', confirmDrop:'Drop Stash', confirmClear:'Clear Stashes', reinstate:'Reinstate index', empty:'No stashes', loading:'Loading…',
  noChanges:'There are no local changes to stash.', failed:'Stash operation failed', stale:'The stash list changed. Refresh and select the stash again.',
  conflict:'Stash applied with conflicts. Resolve them using the conflict workflow.', branchCreate:'Create Branch', diffEmpty:'This stash has no file diff.',
};
const zh: typeof en = {
  stash:'暂存工作区变更…', unstash:'恢复暂存变更…', message:'说明：', keepIndex:'保留暂存区', includeUntracked:'包含未跟踪文件', create:'创建 Stash',
  list:'Stash 列表', viewDiff:'查看差异', apply:'应用', pop:'应用并移除', drop:'删除…', clear:'清空全部…', branch:'从 Stash 新建分支…', branchName:'分支名称',
  dropQuestion:'永久删除此 Stash？{selector} 中保存的变更将被移除。', clearQuestion:'永久清空全部 {count} 个 Stash？所有已保存的变更都会被移除。',
  cancel:'取消', confirmDrop:'删除 Stash', confirmClear:'清空 Stash', reinstate:'恢复暂存区状态', empty:'没有 Stash', loading:'正在加载…',
  noChanges:'没有可暂存的本地变更。', failed:'Stash 操作失败', stale:'Stash 列表已变化，请刷新后重新选择。',
  conflict:'应用 Stash 时产生冲突，请通过冲突解决流程处理。', branchCreate:'创建分支', diffEmpty:'此 Stash 没有文件差异。',
};
export type StashMessage = keyof typeof en;
export function stashText(key: StashMessage, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return (detectLocale(language) === 'zh-CN' ? zh : en)[key];
}
