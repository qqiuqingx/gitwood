import { detectLocale } from './index';
const messages = {
  en: { diff: 'Diff', left: 'Left version', right: 'Right version', next: 'Next Difference', previous: 'Previous Difference',
    differences: '{count} differences', noDiff: 'No differences', closeTab: 'Close diff tab', emptyRevision: 'Empty',
    binary: 'Binary file', undecodable: 'Cannot decode as UTF-8', tooLarge: 'File is too large', empty: 'Empty file',
    noNewline: 'No newline at end of file', expand: 'Expand unchanged lines', applyLeft: 'Apply to left (stage 3)',
    applyRight: 'Apply to right (stage 3)', resize: 'Resize diff panes', bytes: 'bytes', example: 'Diff example' },
  'zh-CN': { diff: '差异', left: '左侧版本', right: '右侧版本', next: '下一处差异', previous: '上一处差异',
    differences: '{count} 处差异', noDiff: '无差异', closeTab: '关闭差异标签', emptyRevision: '空版本',
    binary: '二进制文件', undecodable: '无法按 UTF-8 解码', tooLarge: '文件过大', empty: '空文件',
    noNewline: '文件末尾无换行', expand: '展开未修改行', applyLeft: '应用到左侧（阶段 3）',
    applyRight: '应用到右侧（阶段 3）', resize: '调整差异左右宽度', bytes: '字节', example: '差异示例' },
} as const;
export function diffText(key: keyof typeof messages.en, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return messages[detectLocale(language)][key];
}
