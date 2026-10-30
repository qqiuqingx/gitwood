import { detectLocale } from './index';
const messages = {
  en: { projects: 'Projects', new: 'New Project…', open: 'Open…', clone: 'Clone Repository…', opened: 'Open Projects', recent: 'Recent Projects' },
  'zh-CN': { projects: '项目', new: '新建项目…', open: '打开…', clone: '克隆仓库…', opened: '已打开的项目', recent: '最近的项目' },
};
export function projectText(key: keyof typeof messages.en): string { return messages[detectLocale(navigator.language)][key]; }
