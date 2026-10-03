/**
 * Main-thread client for the solver worker.
 *
 * Responsibilities beyond request/response plumbing:
 *
 * * **Latest-wins coalescing.** While a solve is in flight, newer solve or
 *   sample requests replace the queued one instead of piling up, so a drag
 *   gesture produces a bounded number of solves and the field never lags
 *   behind the cursor by more than one frame of work (PRD §9.2).
 * * **Sweep cancellation** that actually stops the worker (PRD §60).
 */
import type {
  BodyGeometry,
  ImportResult,
  PanelDistribution,
  Probe,
  Scene,
  SolverMetadata,
  SweepConfig,
  SweepPoint,
  SweepResult,
  ValidationResult,
  Vec2,
} from '@/domain/types';
import type { SampledResponse, SamplingRequest, SolvedResponse, WorkerRequest, WorkerResponse } from './protocol';

type Resolver<T> = { resolve: (v: T) => void; reject: (e: Error) => void };

/** `Omit` that distributes over a discriminated union instead of collapsing it. */
type DistributiveOmit<T, K extends keyof T> = T extends unknown ? Omit<T, K> : never;
type RequestBody = DistributiveOmit<WorkerRequest, 'id'>;

interface Pending {
  resolve: (v: WorkerResponse) => void;
  reject: (e: Error) => void;
  onProgress?: (p: SweepPoint, done: number, total: number) => void;
}

type QueuedWork =
  | { kind: 'solve'; scene: Scene; sampling: SamplingRequest; waiters: Resolver<SolvedResponse | SampledResponse>[] }
  | { kind: 'sample'; sampling: SamplingRequest; waiters: Resolver<SolvedResponse | SampledResponse>[] };

export class SolverClient {
  private worker: Worker;
  private pending = new Map<number, Pending>();
  private nextId = 1;
  private inFlight = false;
  private queued: QueuedWork | null = null;
  private disposed = false;
  metadata!: SolverMetadata;

  private constructor(worker?: Worker) {
    this.worker =
      worker ??
      new Worker(new URL('./solver.worker.ts', import.meta.url), { type: 'module', name: 'aeroflow-solver' });
    this.worker.onmessage = (ev: MessageEvent<WorkerResponse>) => this.onMessage(ev.data);
    this.worker.onerror = (ev) => {
      const err = new Error(`Solver worker failed: ${ev.message ?? 'unknown error'}`);
      for (const p of this.pending.values()) p.reject(err);
      this.pending.clear();
    };
  }

  /** Spawn the worker and wait for the WASM module to initialise. */
  static async create(worker?: Worker): Promise<SolverClient> {
    const c = new SolverClient(worker);
    const ready = (await c.request({ type: 'init' })) as Extract<WorkerResponse, { type: 'ready' }>;
    c.metadata = ready.metadata;
    return c;
  }

  private onMessage(msg: WorkerResponse): void {
    const p = this.pending.get(msg.id);
    if (!p) return;
    if (msg.type === 'sweepProgress') {
      p.onProgress?.(msg.point, msg.done, msg.total);
      return;
    }
    this.pending.delete(msg.id);
    if (msg.type === 'error') p.reject(new Error(msg.message));
    else p.resolve(msg);
  }

  private request(body: RequestBody, onProgress?: Pending['onProgress']): Promise<WorkerResponse> {
    if (this.disposed) return Promise.reject(new Error('Solver client disposed.'));
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, onProgress });
      this.worker.postMessage({ id, ...body } as WorkerRequest);
    });
  }

  // ───────────── coalesced solve / sample queue ─────────────

  /**
   * Solve `scene` and sample the requested fields. If a solve is already in
   * flight the request is queued, replacing any earlier queued work; all
   * waiters receive the result of the final request.
   */
  solve(scene: Scene, sampling: SamplingRequest): Promise<SolvedResponse | SampledResponse> {
    return new Promise((resolve, reject) => {
      const waiter = { resolve, reject };
      if (this.queued) {
        const w = this.queued.waiters;
        w.push(waiter);
        this.queued = { kind: 'solve', scene, sampling: { ...this.queued.sampling, ...sampling }, waiters: w };
      } else {
        this.queued = { kind: 'solve', scene, sampling, waiters: [waiter] };
      }
      this.pump();
    });
  }

  /** Re-sample fields for the existing solution (pan/zoom/field switch). */
  sample(sampling: SamplingRequest): Promise<SolvedResponse | SampledResponse> {
    return new Promise((resolve, reject) => {
      const waiter = { resolve, reject };
      if (this.queued) {
        this.queued.waiters.push(waiter);
        this.queued.sampling = { ...this.queued.sampling, ...sampling };
      } else {
        this.queued = { kind: 'sample', sampling, waiters: [waiter] };
      }
      this.pump();
    });
  }

  private pump(): void {
    if (this.inFlight || !this.queued) return;
    const work = this.queued;
    this.queued = null;
    this.inFlight = true;
    const req: RequestBody =
      work.kind === 'solve'
        ? { type: 'solve', scene: work.scene, sampling: work.sampling }
        : { type: 'sample', sampling: work.sampling };
    this.request(req)
      .then((res) => {
        for (const w of work.waiters) w.resolve(res as SolvedResponse | SampledResponse);
      })
      .catch((e: Error) => {
        for (const w of work.waiters) w.reject(e);
      })
      .finally(() => {
        this.inFlight = false;
        this.pump();
      });
  }

  get busy(): boolean {
    return this.inFlight || this.queued !== null;
  }

  // ───────────── one-shot requests ─────────────

  async probe(x: number, y: number): Promise<Probe | null> {
    const r = (await this.request({ type: 'probe', x, y })) as Extract<WorkerResponse, { type: 'probe' }>;
    return r.probe;
  }

  sweep(
    config: SweepConfig,
    onProgress?: (point: SweepPoint, done: number, total: number) => void,
  ): { result: Promise<SweepResult>; cancel: () => void } {
    const result = this.request({ type: 'sweep', config }, onProgress).then(
      (r) => (r as Extract<WorkerResponse, { type: 'sweepDone' }>).result,
    );
    const cancel = () => {
      void this.request({ type: 'cancelSweep' }).catch(() => undefined);
    };
    return { result, cancel };
  }

  async importGeometry(text: string, panelCount: number, distribution: PanelDistribution): Promise<ImportResult> {
    const r = (await this.request({ type: 'importGeometry', text, panelCount, distribution })) as Extract<
      WorkerResponse,
      { type: 'imported' }
    >;
    return r.result;
  }

  async validateGeometry(text: string): Promise<ValidationResult> {
    const r = (await this.request({ type: 'validateGeometry', text })) as Extract<WorkerResponse, { type: 'validated' }>;
    return r.result;
  }

  async generateGeometry(spec: BodyGeometry, count: number): Promise<Vec2[]> {
    const r = (await this.request({ type: 'generateGeometry', spec, count })) as Extract<
      WorkerResponse,
      { type: 'generated' }
    >;
    return r.points;
  }

  async pointsToCsv(points: Vec2[]): Promise<string> {
    const r = (await this.request({ type: 'pointsToCsv', points })) as Extract<WorkerResponse, { type: 'csv' }>;
    return r.text;
  }

  dispose(): void {
    this.disposed = true;
    this.worker.terminate();
    for (const p of this.pending.values()) p.reject(new Error('Solver client disposed.'));
    this.pending.clear();
  }
}
