const messages = {
  en: {
    commit: 'Commit',
    diff: 'Diff',
    diffLeft: 'Left version',
    diffRight: 'Right version',
    log: 'Git Log',
    branches: 'Branches',
    commits: 'Commits',
    files: 'Changed Files',
    details: 'Commit Details',
    workspaceLog: 'Workspace / Git Log',
    commitDiff: 'Commit / Diff',
    branchesCommits: 'Branches / Commits',
    commitsInformation: 'Commits / Information',
    filesDetails: 'Changed Files / Commit Details',
    diffVersions: 'Left / Right Diff',
  },
  'zh-CN': {
    commit: '提交',
    diff: '差异',
    diffLeft: '左侧版本',
    diffRight: '右侧版本',
    log: 'Git 日志',
    branches: '分支',
    commits: '提交记录',
    files: '变更文件',
    details: '提交详情',
    workspaceLog: '工作区 / Git 日志',
    commitDiff: '提交 / 差异',
    branchesCommits: '分支 / 提交记录',
    commitsInformation: '提交记录 / 信息区',
    filesDetails: '变更文件 / 提交详情',
    diffVersions: '差异左侧 / 右侧',
  },
} as const;

export type Locale = keyof typeof messages;
type Message = keyof typeof messages.en;

export function detectLocale(language: string): Locale {
  return language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';
}

export function translate(locale: Locale, key: Message): string {
  return messages[locale][key];
}
