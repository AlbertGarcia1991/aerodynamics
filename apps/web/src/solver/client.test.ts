/**
 * The client is tested against a fake worker so that coalescing and
 * cancellation semantics are pinned down without WASM.
 */
import { describe, expect, it, vi } from 'vitest';
import { SolverClient } from './client';
import type { WorkerRequest, WorkerResponse } from './protocol';
import { createScene } from '@/domain/scene';

type DistributiveOmit<T, K extends keyof T> = T extends unknown ? Omit<T, K> : never;
type Reply = DistributiveOmit<WorkerResponse, 'id'>;

class FakeWorker {
  onmessage: ((ev: MessageEvent<WorkerResponse>) => void) | null = null;
  onerror: ((ev: ErrorEvent) => void) | null = null;
  received: WorkerRequest[] = [];
  /** Queue of responders; each decides when to reply. */
  private pending: { msg: WorkerRequest; reply: (r: Reply) => void }[] = [];
  auto = true;

  postMessage(msg: WorkerRequest) {
    this.received.push(msg);
    const reply = (r: Reply) => {
      this.onmessage?.({ data: { ...r, id: msg.id } as WorkerResponse } as MessageEvent<WorkerResponse>);
    };
    if (msg.type === 'init') {
      reply({ type: 'ready', metadata: { version: 't', descriptors: [], helpTopics: [], fieldTypes: [], assumptions: [] } });
      return;
    }
    if (this.auto) this.respond(msg, reply);
    else this.pending.push({ msg, reply });
  }

  respond(msg: WorkerRequest, reply: (r: Reply) => void) {
    switch (msg.type) {
      case 'solve':
        reply({ type: 'solved', solution: { status: 'success', bodies: [], total: { fx: 0, fy: 0, lift: 0, drag: 0, moment: 0, circulation: 0 }, diagnostics: { panel: null, timings: { prepareMs: 0, assembleMs: 0, solveMs: 0, forcesMs: 0, totalMs: 1 }, systemReused: false, elementCount: msg.scene.elements.length, bodyCount: 0, totalCirculation: 0, netOutflow: 0 }, warnings: [], error: null, assumptions: [] }, elapsedMs: 1 });
        break;
      case 'sample':
        reply({ type: 'sampled', elapsedMs: 1 });
        break;
      case 'probe':
        reply({ type: 'probe', probe: null });
        break;
      case 'sweep': {
        const total = msg.config.steps;
        for (let i = 0; i < total; i++) {
          reply({ type: 'sweepProgress', point: { index: i, value: i, status: 'success', bodies: [], totalLift: 0, totalDrag: 0, error: null }, done: i + 1, total });
        }
        reply({ type: 'sweepDone', result: { config: msg.config, points: [], completed: true } });
        break;
      }
      case 'cancelSweep':
        reply({ type: 'ok' });
        break;
      default:
        reply({ type: 'error', message: `unhandled ${msg.type}` });
    }
  }

  flush() {
    const items = this.pending.splice(0);
    for (const it of items) this.respond(it.msg, it.reply);
  }

  terminate() {}
}

/** The client pumps its queue in a `.finally()` after waiters resolve; a macrotask tick lets that run. */
const tick = () => new Promise<void>((r) => setTimeout(r, 0));

async function make() {
  const w = new FakeWorker();
  const client = await SolverClient.create(w as unknown as Worker);
  return { w, client };
}

describe('SolverClient', () => {
  it('initialises and exposes metadata', async () => {
    const { client } = await make();
    expect(client.metadata.version).toBe('t');
  });

  it('coalesces solves issued while one is in flight: latest wins, all waiters resolve', async () => {
    const { w, client } = await make();
    w.auto = false;
    const s1 = createScene('one');
    const s2 = createScene('two');
    const s3 = createScene('three');
    const p1 = client.solve(s1, {});
    const p2 = client.solve(s2, {});
    const p3 = client.solve(s3, {});
    expect(w.received.filter((m) => m.type === 'solve')).toHaveLength(1); // only the first was sent
    w.flush(); // completes s1 → pumps the queue, which now holds s3 only
    await p1;
    await tick();
    expect(w.received.filter((m) => m.type === 'solve')).toHaveLength(2);
    const sent = w.received.filter((m) => m.type === 'solve') as Extract<WorkerRequest, { type: 'solve' }>[];
    expect(sent[1].scene.name).toBe('three'); // s2 was superseded, never sent
    w.flush();
    const [r2, r3] = await Promise.all([p2, p3]);
    expect(r2).toBe(r3); // superseded waiters receive the final result
    await tick(); // `inFlight` clears in the same deferred `.finally()` that pumps the queue
    expect(client.busy).toBe(false);
  });

  it('merges a sample request into a queued solve rather than queueing separately', async () => {
    const { w, client } = await make();
    w.auto = false;
    void client.solve(createScene('a'), {});
    void client.solve(createScene('b'), { scalar: { field: 'cp', minX: 0, minY: 0, maxX: 1, maxY: 1, nx: 4, ny: 4, render: true } });
    void client.sample({ streamlines: { minX: 0, minY: 0, maxX: 1, maxY: 1 } });
    w.flush();
    await tick();
    w.flush();
    await tick();
    const sent = w.received.filter((m) => m.type === 'solve' || m.type === 'sample');
    expect(sent).toHaveLength(2);
    const second = sent[1] as Extract<WorkerRequest, { type: 'solve' }>;
    expect(second.type).toBe('solve');
    expect(second.sampling.scalar?.field).toBe('cp');
    expect(second.sampling.streamlines).toBeDefined();
  });

  it('reports sweep progress and completion', async () => {
    const { client } = await make();
    const seen: number[] = [];
    const run = client.sweep({ parameter: 'angleOfAttack', start: 0, end: 1, steps: 3 }, (_, done) => seen.push(done));
    const result = await run.result;
    expect(seen).toEqual([1, 2, 3]);
    expect(result.completed).toBe(true);
  });

  it('rejects pending requests when the worker errors', async () => {
    const { w, client } = await make();
    w.auto = false;
    const p = client.probe(0, 0);
    w.onerror?.({ message: 'boom' } as ErrorEvent);
    await expect(p).rejects.toThrow(/boom/);
  });

  it('surfaces worker-side errors as rejected promises', async () => {
    const { client } = await make();
    await expect(client.pointsToCsv([])).rejects.toThrow(/unhandled/);
  });

  it('refuses work after dispose', async () => {
    const { client } = await make();
    const spy = vi.fn();
    client.dispose();
    await client.probe(0, 0).catch(spy);
    expect(spy).toHaveBeenCalled();
  });
});
