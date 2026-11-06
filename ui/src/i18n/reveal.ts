import { detectLocale } from './index';
import type { Platform } from '../keymap/shortcuts';

const messages = {
  en: { mac: 'Reveal in Finder', windows: 'Show in Explorer', linux: 'Show in Files' },
  'zh-CN': { mac: '在 Finder 中显示', windows: '在资源管理器中显示', linux: '在文件管理器中显示' },
} as const;

export function revealText(platform: Platform, language = typeof navigator === 'undefined' ? 'en' : navigator.language): string {
  return messages[detectLocale(language)][platform];
}
