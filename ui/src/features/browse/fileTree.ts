import { browseText as t } from '../../i18n/browse';

export function filePathParts(path: string) {
  const separator = path.lastIndexOf('/');
  return { name: path.slice(separator + 1), directory: separator < 0 ? '' : path.slice(0, separator) };
}
export function fileCount(count: number) {
  return count === 1 ? t('oneFile') : t('fileCount').replace('{count}', String(count));
}
export function fileGroup(id: string): id is 'files' | 'changes' | 'unversioned' {
  return id === 'files' || id === 'changes' || id === 'unversioned';
}
export function fileRowLabel(row: { id: string; label: string; count: number; path: { display: string } | null }, flat: boolean) {
  if (fileGroup(row.id)) return `${t(row.id)} ${fileCount(row.count)}`;
  if (!row.path || !flat) return row.label;
  const { name, directory } = filePathParts(row.path.display);
  return directory ? `${name} ${directory}` : name;
}
const fileIcons: Record<string, { glyph: string; color: string }> = {
  java: { glyph: 'J', color: 'syntax-number' }, kt: { glyph: 'K', color: 'syntax-keyword' },
  ts: { glyph: 'TS', color: 'status-modified' }, js: { glyph: 'JS', color: 'syntax-number' },
  svelte: { glyph: 'S', color: 'status-untracked' }, rs: { glyph: 'R', color: 'syntax-type' },
  md: { glyph: '≡', color: 'text-secondary' }, yml: { glyph: 'Y', color: 'syntax-string' },
  yaml: { glyph: 'Y', color: 'syntax-string' }, xml: { glyph: '‹›', color: 'syntax-number' },
  sql: { glyph: '▤', color: 'status-modified' }, json: { glyph: '{}', color: 'syntax-number' },
};
export function fileTypeIcon(name: string) {
  return fileIcons[name.slice(name.lastIndexOf('.') + 1).toLowerCase()] ?? { glyph: '≡', color: 'text-secondary' };
}
