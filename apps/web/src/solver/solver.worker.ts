/**
 * Solver worker: the only place the WASM module is instantiated.
 *
 * Messages are handled one at a time. A sweep is the only long operation; it
 * runs in ~12 ms chunks and yields to the event loop between chunks so that a
 * `cancelSweep` message is honoured promptly (PRD §60, WASM-006).
 */
import init, {
  AeroflowSolver,
  assumptions,
  elementDescriptors,
  fieldTypes,
  generateGeometry,
  helpTopics,
  importGeometry,
  pointsToCsv,
  validateGeometry,
  version,
} from '../wasm/pkg/aeroflow.js';
import type { Bounds, FieldRequest, SolverMetadata, SweepResult } from '@/domain/types';
import { transferablesOf, type SampledData, type SamplingRequest, type WorkerRequest, type WorkerResponse } from './protocol';

const ctx = self as unknown as {
  postMessage(message: WorkerResponse, transfer?: Transferable[]): void;
  onmessage: ((ev: MessageEvent<WorkerRequest>) => void) | null;
};

let solver: AeroflowSolver | null = null;
let readyPromise: Promise<void> | null = null;
let sweepToken = 0;

function post(message: WorkerResponse, transfer: Transferable[] = []): void {
  ctx.postMessage(message, transfer);
}

function fail(id: number, e: unknown): void {
  const message = e instanceof Error ? e.message : typeof e === 'string' ? e : JSON.stringify(e);
  post({ id, type: 'error', message });
}

async function ensureReady(): Promise<AeroflowSolver> {
  if (!readyPromise) {
    readyPromise = init().then(() => {
      solver = new AeroflowSolver();
    });
  }
  await readyPromise;
  return solver!;
}

function metadata(): SolverMetadata {
  return {
    version: version(),
    descriptors: elementDescriptors(),
    helpTopics: helpTopics(),
    fieldTypes: fieldTypes(),
    assumptions: assumptions(),
  };
}

const boundsOf = (r: FieldRequest): Bounds => ({ min: { x: r.minX, y: r.minY }, max: { x: r.maxX, y: r.maxY } });

function sample(s: AeroflowSolver, sampling: SamplingRequest): SampledData {
  const t0 = performance.now();
  const out: SampledData = { elapsedMs: 0 };
  if (sampling.scalar) {
    out.scalar = { ...s.sampleScalar(JSON.stringify(sampling.scalar)), bounds: boundsOf(sampling.scalar) };
  }
  if (sampling.vectors) {
    out.vectors = { ...s.sampleVectors(JSON.stringify(sampling.vectors)), bounds: boundsOf(sampling.vectors) };
  }
  if (sampling.streamlines) out.streamlines = s.streamlines(JSON.stringify(sampling.streamlines));
  out.elapsedMs = performance.now() - t0;
  return out;
}

async function runSweep(id: number, s: AeroflowSolver, configJson: string): Promise<void> {
  const total = s.beginSweep(configJson);
  const token = ++sweepToken;
  let done = 0;
  for (;;) {
    const t0 = performance.now();
    while (performance.now() - t0 < 12) {
      if (sweepToken !== token) return; // cancelled: the cancel handler already replied
      const point = s.sweepStep();
      if (point === null) {
        const result = s.sweepResult() as SweepResult;
        s.cancelSweep();
        post({ id, type: 'sweepDone', result });
        return;
      }
      done += 1;
      post({ id, type: 'sweepProgress', point, done, total });
    }
    // Yield so queued messages (notably cancelSweep) get processed.
    await new Promise<void>((r) => setTimeout(r, 0));
  }
}

let activeSweepId: number | null = null;

ctx.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  const msg = ev.data;
  const { id } = msg;
  try {
    if (msg.type === 'init') {
      await ensureReady();
      post({ id, type: 'ready', metadata: metadata() });
      return;
    }
    const s = await ensureReady();
    switch (msg.type) {
      case 'solve': {
        s.setScene(JSON.stringify(msg.scene));
        const solution = s.solve();
        const sampled = solution.status === 'error' ? { elapsedMs: 0 } : sample(s, msg.sampling);
        post({ id, type: 'solved', solution, ...sampled }, transferablesOf(sampled));
        break;
      }
      case 'sample': {
        const sampled = sample(s, msg.sampling);
        post({ id, type: 'sampled', ...sampled }, transferablesOf(sampled));
        break;
      }
      case 'probe':
        post({ id, type: 'probe', probe: s.probe(msg.x, msg.y) });
        break;
      case 'sweep':
        if (activeSweepId !== null) {
          // Supersede any sweep still running.
          sweepToken += 1;
          s.cancelSweep();
        }
        activeSweepId = id;
        await runSweep(id, s, JSON.stringify(msg.config));
        if (activeSweepId === id) activeSweepId = null;
        break;
      case 'cancelSweep': {
        if (activeSweepId !== null) {
          const partial = (s.sweepResult() as SweepResult | null) ?? { config: null as never, points: [], completed: false };
          sweepToken += 1;
          s.cancelSweep();
          post({ id: activeSweepId, type: 'sweepDone', result: { ...partial, completed: false } });
          activeSweepId = null;
        }
        post({ id, type: 'ok' });
        break;
      }
      case 'importGeometry':
        post({ id, type: 'imported', result: importGeometry(msg.text, msg.panelCount, msg.distribution) });
        break;
      case 'validateGeometry':
        post({ id, type: 'validated', result: validateGeometry(msg.text) });
        break;
      case 'generateGeometry':
        post({ id, type: 'generated', points: generateGeometry(JSON.stringify(msg.spec), msg.count) });
        break;
      case 'pointsToCsv':
        post({ id, type: 'csv', text: pointsToCsv(JSON.stringify(msg.points)) });
        break;
    }
  } catch (e) {
    fail(id, e);
  }
};
