// SVGs copied unmodified from intellij-community platform/icons/src/expui (Apache-2.0). See NOTICE.
const urls = import.meta.glob('./*.svg', { eager: true, query: '?url', import: 'default' }) as Record<string, string>;

/** URL of an IDEA icon, preferring its `_dark` variant on dark themes when upstream ships one. */
export function ideaIcon(name: string, dark: boolean): string {
  return (dark && urls[`./${name}_dark.svg`]) || urls[`./${name}.svg`];
}
