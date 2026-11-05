import { detectLocale } from './index';
const messages = {
  en: {
    branches: 'Git Branches', newBranch: 'New Branch…', checkoutRevision: 'Checkout Tag or Revision… (P1)',
    update: 'Update Project…', commit: 'Commit…', push: 'Push…', fetch: 'Fetch',
    recent: 'Recent', local: 'Local', remote: 'Remote', tags: 'Tags',
    checkout: 'Checkout', checkoutCurrent: 'Checkout (current branch)', newFrom: "New Branch from '{name}'…",
    updateBranch: 'Update',
    rename: 'Rename…', delete: 'Delete…', deleteCurrent: 'Delete (current branch)',
    outgoing: '{count} outgoing commit(s)', incoming: '{count} incoming commit(s)',
    updateProject: 'Update Project', pushAction: 'Push',
  },
  'zh-CN': {
    branches: 'Git 分支', newBranch: '新建分支…', checkoutRevision: '检出标签或修订… (P1)',
    update: '更新项目…', commit: '提交…', push: '推送…', fetch: '获取',
    recent: '最近', local: '本地', remote: '远程', tags: '标签',
    checkout: '检出', checkoutCurrent: '检出（当前分支）', newFrom: "从 '{name}' 新建分支…",
    updateBranch: '更新',
    rename: '重命名…', delete: '删除…', deleteCurrent: '删除（当前分支）',
    outgoing: '{count} 个传出提交', incoming: '{count} 个传入提交',
    updateProject: '更新项目', pushAction: '推送',
  },
};
export type GitMessage = keyof typeof messages.en;
export function gitText(key: GitMessage): string { return messages[detectLocale(navigator.language)][key]; }
