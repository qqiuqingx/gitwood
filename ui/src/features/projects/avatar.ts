// Copyright 2000-2025 JetBrains s.r.o. and contributors.
// Adapted from AvatarUtils.kt under the Apache-2.0 license. See NOTICE.
export function initials(text: string): string {
  const letter = /\p{L}/u, alphanumeric = /[\p{L}\p{N}]/u;
  // Kotlin Char processing ignores surrogate pairs; preserve that upstream rule.
  const characters = text.split('');
  const first = characters.findIndex(char => letter.test(char));
  const word: string[] = [];
  for (let i = first; i >= 0 && i < characters.length && alphanumeric.test(characters[i]); i++) word.push(characters[i]);
  const camel = word.filter((char, index) => index === 0 || /\p{Lu}/u.test(char)).slice(0, 2).join('').toUpperCase();
  if (camel.length === 2) return camel;
  const filtered = text.replace(/[\uD800-\uDFFF]/g, '').trim();
  for (const delimiter of [/ /, /,/, /-/, /_/, /\./, /[`'"]/]) {
    const words = filtered.split(delimiter).map(word => word.replace(/[^\p{L}\p{N}]/gu, '')).filter(Boolean);
    if (words.length >= 2) return (words[0][0] + words.at(-1)![0]).toUpperCase();
  }
  return camel;
}

/** Java String.hashCode, with Math.abs in JS avoiding Int.MIN_VALUE overflow. */
export function projectColorIndex(path: string): number {
  let hash = 0;
  for (let i = 0; i < path.length; i++) hash = (Math.imul(hash, 31) + path.charCodeAt(i)) | 0;
  return Math.abs(hash) % 9;
}
export function projectName(path: string): string {
  return path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;
}
