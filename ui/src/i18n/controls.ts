import { detectLocale } from './index';
const messages = {
  en: { search: 'Speed search', noMatch: 'No matches', gallery: 'Component gallery',
    list: 'Virtual list', tree: 'File tree', actions: 'Actions', openPopup: 'Open actions',
    preview: 'Preview', unavailable: 'Unavailable action', first: 'List', second: 'Tree',
    closeTree: 'Close tree tab', defaultAction: 'Choose', selected: 'Selected', log: 'Git Log', commit: 'Commit',
    close: 'Close', group: 'Examples', submenu: 'More actions', action: 'Choose item', disabled: 'Disabled item', hiddenTabs: 'Show hidden tabs' },
  'zh-CN': { search: '快速搜索', noMatch: '无匹配项', gallery: '组件展示',
    list: '虚拟列表', tree: '文件树', actions: '操作', openPopup: '打开操作',
    preview: '预览', unavailable: '不可用操作', first: '列表', second: '树',
    closeTree: '关闭树标签', defaultAction: '选择', selected: '已选择', log: 'Git 日志', commit: '提交',
    close: '关闭', group: '示例', submenu: '更多操作', action: '选择项目', disabled: '禁用项目', hiddenTabs: '显示隐藏的标签页' },
} as const;
export function controlText(key: keyof typeof messages.en, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return messages[detectLocale(language)][key];
}
