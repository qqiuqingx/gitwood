const messages = {
  en: {
    file: 'File', open: 'Open…', recentProjects: 'Recent Projects', closeProject: 'Close Project',
    view: 'View', theme: 'Theme', system: 'Follow System', light: 'Light', dark: 'Dark', walnut: 'Walnut',
    quit: 'Quit Gitwood', edit: 'Edit', undo: 'Undo', redo: 'Redo', cut: 'Cut', copy: 'Copy', paste: 'Paste', selectAll: 'Select All',
    settingsError: 'Settings could not be loaded or saved', themeError: 'Theme validation errors',
    git: 'Git', update: 'Update Project…', commitAction: 'Commit…', push: 'Push…', commit: 'Commit',
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
    file: '文件', open: '打开…', recentProjects: '最近的项目', closeProject: '关闭项目',
    view: '视图', theme: '主题', system: '跟随系统', light: '浅色', dark: '深色', walnut: '胡桃木',
    quit: '退出 Gitwood', edit: '编辑', undo: '撤销', redo: '重做', cut: '剪切', copy: '复制', paste: '粘贴', selectAll: '全选',
    settingsError: '无法加载或保存设置', themeError: '主题校验错误',
    git: 'Git', update: '更新项目…', commitAction: '提交…', push: '推送…', commit: '提交',
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
