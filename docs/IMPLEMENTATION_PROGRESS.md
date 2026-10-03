# AeroFlow — Implementation Progress Log

Rolling progress log for the implementation of `docs/prd/PRD1_webApp.md`
(2D Potential Flow Simulator). A new entry is appended at every ~5% milestone.

| Field | Value |
| --- | --- |
| Project | AeroFlow — 2D Potential Flow Simulator |
| Spec | `docs/prd/PRD1_webApp.md` |
| Started | 2026-10-02 |
| Current progress | **100%** |

---

## Progress ledger

| % | Milestone | Status |
| --- | --- | --- |
| 5 | Toolchain validated, architecture locked, `DEC-*` resolved, repo skeleton | ✅ done |
| 10 | `geometry` + `linear-algebra` crates with native tests | ✅ done |
| 15 | `flow-core`: elementary solutions, field evaluation, streamlines | ✅ done |
| 20 | `panel-method`: influence matrices, global multi-body system, Kutta | ✅ done |
| 25 | `solver`: orchestration, forces/coefficients, diagnostics, sweeps | ✅ done |
| 30 | Analytical regression suite green (cylinder, vortex, source, superposition) | ✅ done |
| 35 | `wasm-api` + wasm-pack build + Web Worker transport + typed TS client | ✅ done |
| 40 | React/TS app shell, state domains, design system, theming | ✅ done |
| 45 | Canvas: pan/zoom/grid/axes, object CRUD, selection, drag, handles | ✅ done |
| 50 | WebGL2 scalar-field renderer + colormaps + dynamic legend | ✅ done |
| 55 | Streamlines, vector field, animated particles | ✅ done |
| 60 | Properties panel, scene tree, freestream controls, undo/redo | ✅ done |
| 65 | Results panels: per-body + total forces, diagnostics, warnings | ✅ done |
| 70 | Charts: surface Cp / velocity / pressure plots | ✅ done |
| 75 | Polar sweeps (CL/CD/Cm vs α, drag polar) with cancellation | ✅ done |
| 80 | Geometry import pipeline + validation UX + NACA generator | ✅ done |
| 85 | Persistence, export (JSON/CSV/PNG/SVG), examples, first-run | ✅ done |
| 90 | Help system, assumptions/limitations UX, accessibility pass | ✅ done |
| 95 | Test suites: Rust numerical, Vitest, Playwright E2E, perf benchmarks | ✅ done |
| 100 | Final polish, docs, README, verification report | ✅ done |

---

## 5% — Foundations: toolchain, architecture, open decisions resolved

**Date:** 2026-10-02

### Toolchain verified
- `rustc` stable + `wasm32-unknown-unknown` target + `wasm-pack 0.15.0`
- `node v24.11.0`, `npm 11.6.1`
- `wat2wasm` (wabt) available for WASM inspection
- crates.io + npm registry reachable; a Rust→WASM smoke build was compiled and verified
  before committing to this architecture.

### Repository layout
```
crates/
├── geometry/        # Vec2, polygons, validation, panelisation, NACA generation
├── linear-algebra/  # dense LU w/ partial pivoting, residual, condition estimate
├── flow-core/       # elementary solutions, field evaluation, streamlines, forces
├── panel-method/    # influence coefficients, global multi-body system, Kutta
├── solver/          # orchestration Scene -> Solution, sweeps, diagnostics
└── wasm-api/        # wasm-bindgen + serde boundary (thin)
apps/web/            # React + TypeScript + Vite front end
tests/numerical/     # tolerance report for the analytical acceptance suite
examples/            # bundled example simulations + geometry fixtures
```

### Open decisions resolved (PRD §87)

**DEC-001 — Panel formulation: Hess–Smith.**
Constant-strength source panels (one σⱼ per panel) plus one uniform vortex strength γ
per body; N flow-tangency equations at panel midpoints + 1 Kutta equation per body.
Chosen over per-panel vortex strengths because it needs no regularisation, is the
formulation XFOIL's inviscid core descends from, and extends to multiple bodies by
assembling a single *global* influence matrix (PRD §13 forbids solving bodies
independently). Circulation modes: `kutta` (solve for γ), `none` (γ ≡ 0, for
non-lifting bodies with no sharp trailing edge), `prescribed` (γ fixed from a
user-supplied Γ — enables the rotating-cylinder/Magnus demonstration).

**DEC-002 — Streamlines: WASM RK4 integration of the authoritative velocity field.**
Not ψ-contours: the stream function is multivalued when net source strength is
non-zero (branch cut), and contouring ψ across bodies is fragile. RK4 on the
velocity field is robust, handles bodies via a point-in-polygon stop test, and keeps
the numerical core authoritative. Rendering/animation happens on the GPU over
precomputed polylines.

**DEC-003 — Rendering: WebGL2 scalar fields + Canvas 2D overlay, behind an abstraction.**
The field is uploaded once per solve as an `R32F` texture; the colormap, range and
contour banding are evaluated in the fragment shader, so changing colormap or
clipping range costs zero recompute. Geometry, handles, vectors, streamlines and the
legend draw on a Canvas 2D overlay where crisp text and hit-testing matter.

**DEC-004 — Geometry: import-first, plus a parametric generator.**
Import (`.csv`/`.txt`/`.dat`) is the primary path as the PRD specifies. A built-in
NACA 4-digit / cylinder / ellipse generator is *also* included because PRD §57
requires examples to open "without uploading anything" and §81/§82 mandate cylinder
and NACA 0012 showcases.

