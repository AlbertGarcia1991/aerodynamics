# Verification report — PRD §79 Definition of Done

Every MVP criterion mapped to the test that demonstrates it. All suites green on
2026-10-03 via `scripts/verify.sh`: **Rust 294 · Vitest 50 · Playwright 39 (dev) + 3
(production build)**.

Abbreviations: `core` = `apps/web/e2e/core.spec.ts`, `dod` = `e2e/dod.spec.ts`,
`a11y` = `e2e/a11y.spec.ts`, `visual` = `e2e/visual.spec.ts`, `perf` = `e2e/perf.spec.ts`,
`prod` = `e2e/prod-smoke.spec.ts`, `analytical` = `crates/solver/tests/analytical.rs`,
`examples` = `crates/solver/tests/examples.rs`.

| # | Criterion | Evidence |
| --- | --- | --- |
| 1 | Launch the application | core *launches, solves the default scene and shows a legend*; prod *production build boots the WASM worker…* |
| 2 | Add a uniform flow | dod *DoD 2–8: add, move and edit every elementary element type* |
| 3 | Add a source | dod (as above); core *create source → move it → change strength…* |
| 4 | Add a sink | dod |
| 5 | Add a vortex | dod; core |
| 6 | Add a doublet | dod |
| 7 | Move each object | dod (numeric move, every positioned type); core (pointer drag); a11y *keyboard-only…* (arrow-key nudge) |
| 8 | Change each object's parameters | dod (primary parameter of all five types); core (source strength) |
| 9 | Visualise velocity | visual *airfoil velocity field*; core *switches fields and the legend follows* |
| 10 | Visualise streamlines | core *every example that shows streamlines actually produces them*; visual baselines |
| 11 | Visualise pressure | visual *airfoil pressure field* |
| 12 | Visualise Cp | visual *airfoil Cp field with the body selected*; core (field switch) |
| 13 | Visualise vorticity | core (field switch, legend shows "Vorticity"); flow-core `vorticity_is_zero_away_from_vortices_and_integrates_to_gamma` |
| 14 | Import a closed (x, y) geometry | core *imports a geometry file through the dialog*; geometry `import_pipeline` (9 tests), `fixture_file` |
| 15 | Generate panels | geometry `panel`, `repanel` units; core import (panel count in the scene tree) |
| 16 | Solve the geometry | core *solves a NACA 0012 airfoil…*; analytical suite (22 tests) |
| 17 | Apply a Kutta condition where applicable | core (`kutta` pill); panel-method `kutta_condition_is_satisfied_at_the_trailing_edge`, `kutta_on_a_smooth_body_raises_a_warning`; geometry `trailing_edge` (10 tests) |
| 18 | Display surface Cp | dod *DoD 18: surface Cp plot…*; visual *airfoil Cp field with the body selected* |
| 19 | Display lift | core (NACA CL 0.57–0.63); analytical `naca0012_lift_at_five_degrees_matches_the_reference_range` |
| 20 | Display drag | core (drag shown with the d'Alembert explanation) |
| 21 | Display force coefficients | core; core *a custom moment reference point changes Cm but not CL* |
| 22 | Support multiple bodies | dod *DoD 22…*; analytical tandem / side-by-side / biplane; panel-method `multi_body_coupling_is_present_and_decays_with_distance` |
| 23 | Save a simulation | core *saves and reloads a simulation* |
| 24 | Reload a simulation | core (as above); examples (every fixture deserialises in Rust) |
| 25 | Export numerical data | core *exports force results as CSV*; Vitest `csv.test.ts` (forces, surface, field, geometry, sweep) |
| 26 | Pass all analytical numerical regression tests | analytical (22) + flow-core/panel-method units; measured tolerances in `tests/numerical/TOLERANCES.md` |
| 27 | Pass the core E2E suite | `npx playwright test` — 39/39 |
| 28 | Run numerical computation through WASM | prod (worker + `.wasm` from the production bundle); Vitest *only the solver worker loads the WASM module* |
| 29 | Remain responsive while solving | perf *main-thread frames stay smooth while a 1000-panel cold solve runs* (p95 16.7 ms); core *stays responsive while solving a large body* |

## Beyond the MVP list

| PRD | Requirement | Evidence |
| --- | --- | --- |
| §27 | Configurable moment reference point | core *a custom moment reference point…* |
| §29 | Sweeps over other parameters | core *a circulation sweep on a cylinder follows Kutta–Joukowski* |
| §38 | Tablet / phone layouts | a11y *responsive layout* (820 px and 390 px, no horizontal overflow) |
| §39 | Light / dark / system themes | visual *dark theme*; a11y dark scan (theme asserted, not assumed) |
| §54 | WCAG 2.2 AA where practical | a11y — axe A/AA scans in both themes, dialogs and tabs: 0 violations; keyboard-only workflow; focus ring |
| §60 | Cancellable long operations | core *cancels a long sweep in the worker* |
| §64 | Visual regression | visual — 7 baselines at 0.4 % pixel tolerance |
| §65 | Performance benchmarks | `docs/PERFORMANCE.md`; perf; native benches |
| — | Forces on elementary solutions (Lagally) | flow-core `lagally` (9); analytical `*_near_a_cylinder_*` (circle theorem + momentum balance); `e2e/forces.spec.ts` |
| §66–68 | Module boundaries / shared contracts | Vitest `boundaries.test.ts` (with a positive control); Rust `examples.rs` + Vitest `exampleFixtures.test.ts` |

## Known limitations

* **Cusped trailing edges converge at first order.** Constant-strength source panels
  represent a near-zero-thickness lifting sliver poorly: a Joukowski section is −4.2 %
  in pressure-integrated CL at 400 panels (circulation −1.5 %). NACA sections (a wedge
  trailing edge) are unaffected. The lift-consistency diagnostic flags it in the app.
  Linear-strength vortex panels (as in XFOIL) would remove this.
* **XFOIL reference values are published figures**, not output from running XFOIL in
  this environment. The analytical comparisons (cylinder, ellipse, Kutta–Joukowski,
  Joukowski conformal map) are exact references; the XFOIL ones are secondary.
* **Visual baselines are machine-specific** (font rasterisation). Regenerate on a new
  machine or CI image with `npx playwright test e2e/visual.spec.ts --update-snapshots=all`.
* **Only Chromium is tested.** Firefox and WebKit projects can be added to
  `playwright.config.ts`; WebGL2 and module workers are required.
* **Out of MVP scope per the PRD**: geometry editor (DEC-004), PDF report export,
  boundary-layer or viscous coupling.
* Undo history and unsaved work are in memory only (a `beforeunload` prompt guards
  against accidental loss); there is no autosave.
