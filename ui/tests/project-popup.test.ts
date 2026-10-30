import { describe, expect, it } from 'vitest';
import { initials, projectColorIndex, projectName } from '../src/features/projects/avatar';
import { projectPopupItems } from '../src/features/projects/model';
import { filterPopupItems } from '../src/components/popup';
import { selectable, visibleRange } from '../src/components/list';
import { render } from 'svelte/server';
import ProjectAvatar from '../src/features/projects/ProjectAvatar.svelte';
import Popup from '../src/components/Popup.svelte';

describe('IDEA project avatar', () => {
  it.each([
    ['MyProject', 'MP'], ['myProject', 'MP'], ['My-Project', 'MP'], ['John-Smith-Harris', 'JH'],
    ['fooBar baz', 'FB'], ['John , Smith', 'JS'], ['foo_bar_baz', 'FB'], ['one.two.three', 'OT'],
    ['one`two\'three"four', 'OF'], ['123abc', 'A'], ['  --  ', ''], ['😀Project😀', 'P'], ['项目 测试', '项测'],
  ])('%s → %s', (name, expected) => { expect(initials(name)).toBe(expected); });
  it('uses a stable Java string hash, including negative and MIN_VALUE hashes', () => {
    expect(projectColorIndex('abc')).toBe(96354 % 9);
    expect(projectColorIndex('polygenelubricants')).toBe(2147483648 % 9);
    expect(projectColorIndex('zzzzzz')).toBe(1);
    for (const path of ['', '/repo', '/中文/项目', '😀', 'different']) {
      expect(projectColorIndex(path)).toBeGreaterThanOrEqual(0); expect(projectColorIndex(path)).toBeLessThan(9);
    }
  });
  it('gets the final directory name on both supported path styles', () => {
    expect(projectName('/repo/MyProject/')).toBe('MyProject');
    expect(projectName('C:\\repo\\my-project')).toBe('my-project');
    expect(projectName('/')).toBe('/');
  });
  it('renders plain text, theme palette references and failed desaturation class', () => {
    const body = render(ProjectAvatar, { props: { path: '/repo', name: '<Project>', failed: true } }).body;
    expect(body).toContain('failed'); expect(body).toContain('--project-avatar-'); expect(body).not.toContain('<Project>');
  });
});

const labels = { new: 'New Project…', open: 'Open…', clone: 'Clone Repository…', opened: 'Open Projects', recent: 'Recent Projects', failed: 'Failed' };
describe('project popup', () => {
  it('separates the open project, excludes duplicates, and keeps failed projects actionable', () => {
    const selected: string[] = [];
    const items = projectPopupItems('/repo/current', [
      { path: '/repo/current', displayPath: '~/current', failed: false },
      { path: '/repo/missing', displayPath: '~/missing', failed: true },
    ], labels, () => selected.push('open'), path => selected.push(path));
    expect(items.slice(0, 3).map(item => [item.id, !!item.disabled])).toEqual([['new', true], ['open', false], ['clone', true]]);
    expect(items.filter(item => item.id === '/repo/current')).toHaveLength(1);
    expect(items.find(item => item.id === 'opened')?.firstGroup).toBe(true);
    const missing = items.find(item => item.id === '/repo/missing')!;
    expect(missing.description).toBe('~/missing (Failed)'); expect(missing.avatar?.failed).toBe(true); expect(selectable(missing)).toBe(true);
    missing.run!(); items[1].run!(); expect(selected).toEqual(['/repo/missing', 'open']);
    expect(filterPopupItems(items, 'missing').map(item => item.id)).toEqual(['recent', '/repo/missing']);
    expect(filterPopupItems(items, 'missing')[0].firstGroup).toBe(true);
    expect(filterPopupItems(items, 'no-such-path')).toEqual([]);
  });
  it('keeps plain menus unchanged and searches disabled actions and descriptions', () => {
    const items = projectPopupItems(undefined, [{ path: '/work/repo', displayPath: '~/work/repo', failed: false }], labels, () => {}, () => {});
    expect(filterPopupItems(items, '')).toBe(items);
    expect(filterPopupItems(items, 'Clone')[0].disabled).toBe(true);
    expect(filterPopupItems(items, '~/work')[1].id).toBe('/work/repo');
    expect(items.find(item => item.id === 'recent')?.firstGroup).toBe(true);
  });
  it('offers an editable speed search only for filtering popups', () => {
    const props = { items: [], label: 'Projects', anchor: { x: 0, y: 0, width: 0, height: 0 }, onclose: () => {} };
    expect(render(Popup, { props: { ...props, filter: true } }).body).toContain('<input');
    expect(render(Popup, { props }).body).not.toContain('<input');
  });
  it('keeps two-line row geometry within virtualized bounds', () => {
    expect(visibleRange(4, 24, 25, 40, 0, [0, 24, 72, 108, 156])).toEqual({ start: 1, end: 2 });
    expect(visibleRange(4, 24, 130, 50, 0, [0, 24, 72, 108, 156])).toEqual({ start: 3, end: 4 });
  });
});
