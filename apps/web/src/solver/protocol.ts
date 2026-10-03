/**
 * Message protocol between the main thread and the solver worker.
 *
 * The worker owns the WASM instance; the main thread never touches WASM
 * directly (WASM-002/004). Bulk buffers travel as transferable typed arrays.
 */
import type {
  BodyGeometry,
  FieldRequest,
  ImportResult,
  PanelDistribution,
  Probe,
  ScalarFieldResult,
  Scene,
  Solution,
  SolverMetadata,
  StreamlineRequest,
  StreamlinesResult,
  SweepConfig,
  SweepPoint,
  SweepResult,
  ValidationResult,
  Vec2,
  VectorFieldResult,
} from '@/domain/types';

/** What to sample after a solve, all optional. */
export interface SamplingRequest {
  scalar?: FieldRequest;
  vectors?: FieldRequest;
  streamlines?: StreamlineRequest;
}

export interface SampledData {
  scalar?: ScalarFieldResult;
  vectors?: VectorFieldResult;
  streamlines?: StreamlinesResult;
  elapsedMs: number;
}

export type WorkerRequest =
  | { id: number; type: 'init' }
  | { id: number; type: 'solve'; scene: Scene; sampling: SamplingRequest }
  | { id: number; type: 'sample'; sampling: SamplingRequest }
  | { id: number; type: 'probe'; x: number; y: number }
  | { id: number; type: 'sweep'; config: SweepConfig }
  | { id: number; type: 'cancelSweep' }
  | { id: number; type: 'importGeometry'; text: string; panelCount: number; distribution: PanelDistribution }
  | { id: number; type: 'validateGeometry'; text: string }
  | { id: number; type: 'generateGeometry'; spec: BodyGeometry; count: number }
  | { id: number; type: 'pointsToCsv'; points: Vec2[] };

export type WorkerResponse =
  | { id: number; type: 'ready'; metadata: SolverMetadata }
  | ({ id: number; type: 'solved'; solution: Solution } & SampledData)
  | ({ id: number; type: 'sampled' } & SampledData)
  | { id: number; type: 'probe'; probe: Probe | null }
  | { id: number; type: 'sweepProgress'; point: SweepPoint; done: number; total: number }
  | { id: number; type: 'sweepDone'; result: SweepResult }
  | { id: number; type: 'imported'; result: ImportResult }
  | { id: number; type: 'validated'; result: ValidationResult }
  | { id: number; type: 'generated'; points: Vec2[] }
  | { id: number; type: 'csv'; text: string }
  | { id: number; type: 'ok' }
  | { id: number; type: 'error'; message: string };

export type SolvedResponse = Extract<WorkerResponse, { type: 'solved' }>;
export type SampledResponse = Extract<WorkerResponse, { type: 'sampled' }>;

/** Collect the ArrayBuffers in a response so `postMessage` can transfer them. */
export function transferablesOf(data: SampledData): ArrayBuffer[] {
  const out: ArrayBuffer[] = [];
  const push = (a?: ArrayBufferView) => {
    if (a && a.buffer instanceof ArrayBuffer && !out.includes(a.buffer)) out.push(a.buffer);
  };
  if (data.scalar) {
    push(data.scalar.values);
    push(data.scalar.mask);
  }
  if (data.vectors) {
    push(data.vectors.u);
    push(data.vectors.v);
    push(data.vectors.mask);
  }
  if (data.streamlines) {
    push(data.streamlines.data);
    push(data.streamlines.offsets);
    push(data.streamlines.seedIndex);
  }
  return out;
}