**DEC-005 — Desktop-first**, with side panels collapsing on tablet/mobile.

### Documented deviations from the PRD
1. **Added parametric geometry generation** to MVP (justified above; PRD §32 lists it
   as future, but §57/§81/§82 require it).
2. **Single Vite application instead of an npm monorepo.** Module boundaries are
   enforced by directory structure and a lint rule rather than by package
   boundaries. The PRD explicitly delegates monorepo tooling to the implementer
   (§66); a Cargo workspace *is* used for the crates because independent
   `cargo test` per crate is real value (WASM-007).
3. **Cosine-spaced re-panelisation** of imported geometry added to the geometry
   pipeline — it materially improves accuracy per panel for both cylinders and
   airfoils, where leading-edge curvature dominates the error.

---

## 10% — Geometry and linear-algebra kernels, 83 native tests green

**Date:** 2026-10-02

### `crates/geometry` — 67 tests

| Module | Contents |
| --- | --- |
| `vec2` | `Vec2`/`Bounds`; `perp_right` is the outward normal under CCW winding |
| `polygon` | shoelace area, winding, area centroid, point-in-polygon, proper self-intersection, arc length |
| `panel` | `Panel` (mid/length/tangent/outward normal/local frame), `Panelisation` + quality metrics |
| `validate` | 12 issue kinds with actionable messages, severity, occurrence collapsing; separate `cleanup` step |
| `repanel` | uniform / cosine / curvature / auto distributions, corner pinning |
| `shapes` | circle, ellipse, NACA 4-digit, Joukowski (analytical validation target) |
| `parse` | CSV, whitespace `.dat`, **Lednicer** two-block format, Fortran `D` exponents, size limit |

**Orientation convention nailed down.** Contours are stored counter-clockwise
with no repeated closing point. Under that winding each edge's outward normal is
its tangent rotated −90°, and a source panel's self-influence on its own
midpoint is exactly `+σ/2` along that normal. A test asserts that the common
airfoil file ordering (trailing edge → upper → leading edge → lower) is already
counter-clockwise, so typical imports are never silently reversed.

**Bug found by an analytical test.** The Joukowski generator's start angle was
`atan2(dy, c+dx)` where it must be `atan2(−dy, c+dx)`: the sharp trailing edge
is the map's image of `ζ = +c`, which lies *below* the pre-image circle's centre
when the camber offset is positive. The symmetric case (`dy = 0`) hid the error
completely; only the cambered test, which asserts the trailing edge lands exactly
on `z = 2c`, exposed it. Lesson applied to the rest of the suite: every
analytical case gets an asymmetric variant.

### `crates/linear-algebra` — 16 tests

Dense LU with partial pivoting, scale-invariant singularity detection,
transpose solve, `log|det|`, pivot ratio, and a **Hager 1-norm condition
estimator** (the algorithm behind LAPACK `*gecon`).

Why LU and not an iterative method: the panel system is dense and small
(`N + M` unknowns), and crucially the factorisation is *reusable*. An
angle-of-attack sweep changes only the right-hand side, so an `N_α`-point polar
costs one `O(n³)` factorisation plus `N_α` `O(n²)` back-substitutions rather
than `N_α` full factorisations. That is what makes PRD §29 sweeps interactive.

Condition estimation is validated three ways: ≈1 for the identity, bounded
above by the exact `cond₁ = 10⁶` of `diag(1, 10⁻⁶)` (it is a lower bound by
construction), and monotonically increasing on Hilbert matrices of growing
order.

---

## 15% — Physics core complete: elementary solutions, panel kernel, fields, streamlines, forces (186 tests green)

**Date:** 2026-10-02

### What landed

**`crates/flow-core` — 84 tests.** Owns the physics; no linear algebra, no
rendering, no browser (WASM-007 satisfied).

| Module | Contents |
| --- | --- |
| `conditions` | `U∞`, `α`, `ρ`, `p∞`; Bernoulli; wind-axis transform |
| `elements` | uniform flow, source, sink, vortex, doublet — `V`, `φ`, `ψ` each |
| `panel_kernel` | constant-strength source/vortex panel influence + `φ`/`ψ` integrals |
| `field` | `FlowField` with the generic `evaluate_*` API, grid sampling, masking |
| `streamline` | RK4 arc-length tracing + Jobard–Lefer evenly spaced placement |
| `forces` | surface tables, pressure integration, coefficients, moment |
| `descriptor` | element/parameter metadata + 14 structured help topics |

**`crates/geometry` — 86 tests** (77 unit + 9 import-pipeline integration),
now including `trailing_edge`: sharpest-corner detection and contour rotation,
so the Kutta condition is always imposed at the real trailing edge.

**`crates/linear-algebra` — 16 tests.** LU with partial pivoting, transpose
solve, Hager 1-norm condition estimator.

### Sign conventions locked (PRD §7, §14.1, §68)

`Γ > 0` is **counter-clockwise**, matching `ω_z = ∂v/∂x − ∂u/∂y`. Kutta–Joukowski
therefore reads `L = −ρU∞Γ`, so a *clockwise* circulation lifts. Documented in
the crate docs, asserted by a test, and stated in the UI help text — one
convention, three places that cannot drift.

