/** Adapted from JetBrains intellij-community (Apache-2.0), Copyright 2000-2025
 * JetBrains s.r.o. and contributors. See NOTICE. SpeedSearchComparator(false,true),
 * NameUtilCore.nameToWordList/isWordStart, MinusculeMatcherImpl. No layout/pinyin fallback.
 * Fragments use UTF-16 offsets, as String.slice and DOM text do. */
export type MatchRange = readonly [start: number, end: number];
const upper = (c: string) => /\p{Lu}/u.test(c);
const lower = (c: string) => /\p{Ll}/u.test(c);
const digit = (c: string) => /\p{Nd}/u.test(c);
const letter = (c: string) => /\p{L}/u.test(c);
function boundary(s: string, i: number): boolean {
  const c = s[i], p = s[i - 1] ?? '';
  if (upper(c)) return !upper(p) || lower(s[i + 1] ?? '');
  if (digit(c)) return true;
  if (!letter(c)) return false;
  if (/\p{Unified_Ideograph}/u.test(c)) return true;
  const kana = (v: string) => /[\p{Script=Katakana}\p{Script=Hiragana}]/u.test(v);
  return i === 0 || !/[\p{L}\p{N}]/u.test(p) || (kana(c) || kana(p)) &&
    (/\p{Script=Katakana}/u.test(c) !== /\p{Script=Katakana}/u.test(p) || /\p{Script=Hiragana}/u.test(c) !== /\p{Script=Hiragana}/u.test(p));
}
function words(query: string): string[] {
  const result: string[] = [];
  for (let i = 0; i < query.length;) {
    const start = i;
    let u = 0, l = 0, d = 0, special = 0;
    for (; i < query.length; i++) {
      const c = query[i];
      if (digit(c)) { if (u || l || special) break; d++; }
      else if (upper(c)) { if (l || d || special) break; u++; }
      else if (lower(c)) { if (d || special) break; if (u > 1) { i--; break; } l++; }
      else { if (u || l || d) break; special++; }
    }
    const word = query.slice(start, i);
    if (word.trim()) result.push(word);
  }
  return result;
}
export function matchSpeedSearch(query: string, text: string): MatchRange[] | null {
  let pattern = words(query).join('*');
  if (!pattern.startsWith('*')) pattern = '*' + pattern;
  if (pattern.endsWith('* ')) pattern = pattern.slice(0, -2);
  const wildcard = (i: number) => pattern[i] === '*' || pattern[i] === ' ';
  const equal = (p: number, n: number) => n < text.length &&
    (pattern[p] === text[n] || pattern[p].toLowerCase() === text[n].toLowerCase() || pattern[p].toUpperCase() === text[n].toUpperCase());
  const mixed = [...pattern].some(lower) && [...pattern.replace(/^[* ]*./u, '')].some(upper);
  const separator = (c: string) => /[\s_\-:+.]/u.test(c);
  const next = (p: number, from: number): number => {
    if (pattern[p - 1] !== '*' && !separator(pattern[p]) && mixed && lower(pattern[p]) && !separator(pattern[p - 1] ?? '')) return -1;
    for (let n = from; n < text.length; n++) if (equal(p, n) &&
      (pattern[p - 1] === '*' || separator(pattern[p]) || !/[\p{L}\p{N}]/u.test(pattern[p]) || boundary(text, n))) return n;
    return -1;
  };
  const append = (n: number, len: number, rest: MatchRange[]): MatchRange[] =>
    rest[0]?.[0] === n + len ? [[n, rest[0][1]], ...rest.slice(1)] : [[n, n + len], ...rest];
  function wild(p: number, n: number): MatchRange[] | null {
    if (n < 0) return null;
    if (!wildcard(p)) return p === pattern.length ? [] : fragment(p, n);
    while (wildcard(p)) p++;
    if (p === pattern.length) {
      if (pattern.endsWith(' ') && n !== text.length && !upper(pattern[p - 2] ?? '') && !digit(pattern[p - 2] ?? '')) {
        const space = text.indexOf(' ', n); return space < 0 ? null : [[space, space + 1]];
      }
      return [];
    }
    return skipping(p, next(p, n));
  }
  function skipping(p: number, n: number): MatchRange[] | null {
    for (; n >= 0; n = next(p, n + 1)) {
      if (upper(pattern[p]) && !upper(text[n]) && !boundary(text, n) && mixed) continue;
      const result = fragment(p, n); if (result) return result;
    }
    return null;
  }
  const fragments = new Map<string, MatchRange[] | null>();
  function fragment(p: number, n: number): MatchRange[] | null {
    const key = `${p}:${n}`;
    if (fragments.has(key)) return fragments.get(key)!;
    const result = computeFragment(p, n);
    fragments.set(key, result);
    return result;
  }
  function computeFragment(p: number, n: number): MatchRange[] | null {
    let len = 0;
    while (p + len < pattern.length && equal(p + len, n + len)) len++;
    if (!len) return null;
    if (digit(pattern[p + len] ?? '') && digit(pattern[p + len - 1]) && digit(text[n + len] ?? '')) return null;
    const min = pattern[p - 1] === '*' && !wildcard(p + 1) && /[\p{L}\p{N}]/u.test(text[n]) && !boundary(text, n) ? 3 : 1;
    // improveCamelHumps prefers CurrentUser's U over the lowercase u in Current.
    for (let i = min; i < len; i++) if (upper(pattern[p + i]) && pattern[p + i] !== text[n + i]) {
      for (let at = n + i; at < text.length; at++) if (boundary(text, at) && equal(p + i, at)) {
        const rest = wild(p + i, at); if (rest) return append(n, i, rest); break;
      }
    }
    if (p + len === pattern.length) return [[n, n + len]];
    for (let i = len; i >= min || i > 0 && wildcard(p + i); i--) {
      const at = wildcard(p + i) ? n + i : next(p + i, n + i + 1);
      if (!wildcard(p + i) && pattern.includes('.') && pattern[p + i - 1] !== '.' && text.slice(n + i, at).includes('.')) continue;
      const rest = wildcard(p + i) ? wild(p + i, at) : skipping(p + i, at);
      if (rest) return append(n, i, rest);
    }
    return null;
  }
  if (pattern.length > 100) {
    const chars = [...pattern].filter(c => c !== '*' && c !== ' ').join('');
    for (let n = 0; n <= text.length - chars.length; n++) {
      let i = 0, at = n;
      while (at < text.length && i < chars.length) {
        if (chars[i].toLowerCase() === text[at].toLowerCase()) i++;
        else if (text[at] !== '*' && text[at] !== ' ') break;
        at++;
      }
      if (i === chars.length) return [[n, at]];
    }
    return null;
  }
  return wild(0, 0);
}

export function highlightParts(text: string, ranges: readonly MatchRange[]) {
  const parts: { text: string; matched: boolean }[] = [];
  let offset = 0;
  for (const [start, end] of ranges) {
    if (start > offset) parts.push({ text: text.slice(offset, start), matched: false });
    parts.push({ text: text.slice(start, end), matched: true });
    offset = end;
  }
  if (offset < text.length) parts.push({ text: text.slice(offset), matched: false });
  return parts;
}

/** Scan without allocating an array of every match; wrap like cyclic list navigation. */
export function findSearchMatch(
  count: number, getText: (index: number) => string | null, query: string,
  active: number, direction: 1 | -1, includeCurrent = false,
): number {
  if (!count) return -1;
  const origin = active < 0 ? (direction === 1 ? -1 : 0) : active;
  for (let offset = includeCurrent && active >= 0 ? 0 : 1; offset <= count; offset++) {
    const index = ((origin + offset * direction) % count + count) % count;
    const text = getText(index);
    if (text !== null && matchSpeedSearch(query, text) !== null) return index;
  }
  return -1;
}
