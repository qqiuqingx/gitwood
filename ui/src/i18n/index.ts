const messages = {
  en: {
    file: 'File', open: 'Open…', recentProjects: 'Recent Projects', closeProject: 'Close Project',
    view: 'View', theme: 'Theme', system: 'Follow System', light: 'Light', dark: 'Dark', walnut: 'Walnut',
    quit: 'Quit Gitwood', edit: 'Edit', undo: 'Undo', redo: 'Redo', cut: 'Cut', copy: 'Copy', paste: 'Paste', selectAll: 'Select All',
    settingsError: 'Settings could not be loaded or saved', themeError: 'Theme validation errors',
    themeSettings: 'Themes…', themeSettingsTitle: 'Theme Settings', themeDirectory: 'Theme directory',
    copyPath: 'Copy path', pathCopied: 'Path copied', themeCopyFailed: 'Could not copy path',
    openThemeDirectory: 'Open theme directory', themeDirectoryOpenFailed: 'Could not open theme directory',
    editorTabs: 'Editor Tabs', editorTabLimit: 'Tab limit', saveTabLimit: 'Save Tab Limit', tabLimitSaved: 'Tab limit saved',
    hostingSettings: 'Hosting hosts', githubHosts: 'GitHub Enterprise hosts', gitlabHosts: 'Self-hosted GitLab hosts',
    hostingHostsHelp: 'Enter one hostname and optional port per line.', saveHosts: 'Save Hosts', hostsSaved: 'Hosting hosts saved',
    themeErrors: 'Theme file errors', noThemeErrors: 'No theme file errors',
    contrastWarnings: 'Contrast warnings', contrastHelp: 'These text and background pairs are below the WCAG 2.x contrast target. You can still use the theme.',
    noContrastWarnings: 'No contrast warnings for this theme', themeCancel: 'Cancel', themeApply: 'Apply Theme',
    themeLoading: 'Loading themes…', themePreviewHint: 'Selecting a theme previews its colors. Cancel restores the current theme.',
    themeUnavailable: 'This theme is no longer available', themeBuiltin: 'Built-in', themeUser: 'User theme',
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
    themeSettings: '主题…', themeSettingsTitle: '主题设置', themeDirectory: '主题目录',
    copyPath: '复制路径', pathCopied: '路径已复制', themeCopyFailed: '无法复制路径',
    openThemeDirectory: '打开主题目录', themeDirectoryOpenFailed: '无法打开主题目录',
    editorTabs: '编辑器标签页', editorTabLimit: '标签页数量上限', saveTabLimit: '保存标签页上限', tabLimitSaved: '标签页上限已保存',
    hostingSettings: '托管平台主机', githubHosts: 'GitHub Enterprise 主机', gitlabHosts: '自建 GitLab 主机',
    hostingHostsHelp: '每行输入一个主机名，可附带端口。', saveHosts: '保存主机列表', hostsSaved: '托管平台主机已保存',
    themeErrors: '主题文件错误', noThemeErrors: '没有主题文件错误',
    contrastWarnings: '对比度提示', contrastHelp: '以下文字与背景组合低于 WCAG 2.x 对比度目标。仍可使用此主题。',
    noContrastWarnings: '此主题没有对比度提示', themeCancel: '取消', themeApply: '应用主题',
    themeLoading: '正在加载主题…', themePreviewHint: '选择主题即可预览颜色。取消会恢复当前主题。',
    themeUnavailable: '此主题已不可用', themeBuiltin: '内置', themeUser: '用户主题',
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
