import { describe, expect, it } from 'vitest';
import { fitBounds, formatCoordinate, niceStep, panBy, screenToWorld, visibleBounds, worldToScreen, zoomAt, type Viewport } from './viewport';

const vp: Viewport = { center: { x: 1, y: 2 }, scale: 100, width: 800, height: 600 };

describe('viewport math', () => {
  it('maps the centre to the canvas middle and flips y', () => {
    expect(worldToScreen({ x: 1, y: 2 }, vp)).toEqual({ x: 400, y: 300 });
    const up = worldToScreen({ x: 1, y: 3 }, vp);
    expect(up.y).toBeLessThan(300);
  });

  it('inverts world↔screen exactly', () => {
    const p = { x: -3.25, y: 7.5 };
    const back = screenToWorld(worldToScreen(p, vp), vp);
    expect(back.x).toBeCloseTo(p.x, 12);
    expect(back.y).toBeCloseTo(p.y, 12);
  });

  it('keeps the world point under the cursor fixed while zooming', () => {
    const cursor = { x: 123, y: 456 };
    const before = screenToWorld(cursor, vp);
    const after = screenToWorld(cursor, zoomAt(vp, cursor, 1.7));
    expect(after.x).toBeCloseTo(before.x, 10);
    expect(after.y).toBeCloseTo(before.y, 10);
  });

  it('pans by the pixel delta converted to world units', () => {
    const moved = panBy(vp, 100, -50);
    expect(moved.center.x).toBeCloseTo(0);
    expect(moved.center.y).toBeCloseTo(1.5);
  });

  it('fits bounds inside the canvas with padding', () => {
    const fitted = fitBounds(vp, { min: { x: -2, y: -1 }, max: { x: 2, y: 1 } }, 0.1);
    const vis = visibleBounds(fitted);
    expect(vis.min.x).toBeLessThanOrEqual(-2);
    expect(vis.max.x).toBeGreaterThanOrEqual(2);
    expect(vis.min.y).toBeLessThanOrEqual(-1);
    expect(vis.max.y).toBeGreaterThanOrEqual(1);
    expect(fitted.center).toEqual({ x: 0, y: 0 });
  });

  it('chooses 1-2-5 grid steps', () => {
    for (const s of [3, 30, 300, 3000]) {
      const step = niceStep(s);
      const mant = step / Math.pow(10, Math.floor(Math.log10(step)));
      expect([1, 2, 5]).toContain(Math.round(mant));
    }
  });

  it('formats coordinates with precision matching the step', () => {
    expect(formatCoordinate(1.23456, 1)).toBe('1.2');
    expect(formatCoordinate(1.23456, 0.01)).toBe('1.235');
    expect(formatCoordinate(5, 100)).toBe('5');
  });
});