### Numerical regression cases already green (PRD §61)

| Case | Verified against |
| --- | --- |
| §61.1 uniform flow | `u = U∞`, `v = 0` exactly |
| §61.2/3 source, sink | analytic `V_r = Λ/(2πr)`; flux integral recovers `Λ` |
| §61.4 vortex | analytic `V_θ = Γ/(2πr)`; circulation integral recovers `Γ` |
| §61.5 superposition | Rankine-oval stagnation points at `x = ±√(b² + Λb/πU)` |
| doublet + stream | cylinder: stagnation at `(±a,0)`, `2U` at the crown, zero normal flow on the whole surface |
| §61.7 cylinder + Γ | pressure-integrated lift matches `−ρU∞Γ` to 0.2%; `CD ≈ 0` (d'Alembert) |
| panel kernel | both influence formulas matched against 400 000-point quadrature |
| `∇φ = V`, `ψ` derivatives | central differences, every element and both panel types |

### Design decisions worth recording

**The panel kernel lives in `flow-core`, not `panel-method`.** A
constant-strength panel *is* an elementary analytic solution. That placement is
what makes the crate graph a DAG: field evaluation needs panel contributions,
while the panel solver needs elementary contributions for its right-hand side.
`flow-core` holds the kernel; `panel-method` will hold only assembly and solve.

**One geometric kernel yields both influence coefficients.** In a panel's local
frame the unit vortex field is the unit source field rotated +90°:
`(u_ξ, u_η)ᵛ = (−u_η, u_ξ)ˢ`. Both follow from two scalars, `ln(r₁/r₂)` and the
subtended angle `β` — one `ln` and one `atan2` per evaluation, no `sqrt`.

**Hierarchical far field for the render path.** A 256×256 field with 200 panels
is 13 M kernel evaluations (~0.5–0.8 s), against a sub-100 ms target. Panels are
grouped into 16-panel clusters carrying net source strength and circulation; a
sample far from a cluster sees one point singularity instead of 16 sheets. Error
is `O((L/d)²)`, measured at **under 1%** at a 12× threshold. Critically,
`far_field_ratio = 0` means exact and **is the default** — only rendering opts
in. Numbers reported to the user are never approximated.

**Vorticity gets a principled answer instead of a blank screen (PRD §21).**
Softening `r² → r² + a²` is not merely a clamp: the softened vortex has analytic
vorticity `ω = Γa²/(π(r²+a²)²)`, which integrates over the plane to *exactly*
`Γ`. A test confirms it to 2×10⁻⁴. The plot is physically meaningful and
conserves circulation, and the help text states that the fluid's true vorticity
is zero.

**Two independent self-checks make the solver auditable.** Lift is computed both
by pressure integration and by Kutta–Joukowski from the bound circulation;
`lift_consistency` reports their dimensionless difference. Net source outflow
`Σσⱼ Lⱼ` must be ≈ 0 for a closed body. Both are wired into the diagnostics the
PRD asks for in §44.

**The flow-tangency residual needs no extra machinery.** The tangency rows of
the panel system *are* `V·n̂ = 0`, so `(M·x − b)ᵢ` is literally the leftover
normal velocity at panel `i`. The linear-algebra residual and the physics
accuracy measure are the same number.

### Two real bugs caught by analytical tests

1. **Joukowski start angle.** `atan2(dy, c+dx)` must be `atan2(−dy, c+dx)` — the
   sharp trailing edge is the map's image of `ζ = +c`, which lies *below* the
   pre-image circle's centre for positive camber. The symmetric case hid it
   entirely; only the cambered test, asserting the trailing edge lands exactly
   on `z = 2c`, exposed a 1.2%-of-chord geometry error.
2. **Closed streamlines were traced twice.** Two-sided integration retraced a
   vortex orbit, doubling its reported arc length. A closed forward pass is now
   recognised as the complete curve.

Plus a third, found the same way: the inflow seed rake was placed 0.49 diagonals
upstream of the domain centre, which falls *outside* the box for most aspect
ratios, so every seed was silently filtered out and no streamlines appeared. It
now projects the box corners onto the flow direction and seeds just inside the
true upstream face.

---

## Status at the usage limit *(superseded — historical snapshot at 15 %; see later entries)*

**Done (≈15%):** the complete, independently validated numerical physics core —
186 native tests green via `cargo test --workspace`. Architecture locked, all
five `DEC-*` decisions resolved and recorded, conventions fixed and tested.

**Not started:** `panel-method` assembly/solve (the linear system; design is
worked out in detail in the 5% entry and the module docs), `solver`
orchestration and sweeps, `wasm-api` bindings, the Web Worker transport, and the
entire React/TypeScript front end — canvas, WebGL field renderer, charts,
import/export UX, persistence, examples, help UI, themes, accessibility, and the
Vitest/Playwright suites.

The two placeholder crates (`panel-method`, `solver`, `wasm-api`) contain only
`// placeholder` stubs; the workspace builds and tests clean with them in place.

**Where to resume:** `crates/panel-method/src/lib.rs`. The formulation, matrix
layout, Kutta row construction, fixed-vs-unknown circulation handling and
factorisation reuse for sweeps are all specified in the 5% progress entry and in
`crates/flow-core/src/panel_kernel.rs`'s module documentation, including the
exact self-influence constants (`A_ii = 1/2`, `Bᵗ_ii = 1/2`, `Aᵗ_ii = B_ii = 0`)
that the assembly needs.

---

## 30% — Panel method, solver orchestration, analytical acceptance suite — 261 tests, WASM package built

**Date:** 2026-10-02

### Milestones 20 → 30 in one stretch

**`crates/panel-method` — 15 tests.** Hess–Smith assembly in a single `O(N²)`
pass filling normal *and* tangential influence; one global matrix for all
bodies (PRD §13); fixed circulations folded into the right-hand side so a
prescribed-Γ body adds no unknown; LU reused across right-hand sides. The
flow-tangency residual `(Mx − b)ᵢ` *is* the leftover normal velocity at panel
`i`, so the physics accuracy measure costs nothing extra.

**`crates/solver` — 32 unit + 22 analytical tests.** `Scene` (the versioned
`.aeroflow.json` format), body preparation pipeline (orientation → re-panel →
trailing-edge rotation → world transform → circulation resolution), geometry-hash
caching of the influence matrix, per-body/total forces, scene-level warnings
with object ids, step-wise sweeps for cancellation, point probes.

**`crates/wasm-api`** — thin boundary, typed arrays for bulk data, built with
`wasm-pack --target web` to `apps/web/src/wasm/pkg` (515 KB before gzip).

### Analytical acceptance results — see `tests/numerical/TOLERANCES.md`

| Case | Result |
| --- | --- |
| Cylinder Cp at midpoints | **exact to 3e-15** for every N (circulant system) |
| Ellipse a/b = 2, surface speed | 6.2e-4 → 1.4e-4 → 3.4e-5 for N = 40/80/160: **second order** |
| Rotating cylinder vs Kutta–Joukowski | lift −0.43 %, Cp RMS 3.9e-3 |
| NACA 0012 α = 5° | CL 0.6025 (XFOIL ≈ 0.600), CD 2e-4, Cm −0.0065 |
| NACA 0012 lift slope | 6.907 /rad (XFOIL ≈ 6.9) |
| NACA 2412 α = 0° | CL 0.2554, Cm −0.0543 (XFOIL ≈ −0.053), α₀ = −2.12° |
| Joukowski (true cusp) | first-order: −7.1 % (N=200) → −2.4 % (N=800); circulation −2.4 % → −0.8 % |
| Tandem / side-by-side / biplane | repel / attract / lose lift — all antisymmetric to 1e-9 |

### What the suite caught

1. **Moment sign convention.** NACA 2412 came out `Cm = +0.0543`: right
   magnitude, wrong sign. Lift is defined to the left of the freestream and the
   nose is upstream, so "nose-up" is a *clockwise* rotation for every flow
   angle — the aerodynamic `Cm` is `−M_z/(q∞c²)`. Fixed, and a test now checks
   the convention is flow-angle independent.
2. **Near-wall rendering spikes.** Constant-strength panels have a log
   singularity at every panel *vertex*; field samples within a fraction of a
   panel length showed Cp = −5.4 on a cylinder whose true minimum is −3. The
   renderer now masks a 0.75-panel-length band (numerical API untouched).
3. **Cusped trailing edges converge first-order** with a source-based
   formulation. Investigated, hypothesis of trimming the sliver tested and
   *rejected* (it tilts the TE bisector; −44 % lift for a 2 % trim). Documented;
   the `lift_consistency` diagnostic warns above 1 % and suggests more panels.
4. Several of my own test expectations were wrong in instructive ways — the
   stagnation point of a clockwise vortex is *below* it; stagnation points
   always have Cp = 1 so they cannot show interaction; two stacked lifting
   airfoils form a biplane, not a ground-effect image. Each is now a test that
   asserts the correct physics.

### Performance (PRD §65), native release build

| Panels | assemble + LU | solve | total solve | field 256×171 far-field | field exact |
| --- | --- | --- | --- | --- | --- |
| 50 | 0.07 ms | 0.01 ms | 0.10 ms | 11.1 ms | 36 ms |
| 100 | 0.32 ms | 0.01 ms | 0.36 ms | 12.0 ms | 69 ms |
| 250 | 2.93 ms | 0.09 ms | 3.1 ms | 11.8 ms | 170 ms |
| 500 | 16.8 ms | 0.43 ms | 17.4 ms | 9.2 ms | 340 ms |
| 1000 | 127 ms | 3.2 ms | 131 ms | 10.9 ms | 685 ms |

The far-field hierarchy holds field evaluation at ~11 ms **independent of panel
count** (6× faster than exact at 100 panels, 63× at 1000). With the influence
matrix cached, dragging an element past a 250-panel airfoil costs a 0.09 ms
re-solve plus ~12 ms of field sampling. Baseline panel count for NFR-004:
**500 panels** stays under 100 ms for solve + field on this machine.

### Next

`apps/web`: domain model mirroring the serde schema, worker + client, state
domains, canvas and WebGL renderer.

---

## 40% — WASM boundary, worker transport, state architecture, application shell and canvas — typecheck clean

**Date:** 2026-10-02

### Milestones 35 → 40

**`crates/wasm-api`** (4 native tests) — thin `wasm-bindgen` boundary: structured
data via `serde-wasm-bindgen`, bulk render data as transferable typed arrays,
step-wise sweeps for cancellation. Built with `wasm-pack --target web` into
`apps/web/src/wasm/pkg` (515 KB, ~150 KB gzipped).

**`apps/web`** — React 19 + TypeScript + Vite 6 + Zustand, strict `tsc` clean.

| Layer | Files | Notes |
| --- | --- | --- |
| Domain | `domain/types.ts`, `domain/scene.ts`, `domain/examples.ts` | TS mirror of the serde schema; 10 bundled examples |
| Solver transport | `solver/protocol.ts`, `solver.worker.ts`, `client.ts`, `bridge.ts` | module worker owns WASM; client coalesces (latest wins); bridge decides solve vs. sample |
| State (PRD §49) | `state/{simulation,ui,viewport,visualization,solver}Store.ts` | snapshot undo/redo with drag transactions |
| Rendering | `render/fieldLayer.ts` (WebGL2), `render/colormaps.ts`, `canvas/overlay.ts` | shader-side colour mapping, banding, isolines; mask-aware bilinear |
| Interaction | `canvas/interaction.ts`, `canvas/bodyCache.ts`, `canvas/fit.ts` | select/drag/rotate/box/pan/zoom/pinch/place/seed |
| UI | `components/*`, `charts/LineChart.tsx`, `styles/*` | descriptor-driven properties, results, surface/polar/diagnostics/data tabs, dialogs, help drawer |

### Decisions worth recording

**Self-describing field results.** The client coalesces requests, so a resolved
field may belong to a *later* viewport than the one a given call computed. The
worker therefore stamps every sampled field with its own world `bounds`; the
renderer never guesses where a texture belongs.

**Progressive refinement.** Every change first samples a 96-column preview so
the picture follows the cursor during a drag, then a full-resolution pass runs
after 220 ms of idle. Scene edits solve + sample; pan/zoom and display settings
only re-sample.

**Re-posed body contours.** Contours come from the solver (which owns
panelisation and trailing-edge rotation). During a drag the overlay re-poses the
last solved contour by the transform delta, so the outline tracks the cursor a
frame ahead of the solve.

**Shader-side colour mapping.** The field is an `R32F` texture plus an `R8`
body mask; colour map, range, contour bands and isolines are fragment-shader
uniforms. Switching colour map or range costs zero recompute. Bilinear
interpolation is done by hand from four `texelFetch`es so masked cells drop out
of the weights instead of bleeding body interiors into the fluid.

**Particles without a particle system.** Animated marching dashes
(`setLineDash` + `lineDashOffset`) along each precomputed streamline, paced by
the line's mean speed — one stroke per line, no per-particle state, honours
`prefers-reduced-motion`.

### Next
Browser verification of the running app, Vitest + Playwright suites, then
polish: accessibility pass, responsive layout, README.

---

## 50% — Running application verified in a browser: solver boots, fields render, 10/10 E2E flows and 32 unit tests pass

**Date:** 2026-10-02

### Verified in a real browser

| Check | Result |
| --- | --- |
| Worker + WASM boot | ✅ after disabling the distro `wasm-opt` (v108 corrupts the externref table) |
| Default cylinder solve | ✅ 120 panels · 4 ms solve · 18 ms field, flow-tangency residual 1.6e-15 |
| Field rendering (WebGL2), legend, probe read-out | ✅ |
| Cp surface plot | ✅ matches `1 − 4 sin²θ` |
| Keyboard field switching, dialogs, scene tree, properties forms | ✅ |
| Playwright E2E (PRD §63 required flows) | ✅ 10 / 10 |
| Vitest unit suites | ✅ 32 / 32 |
| Rust native suites | ✅ 263 / 263 |

### Bugs found and fixed through the browser and E2E runs

1. **Infinite re-render loop** — zustand selectors returning a fresh array
   (`?? []`, `.filter()` inside the selector) trip `useSyncExternalStore`'s
   snapshot check. Replaced with a frozen `EMPTY` fallback and `useMemo`.
2. **`WebAssembly.Table.grow()` failure at startup** — the system `wasm-opt`
   (binaryen 108) predates reference types and mangles wasm-bindgen's externref
   table. `wasm-opt = false` in the wasm-pack profile; the Rust release profile
   already does LTO + `opt-level 3`.
3. **Toolbar clicks swallowed** — the visualisation toolbar sits inside the
   canvas container, whose `pointerdown` handler captured the pointer, retargeting
   `pointerup` and suppressing the button's `click`. Pointer, wheel and
   double-click handling now ignore events that don't start on the drawing
   surface. Keyboard field switching had masked the bug.
4. **Unplaceable centre on an empty scene** — the "Add a flow element…" card had
   `pointer-events: auto` in the middle of the canvas, so the first placement
   click landed on it. The card hides while a placement is armed.
5. **Staircase ring around bodies** — whole near-wall cells were discarded,
   leaving blocky edges. Near-wall cells now carry their own mask code and are
   filled by dilation from fluid neighbours; the shader draws them, exports flag
   them (`mask = 2`).

### Test infrastructure
* `window.__aeroflow` dev-only store hook so E2E can assert state (e.g. a
  freshly loaded scene is not "dirty").
* E2E selectors use exact accessible names; menu items gained `aria-label`s,
  which also improves screen-reader output.

---

## 80% — Audit of milestones 55–80 against the PRD; gaps closed; streamline tracing 70× faster

**Date:** 2026-10-03

### Why this entry jumps from 50% to 80%

Milestones 55–80 (streamlines/vectors/particles, properties panel and undo, results
panels, surface charts, polar sweeps, geometry import + NACA generator) were
implemented during the 40–50% stretch but never ticked off. This session
**audited each one against the PRD** instead of ticking them blindly, then fixed what
the audit found.

| Row | PRD | Verified by | Gap found → fixed |
| --- | --- | --- | --- |
| 55 Streamlines, vectors, particles | §17, §18 | browser + visual baselines | tracing cost grew linearly with panel count (see below) |
| 60 Properties, scene tree, undo | §31–§33, §50 | E2E (create/move/edit/undo) | geometry import/creation produced **two** undo entries → now one |
| 65 Results, diagnostics, warnings | §24, §44, §71 | E2E (NACA CL, d'Alembert note) | a **false** `nonUniformPanels` warning on the default NACA example (below); no error boundary → added per panel |
| 70 Surface plots | §28 | E2E + visual | — |
| 75 Polar sweeps + cancellation | §29, §60 | **new** E2E: real cancel through the worker; Γ-sweep obeys `L = −ρU∞Γ` | only α was sweepable → speed, body circulation, element strength added |
| 80 Import + validation + NACA | §10, §11 | E2E (import, self-intersection) | `Naca4::generate(1000)` emitted 998 points → exactly `n` now |
| — Moment reference | §27 | **new** E2E: Cm changes, CL invariant | reference point not settable from the UI → added |
| — Module boundaries | §66–§68 | **new** Vitest suite (8 tests) | the lint rule promised in the 5% entry did not exist → enforced as a test with a positive control |

### Streamline tracing: 365 ms → 5 ms at 1000 panels

Measured before changing anything (`crates/solver/tests/bench_sampling.rs`):

| Panels | scalar field | streamlines, exact | streamlines, render path |
| --- | --- | --- | --- |
| 160 | 15 ms | 60 ms | 7 ms |
| 500 | 10 ms | 184 ms | 5 ms |
| 1000 | 11 ms | 366 ms | 5 ms |

Streamlines integrated the *exact* field, so ~4,800 points × 5 evaluations × N
panels. They are a visualisation — no reported number derives from them — so the
render path now uses the same far-field hierarchy as the scalar field
(`StreamlineConfig::for_rendering`; `EXACT` stays the default). A new test bounds
the deviation from the exact streamline at < 0.02 m across a 10 m domain.

### The false warning — a criterion error, not a threshold error

The default NACA 0012 example showed "Solved · warnings" because the global
`max/min` panel-length ratio was 51 > 50. Cosine clustering *intentionally* grades
lengths by ~n/3 while the condition number stayed at 2×10³. What actually hurts a
panel method is an abrupt change between **neighbouring** panels. Both the solver
warning and the import validation now use the worst adjacent ratio (> 4×), with tests
proving a smooth 1000-panel grading passes and a 10× jump is flagged with its panel
indices.

### Counts
Rust 267 · Vitest 40 · Playwright 20 (13 functional + 7 visual) — all green.

---

## 85% — Shared scene contract proven across TypeScript and Rust; examples verified to open with no warnings

**Date:** 2026-10-03

### Cross-language contract test (PRD §68)

`apps/web/scripts/export-examples.ts` (`npm run examples`) writes every bundled
example to `examples/*.aeroflow.json` with stable ids. Two suites pin the contract
from both sides:

* **Vitest** (`exampleFixtures.test.ts`, 10 tests) — each fixture equals what the UI
  currently produces, so a stale fixture fails.
* **Rust** (`crates/solver/tests/examples.rs`, 8 tests) — each fixture deserialises
  into the Rust `Scene`, solves with no error, **shows no warning to a first-time
  user**, and satisfies a physics claim from its own description (cylinder lift = 0,
  NACA 0012 CL ≈ 0.60, Magnus lift = −ρU∞Γ, NACA 2412 nose-down Cm, biplane
  interference, closed Rankine oval, doublet ≡ panel cylinder off-body).

Plus `examples/naca2412-selig.dat`, a real coordinate file for trying the import
path, with a test that it imports cleanly with a detected trailing edge.

### What the "no warnings on examples" rule caught

1. **Biplane** warned at the default 120 panels (pressure vs. Kutta–Joukowski lift
   1.0% apart on the lower wing). The warning was right; the example now uses 160
   panels per wing.
2. **Magnus** — the example text claimed lift matches `−ρU∞Γ` "exactly". It matches
   to −0.43%; the wording now says so.
3. **A mis-normalised diagnostic.** The lift-consistency metric divided by `q∞c`,
   inflating the apparent error by a factor of CL on high-lift bodies: the Magnus
   cylinder (CL ≈ 4) reported 1.7% for a −0.43% actual error. It is now
   `|ΔL| / max(|L_KJ|, q∞c)` — identical for CL < 1, a true relative error above. A
   new unit test pins this; `TOLERANCES.md` records the revision.

### Persistence / export (already in place, re-verified by E2E)
`.aeroflow.json` save/open (versioned), forces/surface/geometry/field CSV, canvas
PNG, chart SVG, sweep CSV.

### Counts
Rust 277 · Vitest 50 · Playwright 20 — all green.

---

## 90% — Accessibility pass (axe: 25 violations → 0), responsive layout, keyboard-only workflow; missing cylinder streamlines found and fixed

**Date:** 2026-10-03

### Accessibility (PRD §54, WCAG 2.2 AA where practical)

`e2e/a11y.spec.ts` runs **axe-core** (WCAG 2.0/2.1/2.2 A + AA tags) against the
workspace in both themes, the dialogs and the analysis tabs, plus a keyboard-only
workflow and a focus-ring check. First run: **25 violations across 5 rules** → 0.

| Rule | Cause | Fix |
| --- | --- | --- |
| `aria-required-children`, `listitem` | scene tree was `role="listbox"` with non-`option` rows; a "Collapse" button sat inside `role="tablist"` | plain labelled list; tablist holds only tabs |
| `button-name` | icon-only Help menu trigger | `aria-label` |
| `color-contrast` | `--text-faint` 2.5:1 (light) / 3.1:1 (dark); accent on accent tint | colours chosen by **computing** ratios against every surface incl. the selection tint: light `#5b6776`, dark `#8f9bab`, `--accent-hover` for text on tints, muted text on selected rows |
| `target-size` (2.5.8) | 18 px range sliders | 24 px |
| `scrollable-region-focusable` (2.1.1) | Diagnostics read-outs scroll but contain nothing focusable | focusable, labelled regions |

**Keyboard placement.** Placing an element required a canvas click. With an element
armed, **Enter** now drops it at the view centre; arrow keys nudge the selection one
grid step (Shift ×10). The E2E adds, nudges and deletes a source without a pointer.

### A test that was silently testing the wrong thing
`openApp()` forced `theme = light` in an init script that also runs on `reload()`, so
the "dark theme" visual baseline **and** the dark axe scan were both light mode. The
helper now takes the theme as a parameter and `expectTheme()` asserts the resolved
`data-theme`. The first genuine dark scan immediately found one more contrast failure.

### Responsive layout (PRD §38)
At 820 px and 390 px the page scrolled sideways (239 / 669 px overflow): the top bar
didn't fit. Compact rules hide secondary labels (keeping `aria-label`s and a
visually-hidden live status), stack the analysis panel, and the first solved scene is
now fitted to the device's canvas instead of a fixed desktop view. Tested at both
sizes: canvas ≥ 90 % width, panels open as overlays, zero horizontal overflow.

### A flagship-example bug the screenshots revealed
The cylinder example (PRD §81 lists streamlines as a required view) showed **no
streamlines**. Jobard–Lefer placement started from a single seed at the view centre;
a body covering the centre rejected it and the queue was empty. A failing regression
test reproduced it (0 lines), then a fallback upstream rake fixed it. An E2E now
asserts > 8 streamlines for four examples.

It survived this long because the visual tolerance (2 % of pixels) was larger than a
whole thin-line layer (~1.5 %), and Playwright's `--update-snapshots` only rewrites
failing images. Tolerance is now 0.4 %, stable over three consecutive runs.

### Counts
Rust 278 · Vitest 50 · Playwright 28 (14 functional + 7 visual + 5 accessibility +
2 responsive) — all green.

---

## 95% — One-command verification, production-build smoke tests, in-browser performance report, lint gates

**Date:** 2026-10-03

### `scripts/verify.sh` — everything CI would run, in order
rustfmt check → clippy `-D warnings` → Rust tests → WASM build → `tsc` → Vitest →
Playwright (dev server) → Playwright against the **production build**.
`SKIP_E2E=1` skips the browser stages. Full run: all green.

### Production-build smoke tests (`playwright.preview.config.ts`)
Every earlier E2E used the dev server, which hides asset-path problems for the worker
and the `.wasm`. This config runs `npm run build && vite preview` and checks: the
worker boots and solves NACA 0012 (CL in range) with **zero console errors**,
geometry import works through the worker, and the dev-only `window.__aeroflow` hook
is **absent** from the shipped bundle.

### In-browser performance (`docs/PERFORMANCE.md`, `e2e/perf.spec.ts`)
| Panels | cold round trip | warm round trip | frame mean / p95 |
| --- | --- | --- | --- |
| 100 | 28 ms | 25 ms | 16.7 / 16.7 ms |
| 500 | 63 ms | 19 ms | 16.7 / 16.8 ms |
| 1000 | 310 ms | 20 ms | 16.7 / 16.8 ms |

Frames measured **during** a 1000-panel cold solve: p95 16.7 ms, worst 16.8 ms —
no dropped frames. NFR-004 baseline: 500 panels for sub-100 ms cold edits; 1000+ for
dragging (the warm path). A draft of the report claimed frame times during a solve
before they had been measured; the claim was removed, the measurement was added, and
the report now states the measured figure.

The WASM clock moved from `Date.now()` (1 ms resolution: solves read as "0.0 ms") to
the worker's `performance.now()`.

### Lint gates and what they caught
* **rustfmt** applied once across the workspace (nothing committed yet, so no review
  noise); now enforced.
* **clippy** — ~25 warnings fixed. Two suggestions were **rejected on purpose**:
  `!(length > 0.0)` → `length <= 0.0` and the ellipse semi-axis equivalent. The
  rewrites are false for NaN and would have let NaN geometry into the solver. They are
  now an explicit `positive_finite()` helper with tests pinning NaN rejection.
  `needless_range_loop` is allowed, with a comment, in the dense numerical kernels.

### Smaller fixes
* Panel-count field accepted 8 while generators silently clamped to 16; the UI minimum
  is now 16 and the solver adds a note whenever it raises a count.

### Counts
Rust 281 · Vitest 50 · Playwright 30 dev + 3 production — all green.

---

## 100% — MVP complete: every PRD §79 Definition-of-Done item has a passing test; README, LICENSE, verification report

**Date:** 2026-10-03

### Deliverables
* **`README.md`** — quick start (including the required one-time `npm run wasm`, since
  `src/wasm/pkg` is git-ignored), features, model limits and sign conventions,
  accuracy and performance summaries, layout, testing.
* **`LICENSE`** — MIT, as already declared in `Cargo.toml` (wasm-pack had warned
  about the missing file on every build).
* **`docs/VERIFICATION.md`** — all 29 Definition-of-Done criteria mapped to the test
  that demonstrates each, plus §27/§29/§38/§39/§54/§60/§64/§65/§66–68, and an explicit
  known-limitations list.

### Evidence gaps closed while writing the report
Mapping criteria to tests showed that items 2–8 (add / move / edit **each** element
type) were only partly evidenced: uniform flow, sink and doublet had never been added
through the UI by a test. `e2e/dod.spec.ts` now adds, moves and edits all five types,
and adds direct checks for the surface Cp plot (18) and multi-body results (22).

### Final state
`scripts/verify.sh` → exit 0:

| Stage | Result |
| --- | --- |
| rustfmt, clippy `-D warnings` | clean |
| Rust tests | 281 passed |
| WASM build, TypeScript typecheck | clean |
| Vitest | 50 passed |
| Playwright, dev server | 33 passed (functional, DoD, visual, accessibility, responsive, performance) |
| Playwright, production build | 3 passed |

### Known limitations (details in `docs/VERIFICATION.md`)
Cusped trailing edges converge at first order with source panels (Joukowski −4.2 % at
400 panels; NACA unaffected). XFOIL comparison figures are published values, not runs
of XFOIL in this environment. Visual baselines are machine-specific. Chromium only.
Geometry editor and PDF export remain out of MVP scope per the PRD.

---

## Post-MVP change request — 2026-10-03

1. **Resizable panels.** Left, right and bottom panels resize by dragging their inner
   edge (`components/ResizeHandle.tsx`). Sizes live in the UI store, persist to
   `localStorage`, and are clamped (left 180–520 px, right 260–640 px, bottom 140 px
   to 70 % of the window) so the canvas can't be squeezed away. Handles are WAI-ARIA
   window splitters: arrow keys resize (Shift ×4), Home or double-click resets. Side
   handles hide in compact mode, where those panels are overlays.
2. **Distorted surface plot.** The chart drew into a fixed 640×300 viewBox stretched
   with `preserveAspectRatio="none"`, so in a wide, short panel text and lines were
   stretched ~2.7× horizontally. It now measures its container (`ResizeObserver`) and
   draws at real pixel size; tick density follows the available space.
3. **Cut-off Cp / V/U∞ / Δp buttons.** The side column is a vertical flex box in a
   fixed-height panel, so children shrank; the segmented control (which clips its
   overflow) collapsed to a few pixels. Children no longer shrink; the column scrolls.

`e2e/resize.spec.ts` covers all three (drag, persistence, clamping, reset, keyboard;
viewBox equals the rendered size; button heights). Visual baselines regenerated.
`scripts/verify.sh` green: Rust 281 · Vitest 50 · Playwright 36 + 3.

**Investigated, not changed:** the reported Cp ≈ −46 on a NACA 2023 is correct for the
inviscid model at a large effective angle (≈ 50–60°, via α or body rotation): a default
NACA 2023 gives Cp_min = −1.0 at 0° and −1.8 at 5°. The app does not yet warn that
potential flow has no stall at such angles.

---

## Post-MVP change request — forces on elementary solutions — 2026-10-03

**Question asked:** does the app show forces on elementary solutions, as vectors? It
did not: forces came only from pressure integration over bodies, and nothing drew
force arrows. All three proposed additions were implemented.

1. **Body force arrows.** Resultant at the moment reference point with dashed lift and
   drag components; one shared px-per-N/m scale for every arrow, with a key. Toggle
   *Forces* in the canvas toolbar or press `O`. Arrows follow a dragged body a frame
   ahead of the solve (re-posed like the contour).
2. **Lagally forces on elements** (`crates/flow-core/src/lagally.rs`): vortex
   `F = ρΓ(V_y, −V_x)`, source `F = −ρΛV`, doublet `F = ρκ(ê·∇)V` (derived from the
   source–sink limit), with `V` the velocity induced by everything else. Exposed as
   `Solution.elements`, shown in the Properties panel (with the "force needed to hold
   it fixed" caveat and a new `elementForces` help topic), the Data tab and the forces
   CSV.
3. **Validation.** Unit tests against closed forms and Newton's third law; against
   the exact circle-theorem force for a source and a vortex near a cylinder (both
   sides within 0.6 %, bracketing the exact value); and the momentum balance against
   pressure-integrated body forces. Results and the first-order convergence of that
   balance are recorded in `tests/numerical/TOLERANCES.md`.

Two test-construction issues surfaced along the way: a "distant" sink 10⁶ m away still
induced a measurable force, and a source–sink pair at ε = 10⁻⁵ buried a 0.25 N/m result
under ~10¹³ N/m of cancelling mutual forces. Both reference computations were rebuilt
rather than loosened.
