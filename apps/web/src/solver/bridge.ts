/**
 * The bridge between state and solver (PRD §9.2 "dynamic updates").
 *
 * It watches the scene, the viewport and the visualisation settings and turns
 * changes into the minimum solver work:
 *
 * | What changed              | Work                          |
 * | ------------------------- | ----------------------------- |
 * | scene (objects, flow)     | solve + sample                |
 * | viewport (pan/zoom)       | sample only                   |
 * | visualisation settings    | sample only                   |
 *
 * Every change first produces a **low-resolution preview** so the picture
 * follows the cursor during a drag; a **full-resolution pass** runs once input
 * has been idle for a moment. The client coalesces requests, so a drag never
 * queues more than one solve.
 */
import type { Bounds, FieldRequest, StreamlineRequest } from '@/domain/types';
import { useSimulationStore } from '@/state/simulationStore';
import { useSolverStore } from '@/state/solverStore';
import { useUIStore } from '@/state/uiStore';
import { useViewportStore } from '@/state/viewportStore';
import { useVisualizationStore } from '@/state/visualizationStore';
import { expandBounds, visibleBounds, type Viewport } from '@/canvas/viewport';
import type { SolverClient } from './client';
import type { SamplingRequest } from './protocol';

const PREVIEW_NX = 96;
const IDLE_MS = 220;
const FIELD_MARGIN = 0.06;

export function buildSampling(
  vp: Viewport,
  viz: ReturnType<typeof useVisualizationStore.getState>,
  preview: boolean,
): { sampling: SamplingRequest; bounds: Bounds } {
  const bounds = expandBounds(visibleBounds(vp), FIELD_MARGIN);
  const aspect = vp.height / Math.max(vp.width, 1);
  const autoNx = Math.min(320, Math.max(160, Math.round(vp.width / 3.5)));
  const nx = preview ? PREVIEW_NX : viz.resolution === 'auto' ? autoNx : viz.resolution;
  const ny = Math.max(8, Math.round(nx * aspect));
  const base = { minX: bounds.min.x, minY: bounds.min.y, maxX: bounds.max.x, maxY: bounds.max.y };

  const sampling: SamplingRequest = {};
  if (viz.field !== 'none') {
    const scalar: FieldRequest = { field: viz.field, ...base, nx, ny, render: true };
    sampling.scalar = scalar;
  }
  if (viz.vectors.show) {
    const vnx = Math.max(4, Math.round(vp.width / viz.vectors.spacingPx));
    const vny = Math.max(4, Math.round(vp.height / viz.vectors.spacingPx));
    sampling.vectors = { field: 'velocityMagnitude', ...base, nx: vnx, ny: vny, render: true };
  }
  if (viz.streamlines.show) {
    const view = visibleBounds(vp);
    const req: StreamlineRequest = {
      minX: view.min.x,
      minY: view.min.y,
      maxX: view.max.x,
      maxY: view.max.y,
      seeding: {
        strategy: viz.streamlines.strategy,
        count: viz.streamlines.count,
        separation: viz.streamlines.separation,
        stopRatio: 0.55,
        maxLines: preview ? 120 : 400,
      },
      seeds: viz.seeds.flatMap((p) => [p.x, p.y]),
      maxSteps: preview ? 1500 : 4000,
    };
    sampling.streamlines = req;
  }
  return { sampling, bounds };
}

export function startSolverBridge(client: SolverClient): () => void {
  let sceneRev = -1;
  let vizRev = -1;
  let viewRev = -1;
  let idleTimer: ReturnType<typeof setTimeout> | null = null;
  let disposed = false;

  const run = async (solve: boolean, preview: boolean) => {
    if (disposed) return;
    const scene = useSimulationStore.getState().scene;
    const vp = useViewportStore.getState();
    const viz = useVisualizationStore.getState();
    const solver = useSolverStore.getState();
    const { sampling, bounds } = buildSampling(vp, viz, preview);
    if (solve) solver.setStatus('running');
    const t0 = performance.now();
    try {
      const res = solve ? await client.solve(scene, sampling) : await client.sample(sampling);
      if (disposed) return;
      const store = useSolverStore.getState();
      if (res.type === 'solved') {
        store.setSolution(res.solution, performance.now() - t0);
        if (res.solution.status === 'error') {
          store.clearSampled();
          return;
        }
      } else if (!store.solution) {
        return;
      }
      // Results carry their own bounds: with request coalescing, the resolved
      // data may belong to a later viewport than this call's `bounds`.
      void bounds;
      store.setSampled({
        scalar: res.scalar,
        scalarGrid: res.scalar ? { bounds: res.scalar.bounds, nx: res.scalar.nx, ny: res.scalar.ny } : undefined,
        vectors: res.vectors,
        vectorGrid: res.vectors ? { bounds: res.vectors.bounds, nx: res.vectors.nx, ny: res.vectors.ny } : undefined,
        streamlines: res.streamlines,
        preview,
        sampleMs: res.elapsedMs,
      });
    } catch (e) {
      if (disposed) return;
      useSolverStore.getState().setStatus('error');
      useUIStore.getState().toast(`Solver error: ${(e as Error).message}`, 'error');
    }
  };

  const schedule = (solve: boolean) => {
    void run(solve, true);
    if (idleTimer) clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      idleTimer = null;
      void run(false, false);
    }, IDLE_MS);
  };

  const check = () => {
    const s = useSimulationStore.getState().revision;
    const z = useVisualizationStore.getState().revision;
    const v = useViewportStore.getState().revision;
    if (s !== sceneRev) {
      sceneRev = s;
      vizRev = z;
      viewRev = v;
      schedule(true);
    } else if (z !== vizRev || v !== viewRev) {
      vizRev = z;
      viewRev = v;
      schedule(false);
    }
  };

  const unsubs = [
    useSimulationStore.subscribe(check),
    useVisualizationStore.subscribe(check),
    useViewportStore.subscribe(check),
  ];
  // Initial solve.
  check();

  return () => {
    disposed = true;
    if (idleTimer) clearTimeout(idleTimer);
    unsubs.forEach((u) => u());
  };
}

/** Throttled hover probe: at most one request in flight, latest position wins. */
export function createProbeThrottle(client: SolverClient): (x: number, y: number) => void {
  let busy = false;
  let pending: { x: number; y: number } | null = null;
  const fire = async (x: number, y: number) => {
    busy = true;
    try {
      const p = await client.probe(x, y);
      useSolverStore.getState().setProbe(p);
    } catch {
      /* ignore transient errors */
    } finally {
      busy = false;
      if (pending) {
        const n = pending;
        pending = null;
        void fire(n.x, n.y);
      }
    }
  };
  return (x, y) => {
    if (busy) pending = { x, y };
    else void fire(x, y);
  };
}
