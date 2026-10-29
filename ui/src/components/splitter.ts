export interface SplitSize {
  first: number;
  second: number;
  min: number;
  max: number;
}

/** Clamp both panes to their minimums; a too-small host shares space proportionally. */
export function constrainSplit(
  requested: number,
  available: number,
  minFirst: number,
  minSecond: number,
): SplitSize {
  const total = Math.max(0, available);
  const firstMinimum = Math.max(0, minFirst);
  const secondMinimum = Math.max(0, minSecond);
  const minimumTotal = firstMinimum + secondMinimum;

  if (total < minimumTotal) {
    const first = total * (firstMinimum / minimumTotal);
    return { first, second: total - first, min: first, max: first };
  }

  const min = firstMinimum;
  const max = total - secondMinimum;
  const first = Math.min(max, Math.max(min, requested));
  return { first, second: total - first, min, max };
}
