import { describe, expect, it, vi } from 'vitest';
import { hostingMenuItems, type HostingTarget } from '../src/features/git/hosting-menu';
import { hostingText } from '../src/i18n/hosting';
import { revealText } from '../src/i18n/reveal';

describe('hosting context menu', () => {
  it('shows only recognized hosting platforms and includes open and copy actions per remote', () => {
    const open = vi.fn(), copy = vi.fn();
    const target: HostingTarget = { kind: 'commit', revision: 'a'.repeat(40) };
    const items = hostingMenuItems([
      { name: 'origin', platform: 'github' }, { name: 'mirror', platform: 'github' }, { name: 'upstream', platform: 'gitlab' },
    ], target, open, copy, key => hostingText(key, 'en'));
    expect(items.map(item => item.label)).toEqual(['GitHub', 'GitLab']);
    const github = items[0];
    expect(github.children?.map(item => item.label)).toEqual(['origin', 'mirror']);
    github.children?.[1].children?.[0].run?.();
    github.children?.[0].children?.[1].run?.();
    expect(open).toHaveBeenCalledWith('mirror', target);
    expect(copy).toHaveBeenCalledWith('origin', target);
  });

  it('hides unsupported remotes and uses fixed English strings when requested', () => {
    expect(hostingMenuItems([], { kind: 'branch', branch: 'main' }, vi.fn(), vi.fn(), key => hostingText(key, 'en'))).toEqual([]);
    expect(hostingText('open', 'en')).toBe('Open in Browser');
    expect(hostingText('open', 'zh-CN')).toBe('在浏览器中打开');
  });

  it('localizes Reveal In separately for each desktop file manager', () => {
    expect(revealText('mac', 'en')).toBe('Reveal in Finder');
    expect(revealText('windows', 'en')).toBe('Show in Explorer');
    expect(revealText('linux', 'en')).toBe('Show in Files');
    expect(revealText('mac', 'zh-CN')).toBe('在 Finder 中显示');
    expect(revealText('windows', 'zh-CN')).toBe('在资源管理器中显示');
    expect(revealText('linux', 'zh-CN')).toBe('在文件管理器中显示');
  });
});
