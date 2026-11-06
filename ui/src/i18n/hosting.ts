import { detectLocale } from './index';
const en = { github: 'GitHub', gitlab: 'GitLab', open: 'Open in Browser', copy: 'Copy Link', createPull: 'Create Pull Request', createMerge: 'Create Merge Request' };
const zh: typeof en = { github: 'GitHub', gitlab: 'GitLab', open: '在浏览器中打开', copy: '复制链接', createPull: '创建拉取请求', createMerge: '创建合并请求' };
export type HostingMessage = keyof typeof en;
export function hostingText(key: HostingMessage, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return (detectLocale(language) === 'zh-CN' ? zh : en)[key];
}
