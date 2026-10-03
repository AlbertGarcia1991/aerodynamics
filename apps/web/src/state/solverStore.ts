/** SolverState (PRD §49, §72): status, latest solution, sampled fields, sweeps. */
import { create } from 'zustand';
import type {
  Bounds,
  Probe,
  ScalarFieldResult,
  Solution,
  SolverMetadata,
  StreamlinesResult,
  SweepConfig,
  SweepPoint,
  SweepResult,
  VectorFieldResult,
} from '@/domain/types';

export type SolverStatus = 'idle' | 'queued' | 'running' | 'success' | 'warning' | 'error' | 'cancelled';

export interface SampledGrid {
  bounds: Bounds;
  nx: number;
  ny: number;
}

export interface SweepState {
  running: boolean;
  config: SweepConfig | null;
  points: SweepPoint[];
  total: number;
  result: SweepResult | null;
  error: string | null;
}

export interface SolverStore {
  ready: boolean;
  initError: string | null;
  metadata: SolverMetadata | null;
  status: SolverStatus;
  solution: Solution | null;
  scalar: ScalarFieldResult | null;
  scalarGrid: SampledGrid | null;
  vectors: VectorFieldResult | null;
  vectorGrid: SampledGrid | null;
  streamlines: StreamlinesResult | null;
  /** True while a low-resolution preview is shown and a full-resolution pass is pending. */
  preview: boolean;
  lastSolveMs: number;
  lastSampleMs: number;
  lastRoundTripMs: number;
  solveCount: number;
  probe: Probe | null;
  sweep: SweepState;

  setReady(metadata: SolverMetadata): void;
  setInitError(message: string): void;
  setStatus(status: SolverStatus): void;
  setSolution(solution: Solution, roundTripMs: number): void;
  setSampled(data: {
    scalar?: ScalarFieldResult;
    scalarGrid?: SampledGrid;
    vectors?: VectorFieldResult;
    vectorGrid?: SampledGrid;
    streamlines?: StreamlinesResult;
    preview: boolean;
    sampleMs: number;
  }): void;
  clearSampled(): void;
  setProbe(p: Probe | null): void;
  sweepStart(config: SweepConfig, total: number): void;
  sweepProgress(point: SweepPoint): void;
  sweepDone(result: SweepResult): void;
  sweepError(message: string): void;
  sweepReset(): void;
}

const EMPTY_SWEEP: SweepState = { running: false, config: null, points: [], total: 0, result: null, error: null };

export const useSolverStore = create<SolverStore>((set) => ({
  ready: false,
  initError: null,
  metadata: null,
  status: 'idle',
  solution: null,
  scalar: null,
  scalarGrid: null,
  vectors: null,
  vectorGrid: null,
  streamlines: null,
  preview: false,
  lastSolveMs: 0,
  lastSampleMs: 0,
  lastRoundTripMs: 0,
  solveCount: 0,
  probe: null,
  sweep: EMPTY_SWEEP,

  setReady: (metadata) => set({ ready: true, metadata, initError: null }),
  setInitError: (initError) => set({ initError, ready: false }),
  setStatus: (status) => set({ status }),
  setSolution: (solution, roundTripMs) =>
    set((s) => ({
      solution,
      status: solution.status,
      lastSolveMs: solution.diagnostics.timings.totalMs,
      lastRoundTripMs: roundTripMs,
      solveCount: s.solveCount + 1,
    })),
  setSampled: ({ scalar, scalarGrid, vectors, vectorGrid, streamlines, preview, sampleMs }) =>
    set((s) => ({
      scalar: scalar ?? (scalarGrid ? null : s.scalar),
      scalarGrid: scalarGrid ?? (scalar ? s.scalarGrid : null),
      vectors: vectors ?? null,
      vectorGrid: vectorGrid ?? null,
      streamlines: streamlines ?? null,
      preview,
      lastSampleMs: sampleMs,
    })),
  clearSampled: () => set({ scalar: null, scalarGrid: null, vectors: null, vectorGrid: null, streamlines: null }),
  setProbe: (probe) => set({ probe }),
  sweepStart: (config, total) => set({ sweep: { running: true, config, points: [], total, result: null, error: null } }),
  sweepProgress: (point) => set((s) => ({ sweep: { ...s.sweep, points: [...s.sweep.points, point] } })),
  sweepDone: (result) => set((s) => ({ sweep: { ...s.sweep, running: false, result, points: result.points } })),
  sweepError: (error) => set((s) => ({ sweep: { ...s.sweep, running: false, error } })),
  sweepReset: () => set({ sweep: EMPTY_SWEEP }),
}));
