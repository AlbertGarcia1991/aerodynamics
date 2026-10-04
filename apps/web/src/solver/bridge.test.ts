/**
 * PRD2 §89 (stale results) and §31 (preview then accurate solve), against a
 * fake client whose replies the test releases by hand.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { startSolverBridge } from './bridge';
import type { SolverClient, SolveOptions } from './client';
import { useSimulationStore } from '@/state/simulationStore';
import { useSolverStore } from '@/state/solverStore';
import { createBody, createScene } from '@/domain/scene';
import { PREVIEW_PANELS, templateGeometry } from '@/domain/bezier';
import type { Scene, Solution } from '@/domain/types';

const solution = (lift: number): Solution =>
  ({
    status: 'success',
    bodies: [],
    elements: [],
    total: { fx: 0, fy: 0, lift, drag: 0, moment: 0, circulation: 0 },
    diagnostics: { panel: null, timings: { prepareMs: 0, assembleMs: 0, solveMs: 0, forcesMs: 0, totalMs: 1 }, systemReused: false, elementCount: 0, bodyCount: 0, totalCirculation: 0, netOutflow: 0 },
    warnings: [],
    error: null,
    assumptions: [],
  }) as Solution;

interface Call {
  scene: Scene;
  opts: SolveOptions;
  finish: (lift: number) => void;
}

function fakeClient() {
  const calls: Call[] = [];
  const client = {
    solve: vi.fn((scene: Scene, _s: unknown, opts: SolveOptions = {}) =>
      new Promise((resolve) => {
        calls.push({ scene, opts, finish: (lift) => resolve({ type: 'solved', solution: solution(lift), revision: opts.revision ?? null, elapsedMs: 1 }) });
      }),
    ),
    sample: vi.fn(async () => ({ type: 'sampled', revision: null, elapsedMs: 1 })),
  } as unknown as SolverClient;
  return { client, calls };
}

beforeEach(() => {
  useSimulationStore.getState().loadScene(createScene());
  useSolverStore.setState({ solution: null, solvedRevision: -1, status: 'idle' });
});
afterEach(() => vi.useRealTimers());

describe('solver bridge', () => {
  it('rejects a result from an older revision that finishes after a newer one', async () => {
    const { client, calls } = fakeClient();
    const stop = startSolverBridge(client);
    const rev10 = useSimulationStore.getState().revision;
    useSimulationStore.getState().setConditions({ velocity: 12 });
    const rev11 = useSimulationStore.getState().revision;
    expect(rev11).toBeGreaterThan(rev10);
    expect(calls.map((c) => c.opts.revision)).toEqual([rev10, rev11]);

    calls[1].finish(11);
    await vi.waitFor(() => expect(useSolverStore.getState().solvedRevision).toBe(rev11));
    calls[0].finish(10); // revision 10 arrives last
    await new Promise((r) => setTimeout(r, 10));

    expect(useSolverStore.getState().solvedRevision).toBe(rev11);
    expect(useSolverStore.getState().solution?.total.lift).toBe(11);
    stop();
  });

  it('marks the shown solution stale until its revision catches up with the scene', async () => {
    const { client, calls } = fakeClient();
    const stop = startSolverBridge(client);
    useSimulationStore.getState().setConditions({ angle: 0.1 });
    const current = useSimulationStore.getState().revision;
    expect(useSolverStore.getState().solvedRevision).not.toBe(current); // nothing solved for it yet
    calls[calls.length - 1].finish(1);
    await vi.waitFor(() => expect(useSolverStore.getState().solvedRevision).toBe(current));
    stop();
  });

  it('while dragging a Bézier edit, solves with reduced panels first, then at full count once idle', async () => {
    vi.useFakeTimers();
    const { client, calls } = fakeClient();
    const scene = createScene();
    const body = createBody(scene, templateGeometry('naca2412'));
    body.panels = { count: 256, distribution: 'cosine' };
    scene.bodies.push(body);
    useSimulationStore.getState().loadScene(scene);
    useSimulationStore.getState().beginTransaction(); // a drag is in flight

    const stop = startSolverBridge(client);
    expect(calls[0].opts.previewPanels).toBe(PREVIEW_PANELS);
    calls[0].finish(1);
    await vi.advanceTimersByTimeAsync(400);
    expect(calls).toHaveLength(2);
    expect(calls[1].opts.previewPanels).toBeUndefined();
    stop();
  });

  it('keeps full accuracy for edits that are not a drag, so the influence matrix is reused', async () => {
    vi.useFakeTimers();
    const { client, calls } = fakeClient();
    const scene = createScene();
    const body = createBody(scene, templateGeometry('naca2412'));
    body.panels = { count: 256, distribution: 'cosine' };
    scene.bodies.push(body);
    useSimulationStore.getState().loadScene(scene);
    const stop = startSolverBridge(client);
    expect(calls[0].opts.previewPanels).toBeUndefined();
    calls[0].finish(1);
    await vi.advanceTimersByTimeAsync(400);
    expect(calls).toHaveLength(1);
    stop();
  });

  it('does not re-solve after a preview when panels are already at or below the preview count', async () => {
    vi.useFakeTimers();
    const { client, calls } = fakeClient();
    const scene = createScene();
    const body = createBody(scene, templateGeometry('circle'));
    body.panels = { count: 48, distribution: 'auto' };
    scene.bodies.push(body);
    useSimulationStore.getState().loadScene(scene);
    useSimulationStore.getState().beginTransaction();
    const stop = startSolverBridge(client);
    calls[0].finish(1);
    await vi.advanceTimersByTimeAsync(400);
    expect(calls).toHaveLength(1);
    stop();
  });
});
