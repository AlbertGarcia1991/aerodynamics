/** VisualizationState (PRD §35–§36, §17–§18): what to draw and how. */
import { create } from 'zustand';
import type { FieldType, SeedStrategy, Vec2 } from '@/domain/types';

export type ColormapId = 'viridis' | 'magma' | 'turbo' | 'coolwarm' | 'greys' | 'cividis';
export type RangeMode = 'robust' | 'minmax' | 'manual';

export interface StreamlineSettings {
  show: boolean;
  strategy: SeedStrategy;
  /** Separation as a fraction of the view diagonal (evenly spaced). */
  separation: number;
  /** Line count for inflow/grid seeding. */
  count: number;
  colorBySpeed: boolean;
  particles: boolean;
  particleSpeed: number;
  lineWidth: number;
}

export interface VectorSettings {
  show: boolean;
  /** Pixel spacing between arrows. */
  spacingPx: number;
  /** Arrow length multiplier. */
  scale: number;
}

export interface VisualizationStore {
  field: FieldType | 'none';
  colormap: ColormapId;
  rangeMode: RangeMode;
  manualMin: number;
  manualMax: number;
  fieldOpacity: number;
  /** 0 = smooth shading; n > 0 = n contour bands. */
  contourBands: number;
  isolines: boolean;
  streamlines: StreamlineSettings;
  vectors: VectorSettings;
  showGrid: boolean;
  showAxes: boolean;
  showLegend: boolean;
  showSingularityMarkers: boolean;
  /** Force arrows on bodies (resultant + lift/drag) and elements (Lagally). */
  showForces: boolean;
  /** Field grid columns; 'auto' derives from canvas width. */
  resolution: 'auto' | 128 | 192 | 256 | 384;
  /** Manually placed streamline seeds (world). */
  seeds: Vec2[];
  revision: number;

  setField(field: FieldType | 'none'): void;
  setColormap(c: ColormapId): void;
  setRange(mode: RangeMode, min?: number, max?: number): void;
  setFieldOpacity(v: number): void;
  setContourBands(n: number): void;
  setIsolines(on: boolean): void;
  updateStreamlines(patch: Partial<StreamlineSettings>): void;
  updateVectors(patch: Partial<VectorSettings>): void;
  toggle(key: 'showGrid' | 'showAxes' | 'showLegend' | 'showSingularityMarkers' | 'showForces'): void;
  setResolution(r: VisualizationStore['resolution']): void;
  addSeed(p: Vec2): void;
  removeSeed(index: number): void;
  clearSeeds(): void;
}

const bump = (s: { revision: number }) => s.revision + 1;

export const useVisualizationStore = create<VisualizationStore>((set) => ({
  field: 'velocityMagnitude',
  colormap: 'viridis',
  rangeMode: 'robust',
  manualMin: 0,
  manualMax: 1,
  fieldOpacity: 0.9,
  contourBands: 0,
  isolines: false,
  streamlines: {
    show: true,
    strategy: 'evenlySpaced',
    separation: 0.035,
    count: 32,
    colorBySpeed: false,
    particles: true,
    particleSpeed: 1,
    lineWidth: 1.2,
  },
  vectors: { show: false, spacingPx: 44, scale: 1 },
  showGrid: true,
  showAxes: true,
  showLegend: true,
  showSingularityMarkers: true,
  showForces: true,
  resolution: 'auto',
  seeds: [],
  revision: 0,

  setField: (field) => set((s) => ({ field, revision: bump(s) })),
  setColormap: (colormap) => set((s) => ({ colormap, revision: bump(s) })),
  setRange: (rangeMode, min, max) =>
    set((s) => ({
      rangeMode,
      manualMin: min ?? s.manualMin,
      manualMax: max ?? s.manualMax,
      revision: bump(s),
    })),
  setFieldOpacity: (fieldOpacity) => set((s) => ({ fieldOpacity, revision: bump(s) })),
  setContourBands: (contourBands) => set((s) => ({ contourBands, revision: bump(s) })),
  setIsolines: (isolines) => set((s) => ({ isolines, revision: bump(s) })),
  updateStreamlines: (patch) => set((s) => ({ streamlines: { ...s.streamlines, ...patch }, revision: bump(s) })),
  updateVectors: (patch) => set((s) => ({ vectors: { ...s.vectors, ...patch }, revision: bump(s) })),
  toggle: (key) => set((s) => ({ [key]: !s[key], revision: bump(s) }) as Partial<VisualizationStore>),
  setResolution: (resolution) => set((s) => ({ resolution, revision: bump(s) })),
  addSeed: (p) => set((s) => ({ seeds: [...s.seeds, p], revision: bump(s) })),
  removeSeed: (i) => set((s) => ({ seeds: s.seeds.filter((_, k) => k !== i), revision: bump(s) })),
  clearSeeds: () => set((s) => ({ seeds: [], revision: bump(s) })),
}));
