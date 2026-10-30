import { describe, expect, it } from 'vitest';
import { findSearchMatch, highlightParts, matchSpeedSearch } from '../src/components/speed-search';
describe('speed search and plain text highlights', () => {
  it.each([
    ['gbp', 'GitBranchPanel', [[0, 1], [3, 4], [9, 10]]],
    ['GitBr', 'GitBranchPanel', [[0, 5]]],
    ['fea bra', 'feature-branch', [[0, 3], [8, 11]]],
    ['XMLP', 'XMLParser', [[0, 4]]],
    ['branch', 'feature/Branch', [[8, 14]]],
    ['b2', 'Branch2', [[0, 1], [6, 7]]],
    ['文件', 'src/文件.ts', [[4, 6]]],
    ['<img>', 'a<img>.txt', [[1, 6]]],
    ['', 'anything', []],
  ])('matches %s in %s with source offsets', (query, text, expected) => {
    expect(matchSpeedSearch(query, text)).toEqual(expected);
  });
  it('does not allow arbitrary subsequence gaps inside a word', () => {
    expect(matchSpeedSearch('fbr', 'foobar')).toBeNull();
    expect(matchSpeedSearch('missing', 'GitBranchPanel')).toBeNull();
  });
  it('backtracks past an unhelpful substring to find a later word', () => {
    expect(matchSpeedSearch('gb', 'giraffe GitBranch')).toEqual([[0, 1], [11, 12]]);
  });
  it('highlights only matched fragments and preserves hostile text verbatim', () => {
    const text = '<script>GitBranch</script>';
    const parts = highlightParts(text, matchSpeedSearch('gb', text)!);
    expect(parts.map((p) => p.text).join('')).toBe(text);
    expect(parts.filter((p) => p.matched).map((p) => p.text)).toEqual(['G', 'B']);
  });
  it('moves between matches, wraps, skips nonselectable rows, and reports no match', () => {
    const text = ['GitBranch', null, 'No', 'GitButton'];
    expect(findSearchMatch(4, (i) => text[i], 'gb', 0, 1)).toBe(3);
    expect(findSearchMatch(4, (i) => text[i], 'gb', 3, 1)).toBe(0);
    expect(findSearchMatch(4, (i) => text[i], 'gb', 0, -1)).toBe(3);
    expect(findSearchMatch(4, (i) => text[i], 'gb', 0, 1, true)).toBe(0);
    expect(findSearchMatch(4, (i) => text[i], 'missing', 0, 1)).toBe(-1);
    expect(findSearchMatch(0, () => '', 'gb', -1, 1)).toBe(-1);
  });
});

// SpeedSearchComparator.obtainMatcher splits nameToWordList with '*', then
// NameUtil.MatcherBuilder defaults to IGNORE_CASE (not FIRST_LETTER).
describe('upstream matcher rules', () => {
  it.each([
    ['git', 'GitBranch', [[0, 3]]],
    ['Git', 'gitBranch', [[0, 3]]],
    // MinusculeMatcherImpl.improveCamelHumps prefers later uppercase U.
    ['CU', 'CurrentUser', [[0, 1], [7, 8]]],
    // nameToWordList inserts * at camel/digit boundaries.
    ['gB', 'git branch', [[0, 1], [4, 5]]],
    ['b2', 'Branch12', [[0, 1], [7, 8]]],
    // isSkippingDigitBetweenPatternDigits rejects skipping inside a number.
    ['12', '102', null],
    // obtainMatcher discards blank words and inserts stars; '*' permits middle fragments.
    ['foo*ar', 'foobar', [[0, 3], [4, 6]]],
    ['foo ar', 'foobar', [[0, 3], [4, 6]]],
    ['foo ar', 'foo array', [[0, 3], [4, 6]]],
    // nameToWordList discards blank words (no trim/whitespace rewrite).
    ['git ', 'GitBranch', [[0, 3]]],
    // isMiddleMatch/findLongestMatchingPrefix accepts short complete suffixes.
    ['it', 'GitBranch', [[1, 3]]],
    ['***', 'anything', []],
  ])('%s / %s', (query, text, expected) => expect(matchSpeedSearch(query, text)).toEqual(expected));
});
