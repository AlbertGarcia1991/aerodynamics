/**
 * In-browser performance benchmark (PRD §65). Not a pass/fail gate beyond loose
 * sanity bounds — it records the numbers to `test-results/perf.json` for
 * docs/PERFORMANCE.md. Run: `npx playwright test e2e/perf.spec.ts`.
 */
import { expect, test } from '@playwright/test';
import { writeFileSync, mkdirSync } from 'node:fs';
import { loadExample, openApp } from './helpers';

type Store = {
  getState(): {
    solution: { diagnostics: { timings: { assembleMs: number; solveMs: number; totalMs: number }; systemReused: boolean; panel: { panelCount: number } | null } } | null;
    lastSampleMs: number;
    lastRoundTripMs: number;
    preview: boolean;
    solveCount: number;
  };
};

test('benchmark solve, sampling and frame times across panel counts', async ({ page }) => {
  test.setTimeout(180_000);
  await openApp(page);
  await loadExample(page, 'NACA 0012');
  await page.getByRole('button', { name: /^NACA 0012/ }).click();
  const rows: Record<string, number | boolean>[] = [];

  for (const n of [16, 50, 100, 250, 500, 1000]) {
    const count = page.getByRole('textbox', { name: 'Panel count' });
    await count.fill(String(n));
    await count.press('Enter');
    await expect(page.getByRole('banner')).toContainText(new RegExp(`${n} panels`), { timeout: 60_000 });
    // Wait for the full-resolution pass to land.
    await expect.poll(() => page.evaluate(() => (window as unknown as { __aeroflow: { useSolverStore: Store } }).__aeroflow.useSolverStore.getState().preview)).toBe(false);
    const cold = await page.evaluate(() => {
      const s = (window as unknown as { __aeroflow: { useSolverStore: Store } }).__aeroflow.useSolverStore.getState();
      const t = s.solution!.diagnostics.timings as { assembleMs: number; solveMs: number; totalMs: number };
      return { assembleMs: t.assembleMs, solveMs: t.solveMs, totalMs: t.totalMs, sampleMs: s.lastSampleMs, roundTripMs: s.lastRoundTripMs };
    });

    // Warm path: change only the freestream (matrix reused), as during a slider drag.
    const before = await page.evaluate(() => (window as unknown as { __aeroflow: { useSolverStore: Store } }).__aeroflow.useSolverStore.getState().solveCount);
    await page.evaluate(() => {
      const w = window as unknown as { __aeroflow: { useSimulationStore: { getState(): { setConditions(p: { angle: number }): void } } } };
      w.__aeroflow.useSimulationStore.getState().setConditions({ angle: 0.07 });
    });
    await expect.poll(() => page.evaluate(() => (window as unknown as { __aeroflow: { useSolverStore: Store } }).__aeroflow.useSolverStore.getState().solveCount)).toBeGreaterThan(before);
    await expect.poll(() => page.evaluate(() => (window as unknown as { __aeroflow: { useSolverStore: Store } }).__aeroflow.useSolverStore.getState().preview)).toBe(false);
    const warm = await page.evaluate(() => {
      const s = (window as unknown as { __aeroflow: { useSolverStore: Store } }).__aeroflow.useSolverStore.getState();
      return { reused: s.solution!.diagnostics.systemReused, solveMs: s.solution!.diagnostics.timings.totalMs, sampleMs: s.lastSampleMs, roundTripMs: s.lastRoundTripMs };
    });

    // Frame time of the main thread with particles animating (UI responsiveness, NFR-001).
    const frame = await page.evaluate(
      () =>
        new Promise<{ meanMs: number; p95Ms: number }>((resolve) => {
          const t: number[] = [];
          let last = performance.now();
          const tick = (now: number) => {
            t.push(now - last);
            last = now;
            if (t.length < 90) requestAnimationFrame(tick);
            else {
              const s = t.slice(10).sort((a, b) => a - b);
              resolve({ meanMs: s.reduce((a, b) => a + b, 0) / s.length, p95Ms: s[Math.floor(s.length * 0.95)] });
            }
          };
          requestAnimationFrame(tick);
        }),
    );

    rows.push({
      panels: n,
      coldAssembleMs: cold.assembleMs,
      coldSolveMs: cold.solveMs,
      coldTotalMs: cold.totalMs,
      coldSampleMs: cold.sampleMs,
      coldRoundTripMs: cold.roundTripMs,
      warmReused: warm.reused,
      warmSolveMs: warm.solveMs,
      warmSampleMs: warm.sampleMs,
      warmRoundTripMs: warm.roundTripMs,
      frameMeanMs: frame.meanMs,
      frameP95Ms: frame.p95Ms,
    });
    expect(warm.reused, `${n} panels: influence matrix should be reused`).toBe(true);
  }
  mkdirSync('test-results', { recursive: true });
  writeFileSync('test-results/perf.json', JSON.stringify(rows, null, 2));
  console.table(rows);
});

test('main-thread frames stay smooth while a 1000-panel cold solve runs (NFR-003)', async ({ page }) => {
  test.setTimeout(120_000);
  await openApp(page);
  await loadExample(page, 'NACA 0012');
  await page.getByRole('button', { name: /^NACA 0012/ }).click();
  // Record rAF intervals across the whole solve.
  await page.evaluate(() => {
    const w = window as unknown as { __frames: number[]; __rec: boolean };
    w.__frames = [];
    w.__rec = true;
    let last = performance.now();
    const tick = (now: number) => {
      w.__frames.push(now - last);
      last = now;
      if (w.__rec) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const count = page.getByRole('textbox', { name: 'Panel count' });
  await count.fill('1000');
  await count.press('Enter');
  await expect(page.getByRole('banner')).toContainText(/1000 panels/, { timeout: 60_000 });
  const frames = await page.evaluate(() => {
    const w = window as unknown as { __frames: number[]; __rec: boolean };
    w.__rec = false;
    return w.__frames.slice(2);
  });
  const sorted = [...frames].sort((a, b) => a - b);
  const p95 = sorted[Math.floor(sorted.length * 0.95)];
  const worst = sorted[sorted.length - 1];
  console.log(`frames during cold solve: n=${frames.length} p95=${p95.toFixed(1)} ms worst=${worst.toFixed(1)} ms`);
  mkdirSync('test-results', { recursive: true });
  writeFileSync('test-results/perf-during-solve.json', JSON.stringify({ n: frames.length, p95, worst }, null, 2));
  // The worker solves for ~300 ms; the main thread must keep painting throughout.
  expect(frames.length).toBeGreaterThan(10);
  expect(p95).toBeLessThan(34); // at most one dropped frame at p95
});
