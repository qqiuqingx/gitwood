import { detectLocale } from './index';
const messages = {
  en: { andOthers: 'and another',
    file: 'File', open: 'Open…', recentProjects: 'Recent Projects', closeProject: 'Close Project', welcome: 'Welcome to Gitwood',
    chooseRepository: 'Open a local Git repository to get started.', opening: 'Opening project…', loading: 'Loading…',
    errorTitle: 'Cannot Open Project', error: 'Unable to read the repository.', unsafeDirectory: 'Git has refused access because this repository has a different owner. Review its ownership and your Git safe.directory configuration.',
    bareUnsupported: 'Bare repositories cannot be opened. Choose a working tree.', notRepository: 'The selected directory is not an accessible Git repository.',
    ok: 'OK', retry: 'Retry', refresh: 'Refresh', changes: 'Changes', unversioned: 'Unversioned Files', staged: 'Staged', unstaged: 'Unstaged', conflicted: 'Conflicted',
    local: 'Local', remote: 'Remote', tags: 'Tags', head: 'HEAD (Current Branch)', all: 'All',
    text: 'Text or hash', branch: 'Branch', user: 'User', date: 'Date', paths: 'Paths', since: 'After', until: 'Before', apply: 'Apply', clear: 'Clear',
    noCommits: 'No commits found', noChanges: 'No changes', selectCommit: 'Select a commit to view details', diffPlaceholder: 'Select a file to view differences',
    subject: 'Subject', author: 'Author', today: 'Today', yesterday: 'Yesterday', moments: 'Moments ago', minute: 'A minute ago', minutes: '{count} minutes ago', hour: '1 hour ago', parents: 'Parents', hash: 'Hash', files: 'Changed Files',
    fileCount: '{count} files', oneFile: '1 file', viewOptions: 'View Options', showPreview: 'Show Diff Preview', shelveSilently: 'Shelve Silently', groupDirectories: 'Group by Directory', expandAll: 'Expand All', collapseAll: 'Collapse All', shelf: 'Shelf', readOnly: 'Read-only', failed: 'Unavailable',
    commitMessage: 'Commit message', amend: 'Amend', commit: 'Commit', commitPush: 'Commit and Push…', log: 'Log', close: 'Close',
  },
  'zh-CN': { andOthers: '和另外',
    file: '文件', open: '打开…', recentProjects: '最近的项目', closeProject: '关闭项目', welcome: '欢迎使用 Gitwood',
    chooseRepository: '打开本地 Git 仓库开始浏览。', opening: '正在打开项目…', loading: '正在加载…',
    errorTitle: '无法打开项目', error: '无法读取仓库。', unsafeDirectory: 'Git 因仓库所有者不同而拒绝访问。请检查仓库所有权及 Git safe.directory 配置。',
    bareUnsupported: '不支持打开裸仓库，请选择工作区目录。', notRepository: '所选目录不是可访问的 Git 仓库。',
    ok: '确定', retry: '重试', refresh: '刷新', changes: '变更', unversioned: '未版本化的文件', staged: '已暂存', unstaged: '未暂存', conflicted: '冲突',
    local: '本地', remote: '远程', tags: '标签', head: 'HEAD（当前分支）', all: '全部',
    text: '说明或哈希', branch: '分支', user: '用户', date: '日期', paths: '路径', since: '晚于', until: '早于', apply: '应用', clear: '清除',
    noCommits: '未找到提交', noChanges: '没有变更', selectCommit: '选择提交以查看详情', diffPlaceholder: '选择文件以查看差异',
    subject: '说明', author: '作者', today: '今天', yesterday: '昨天', moments: '刚刚', minute: '1 分钟前', minutes: '{count} 分钟前', hour: '1 小时前', parents: '父提交', hash: '哈希', files: '变更文件',
    fileCount: '{count} 个文件', oneFile: '1 个文件', viewOptions: '视图选项', showPreview: '显示差异预览', shelveSilently: '静默搁置', groupDirectories: '按目录分组', expandAll: '全部展开', collapseAll: '全部折叠', shelf: '搁置', readOnly: '只读', failed: '不可用',
    commitMessage: '提交说明', amend: '修订', commit: '提交', commitPush: '提交并推送…', log: '日志', close: '关闭',
  },
} as const;
export type BrowseMessage = keyof typeof messages.en;
export function browseText(key: BrowseMessage, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return messages[detectLocale(language)][key];
}

export function prettyDateWords() {
  return { today: browseText('today'), yesterday: browseText('yesterday'), moments: browseText('moments'), minute: browseText('minute'), minutes: browseText('minutes'), hour: browseText('hour') };
}
