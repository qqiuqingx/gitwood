import { describe, expect, it } from 'vitest';
import { constrainSplit } from '../src/components/splitter';

describe('splitter size constraints', () => {
  it('keeps a requested size within the two minimums', () => {
    expect(constrainSplit(320, 1000, 220, 420)).toEqual({
      first: 320, second: 680, min: 220, max: 580,
    });
  });

  it('stops dragging at the first pane minimum', () => {
    expect(constrainSplit(-500, 1000, 220, 420).first).toBe(220);
  });

  it('reserves the second pane minimum at the other end', () => {
    expect(constrainSplit(1500, 1000, 220, 420).second).toBe(420);
  });

  it('handles a host exactly as large as its minimums', () => {
    expect(constrainSplit(0, 640, 220, 420)).toEqual({
      first: 220, second: 420, min: 220, max: 220,
    });
  });

  it('shares insufficient space proportionally without overflow or negative sizes', () => {
    expect(constrainSplit(500, 150, 100, 200)).toEqual({
      first: 50, second: 100, min: 50, max: 50,
    });
  });

  it('handles the initial zero-sized host', () => {
    expect(constrainSplit(500, 0, 100, 200)).toEqual({
      first: 0, second: 0, min: 0, max: 0,
    });
  });

  it('allows zero minimums and an empty host', () => {
    expect(constrainSplit(0, 0, 0, 0)).toEqual({
      first: 0, second: 0, min: 0, max: 0,
    });
  });

  it('reapplies both minimums when a window shrinks', () => {
    const initial = constrainSplit(600, 1200, 220, 420);
    const shrunk = constrainSplit(initial.first / 1200 * 700, 700, 220, 420);
    expect(shrunk.first).toBe(280);
    expect(shrunk.second).toBe(420);
  });
});
