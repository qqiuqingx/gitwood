import { describe, expect, it } from 'vitest';
import { positionPopup } from '../src/components/popup';
const viewport = { width: 800, height: 600 };
describe('popup placement', () => {
  it('opens below an anchor or at a pointer', () => {
    expect(positionPopup({ x: 30, y: 40, width: 60, height: 20 }, { width: 200, height: 100 }, viewport, 8))
      .toEqual({ x: 30, y: 60, width: 200, height: 100 });
    expect(positionPopup({ x: 30, y: 40, width: 0, height: 0 }, { width: 200, height: 100 }, viewport, 8).y).toBe(40);
  });
  it('flips upward and aligns the right edge at viewport bounds', () => {
    expect(positionPopup({ x: 750, y: 550, width: 20, height: 20 }, { width: 200, height: 100 }, viewport, 8))
      .toEqual({ x: 570, y: 450, width: 200, height: 100 });
  });
  it('opens submenus beside a row and flips them left', () => {
    expect(positionPopup({ x: 600, y: 80, width: 150, height: 24 }, { width: 200, height: 100 }, viewport, 8, true).x).toBe(400);
    expect(positionPopup({ x: 50, y: 80, width: 150, height: 24 }, { width: 200, height: 100 }, viewport, 8, true).x).toBe(200);
  });
  it('constrains huge popups and offscreen anchors to the viewport', () => {
    expect(positionPopup({ x: -500, y: -500, width: 0, height: 0 }, { width: 2000, height: 2000 }, viewport, 8))
      .toEqual({ x: 8, y: 8, width: 784, height: 584 });
  });
});
