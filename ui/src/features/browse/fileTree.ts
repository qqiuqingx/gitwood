import { conflictText } from '../../i18n/conflict';
import { browseText as t } from '../../i18n/browse';

export function filePathParts(path: string) {
  const separator = path.lastIndexOf('/');
  return { name: path.slice(separator + 1), directory: separator < 0 ? '' : path.slice(0, separator) };
}
export function fileCount(count: number) {
  return count === 1 ? t('oneFile') : t('fileCount').replace('{count}', String(count));
}
export function fileGroup(id: string): id is 'files' | 'changes' | 'unversioned' | 'conflicts' {
  return id === 'conflicts' || id === 'files' || id === 'changes' || id === 'unversioned';
}
export function fileRowLabel(row: { id: string; label: string; count: number; path: { display: string } | null }, flat: boolean) {
  if (fileGroup(row.id)) return `${row.id === 'conflicts' ? conflictText('title') : t(row.id)} ${fileCount(row.count)}`;
  if (!row.path || !flat) return row.label;
  const { name, directory } = filePathParts(row.path.display);
  return directory ? `${name} ${directory}` : name;
}
// IDEA expui icon per extension; file names cannot tell Java classes from interfaces, so `.java` uses the class icon.
const fileIcons: Record<string, string> = {
  java: 'class', vue: 'vue', js: 'javaScript', mjs: 'javaScript', cjs: 'javaScript', jsx: 'javaScript',
  ts: 'typeScript', mts: 'typeScript', cts: 'typeScript', tsx: 'typeScript', json: 'json', xml: 'xml',
  yml: 'yaml', yaml: 'yaml', sql: 'sql', html: 'html', htm: 'html', css: 'css', scss: 'css', less: 'css',
  properties: 'properties', sh: 'shell', bash: 'shell', zsh: 'shell', gradle: 'gradle', groovy: 'groovy',
  png: 'image', jpg: 'image', jpeg: 'image', gif: 'image', svg: 'image', webp: 'image', ico: 'image',
  csv: 'csv', toml: 'toml', c: 'c', cpp: 'cpp', cc: 'cpp', cxx: 'cpp', hpp: 'cpp', h: 'h',
  patch: 'patch', diff: 'patch', zip: 'archive', jar: 'archive', war: 'archive', tar: 'archive', gz: 'archive',
  conf: 'config', ini: 'config', cfg: 'config',
};
export function fileTypeIcon(name: string): string {
  if (name === 'Dockerfile') return 'docker';
  const dot = name.lastIndexOf('.');
  return (dot > 0 && fileIcons[name.slice(dot + 1).toLowerCase()]) || 'text';
}
