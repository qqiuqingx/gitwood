import { detectLocale } from './index';
const messages = {
  en: { projects: 'Projects', new: 'New Project…', open: 'Open…', clone: 'Clone Repository…', opened: 'Open Projects', recent: 'Recent Projects', openProject: 'Open Project', openWhere: 'Where would you like to open “{project}”?', currentWindow: 'This Window', newWindow: 'New Window', cancel: 'Cancel' },
  'zh-CN': { projects: '项目', new: '新建项目…', open: '打开…', clone: '克隆仓库…', opened: '已打开的项目', recent: '最近的项目', openProject: '打开项目', openWhere: '你希望在哪个窗口打开“{project}”？', currentWindow: '当前窗口', newWindow: '新窗口', cancel: '取消' },
};
export function projectText(key: keyof typeof messages.en): string { return messages[detectLocale(navigator.language)][key]; }
