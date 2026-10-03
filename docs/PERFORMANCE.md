# Performance report (PRD §46, §65, NFR-001…004)

Measured 2026-10-03 on the development machine (Linux, Chromium 153 headless shell
via Playwright). Reproduce:

```bash
# Native (Rust release build)
cargo test -p aeroflow-solver --release --test analytical bench_panel_counts -- --ignored --nocapture
cargo test -p aeroflow-solver --release --test bench_sampling -- --ignored --nocapture
# In the browser (WASM in a Web Worker); writes apps/web/test-results/perf.json
cd apps/web && npx playwright test e2e/perf.spec.ts
```

## In the browser — NACA 0012, WASM in a Web Worker

*Cold* = geometry changed (panel count edited): influence matrix assembled and
factorised. *Warm* = only the freestream changed, as while dragging a slider or an
element: the cached LU factorisation is reused (asserted by the benchmark).
"Field + streamlines" is the full-resolution sampling pass (≈280×150 scalar grid plus
evenly spaced streamlines). "Round trip" is main thread → worker → main thread.
Frame times are main-thread `requestAnimationFrame` intervals with animated
particles running, sampled after the solve settles.

| Panels | assemble + LU | solve (total) | field + streamlines | round trip | warm solve | warm field | warm round trip | frame mean / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 16 | 0.10 ms | 0.10 ms | 16 ms | 28 ms | 0.00 ms | 16 ms | 20 ms | 16.7 / 16.7 ms |
| 50 | 0.20 ms | 0.20 ms | 17 ms | 30 ms | 0.10 ms | 16 ms | 19 ms | 16.7 / 16.8 ms |
| 100 | 0.70 ms | 0.80 ms | 19 ms | 28 ms | 0.20 ms | 22 ms | 25 ms | 16.7 / 16.7 ms |
| 250 | 5.40 ms | 5.70 ms | 17 ms | 28 ms | 0.40 ms | 17 ms | 20 ms | 16.7 / 16.7 ms |
| 500 | 47.60 ms | 48.50 ms | 13 ms | 63 ms | 1.20 ms | 14 ms | 19 ms | 16.7 / 16.8 ms |
| 1000 | 293.00 ms | 297.70 ms | 16 ms | 310 ms | 3.20 ms | 14 ms | 20 ms | 16.7 / 16.8 ms |

## Native — same solver, Rust release build

| Panels | assemble + LU | solve | total | scalar field 256×171, render path | scalar field, exact |
| --- | --- | --- | --- | --- | --- |
| 50 | 0.07 ms | 0.01 ms | 0.10 ms | 11 ms | 36 ms |
| 100 | 0.32 ms | 0.01 ms | 0.36 ms | 12 ms | 69 ms |
| 250 | 2.93 ms | 0.09 ms | 3.1 ms | 12 ms | 170 ms |
| 500 | 16.8 ms | 0.43 ms | 17.4 ms | 9 ms | 340 ms |
| 1000 | 127 ms | 3.2 ms | 131 ms | 11 ms | 685 ms |

Streamline tracing (≈4,800 points), exact field vs. render path:

| Panels | exact | render path |
| --- | --- | --- |
| 160 | 60 ms | 7 ms |
| 500 | 184 ms | 5 ms |
| 1000 | 366 ms | 5 ms |

## Reading the numbers against the requirements

| Requirement | Target | Result |
| --- | --- | --- |
| NFR-001 UI frame rate | ≈ 60 FPS | 16.7 ms mean **and** p95 at every size up to 1000 panels |
| NFR-002 typical interactive solve | < 100 ms | warm round trip 19–25 ms at every size; cold round trip < 100 ms up to 500 panels |
| NFR-003 main thread never blocked | — | all numerics run in the worker. Measured **during** a 1000-panel cold solve (≈ 380 ms, 23 frames): p95 16.7 ms, worst 16.8 ms — no dropped frames (`e2e/perf.spec.ts`, NFR-003 test) |
| NFR-004 baseline panel count | determined empirically | **500 panels** for sub-100 ms cold edits; up to **1000+** for interactive dragging, since drags hit the warm path |

## Why the curve is shaped this way

* **Assembly + LU is the only term that grows with N** (O(N²) influence, O(N³)
  LU); it is paid only when geometry changes. WASM runs it ≈ 2.3× slower than
  native at 1000 panels (293 vs. 127 ms).
* **Warm solves are O(N²) back-substitution** — 3 ms at 1000 panels — because the
  factorisation is cached by a hash of the body geometry. Dragging elements or
  changing α, U∞ or circulation never re-factorises.
* **Sampling is flat in N.** The render path replaces distant panel clusters by
  equivalent point singularities (error < 1 %; numbers shown to the user always use
  the exact field). Streamlines use the same approximation since 2026-10-03, which
  took them from linear-in-N to constant (366 → 5 ms at 1000 panels).
* **Progressive refinement** keeps drags smooth regardless: a 96-column preview is
  sampled during the gesture and the full-resolution pass runs after 220 ms idle.
