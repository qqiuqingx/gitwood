import { detectLocale } from './index';
const messages = {
  en: {
    menu: 'Current File', history: 'Show History', annotate: 'Annotate', previous: 'Annotate Previous Revision',
    compareRevision: 'Compare with Revision…', compareBranch: 'Compare with Branch…', close: 'Close', loading: 'Loading…',
    revision: 'Revision', file: 'File', commits: 'Commits', choose: 'Compare', noHistory: 'No file history found',
    binary: 'Binary or non UTF-8 files cannot be annotated', canceled: 'Annotation canceled',
  },
  'zh-CN': {
    menu: '当前文件', history: '显示历史记录', annotate: '注释', previous: '注释上一修订',
    compareRevision: '与修订比较…', compareBranch: '与分支比较…', close: '关闭', loading: '正在加载…',
    revision: '修订', file: '文件', commits: '提交', choose: '比较', noHistory: '未找到文件历史记录',
    binary: '无法注释二进制文件或非 UTF-8 文件', canceled: '注释已取消',
  },
} as const;
export type CurrentFileMessage = keyof typeof messages.en;
export function currentFileText(key: CurrentFileMessage, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return messages[detectLocale(language)][key];
}
