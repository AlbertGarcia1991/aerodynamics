/** ViewportState (PRD §49, §34): pan, zoom, fit, reset. */
import { create } from 'zustand';
import type { Bounds, Vec2 } from '@/domain/types';
import { fitBounds, panBy, zoomAt, type Viewport } from '@/canvas/viewport';

export interface ViewportStore extends Viewport {
  /** Incremented on every change, for cheap change detection. */
  revision: number;
  setSize(width: number, height: number): void;
  pan(dxPx: number, dyPx: number): void;
  zoom(screenPt: Vec2, factor: number): void;
  fit(bounds: Bounds, padding?: number): void;
  setView(center: Vec2, scale: number): void;
  reset(): void;
}

const DEFAULT_VIEW: Viewport = { center: { x: 0.5, y: 0 }, scale: 160, width: 800, height: 600 };

export const useViewportStore = create<ViewportStore>((set, get) => ({
  ...DEFAULT_VIEW,
  revision: 0,
  setSize(width, height) {
    if (width === get().width && height === get().height) return;
    set((s) => ({ width, height, revision: s.revision + 1 }));
  },
  pan(dx, dy) {
    set((s) => ({ ...panBy(s, dx, dy), revision: s.revision + 1 }));
  },
  zoom(pt, factor) {
    set((s) => ({ ...zoomAt(s, pt, factor), revision: s.revision + 1 }));
  },
  fit(bounds, padding) {
    set((s) => ({ ...fitBounds(s, bounds, padding), revision: s.revision + 1 }));
  },
  setView(center, scale) {
    set((s) => ({ center, scale, revision: s.revision + 1 }));
  },
  reset() {
    set((s) => ({ center: DEFAULT_VIEW.center, scale: DEFAULT_VIEW.scale, revision: s.revision + 1 }));
  },
}));
