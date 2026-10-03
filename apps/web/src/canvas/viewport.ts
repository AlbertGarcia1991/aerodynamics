/**
 * Pure viewport mathematics: world ↔ screen, visible bounds, grid spacing.
 *
 * The world frame is +x right, +y up (PRD §14.1); the screen frame is +y down,
 * so the transform flips y. `scale` is pixels per metre.
 */
import type { Bounds, Vec2 } from '@/domain/types';

export interface Viewport {
  /** World point shown at the canvas centre. */
  center: Vec2;
  /** Pixels per metre. */
  scale: number;
  /** Canvas size in CSS pixels. */
  width: number;
  height: number;
}

export const MIN_SCALE = 1e-3;
export const MAX_SCALE = 1e6;

export function worldToScreen(p: Vec2, vp: Viewport): Vec2 {
  return {
    x: vp.width / 2 + (p.x - vp.center.x) * vp.scale,
    y: vp.height / 2 - (p.y - vp.center.y) * vp.scale,
  };
}

export function screenToWorld(s: Vec2, vp: Viewport): Vec2 {
  return {
    x: vp.center.x + (s.x - vp.width / 2) / vp.scale,
    y: vp.center.y - (s.y - vp.height / 2) / vp.scale,
  };
}

/** World-space rectangle currently visible. */
export function visibleBounds(vp: Viewport): Bounds {
  const hw = vp.width / (2 * vp.scale);
  const hh = vp.height / (2 * vp.scale);
  return {
    min: { x: vp.center.x - hw, y: vp.center.y - hh },
    max: { x: vp.center.x + hw, y: vp.center.y + hh },
  };
}

export function expandBounds(b: Bounds, fraction: number): Bounds {
  const dx = (b.max.x - b.min.x) * fraction;
  const dy = (b.max.y - b.min.y) * fraction;
  return { min: { x: b.min.x - dx, y: b.min.y - dy }, max: { x: b.max.x + dx, y: b.max.y + dy } };
}

/** Zoom about a fixed screen point so the world point under the cursor stays put. */
export function zoomAt(vp: Viewport, screenPt: Vec2, factor: number): Viewport {
  const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, vp.scale * factor));
  const before = screenToWorld(screenPt, vp);
  const next = { ...vp, scale };
  const after = screenToWorld(screenPt, next);
  return { ...next, center: { x: vp.center.x + (before.x - after.x), y: vp.center.y + (before.y - after.y) } };
}

export function panBy(vp: Viewport, dxPx: number, dyPx: number): Viewport {
  return { ...vp, center: { x: vp.center.x - dxPx / vp.scale, y: vp.center.y + dyPx / vp.scale } };
}

/** Fit `bounds` into the canvas with `padding` as a fraction of the canvas size. */
export function fitBounds(vp: Viewport, bounds: Bounds, padding = 0.12): Viewport {
  const w = Math.max(bounds.max.x - bounds.min.x, 1e-9);
  const h = Math.max(bounds.max.y - bounds.min.y, 1e-9);
  const usableW = vp.width * (1 - 2 * padding);
  const usableH = vp.height * (1 - 2 * padding);
  const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, Math.min(usableW / w, usableH / h)));
  return {
    ...vp,
    scale,
    center: { x: (bounds.min.x + bounds.max.x) / 2, y: (bounds.min.y + bounds.max.y) / 2 },
  };
}

/**
 * A "nice" grid step (1, 2, 5 × 10ⁿ) giving roughly `targetPx` pixels between
 * lines. Returns the major step; minor lines use `step / 5`.
 */
export function niceStep(scale: number, targetPx = 90): number {
  const raw = targetPx / scale; // metres per target spacing
  const pow = Math.pow(10, Math.floor(Math.log10(raw)));
  const m = raw / pow;
  const nice = m < 1.5 ? 1 : m < 3.5 ? 2 : m < 7.5 ? 5 : 10;
  return nice * pow;
}

/** Format a world coordinate with precision appropriate to the grid step. */
export function formatCoordinate(value: number, step: number): string {
  const decimals = Math.max(0, Math.min(6, -Math.floor(Math.log10(step)) + 1));
  return value.toFixed(decimals);
}
