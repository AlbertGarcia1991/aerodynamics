# Numerical acceptance tolerances

PRD §62 requires every numerical tolerance to be **established empirically and
documented**, not asserted. This file records the values the solver actually
produces (measured on 2026-10-02 with `cargo test -p aeroflow-solver --test
analytical -- --nocapture`) next to the bound each test enforces. A regression
therefore shows up as a number moving, not merely as a failed assert.

Formulation under test: Hess–Smith constant-strength source panels + one
uniform vortex strength per body, Kutta condition `V_t,0 + V_t,N−1 = 0`,
midpoint collocation, `f64` LU with partial pivoting.

## Elementary solutions (PRD §61.1–§61.5) — `crates/flow-core`

| Case | Quantity checked | Observed | Bound |
| --- | --- | --- | --- |
| Uniform flow, α = 0 | `u − U∞`, `v` | < 1e-14 | 1e-14 |
| Source | radial speed vs `Λ/(2πr)`; tangential component | < 1e-12; < 1e-14 | 1e-12; 1e-14 |
| Source | flux through enclosing circle vs `Λ` (20 000-pt quadrature) | < 1e-8 | 1e-8 |
| Vortex | tangential speed vs `Γ/(2πr)`, counter-clockwise | < 1e-12 | 1e-12 |
| Vortex | `∮V·dl` vs `Γ` | < 1e-8 | 1e-8 |
| Doublet + stream | stagnation at `(±a, 0)`; crown speed `2U`; wall-normal flow | < 1e-12 | 1e-12 |
| Rankine oval | stagnation points at `x = ±√(b² + Λb/πU)` | < 1e-12 | 1e-12 |
| All elements | `∇φ = V`, `(∂ψ/∂y, −∂ψ/∂x) = V` by central differences (h = 1e-6) | < 1e-5 | 1e-5 |
| Panel kernel | source & vortex panel velocity vs 400 000-pt quadrature | < 1e-7 rel. | 1e-7 |

## Cylinder (PRD §61.6, §81) — core acceptance case

| Panels | `Cp` RMS vs `1 − 4sin²θ` at midpoints | Bound |
| --- | --- | --- |
| 40 | 1.27e-15 | 1e-12 |
| 80 | 2.52e-15 | 1e-12 |
| 120 | 3.16e-15 | 5e-3 (primary test), 1e-12 (exactness test) |
| 160 | 4.03e-15 | 1e-12 |

**Finding.** On a regular N-gon the influence matrix is circulant and the
discrete solution reproduces the analytic surface speed at every panel midpoint
to round-off, for every N tested. This is a property of the formulation, not a
coincidence, and it means the cylinder is a *correctness* test but not a
*convergence* test. Convergence is measured on the ellipse below.

| Quantity (N = 120–160) | Observed | Bound |
| --- | --- | --- |
| `CL`, `CD` at rest | < 1e-9 | 1e-9 |
| Front-midpoint `Cp` vs `1 − 4sin²(π/N)` | < 1e-9 | 1e-9 |
| Crown `|V_t|` vs `2U` (N = 160) | < 5e-3 | 5e-3 |
| Off-body velocity vs doublet solution, 6 probe points (N = 160) | 5.2e-3 | 1e-2 |

The off-body error is dominated by the probe 0.27 radii from the wall;
constant-strength panels are least accurate within a few panel lengths of the
surface. The renderer masks a band of 0.75 panel lengths for this reason.

## Ellipse (a/b = 2) — convergence of the surface speed

Exact: `V = U(a + b)|sin t| / √(a² sin² t + b² cos² t)`.

| Panels | RMS error in `|V_t|` | Ratio to previous |
| --- | --- | --- |
| 40 | 6.19e-4 | — |
| 80 | 1.43e-4 | 4.3 |
| 160 | 3.42e-5 | 4.2 |

Bound: each doubling must cut the error below 0.7× (observed ≈ 0.23×), and
`N = 160` must be below 5e-3. **The method is second-order on smooth bodies.**

## Cylinder with circulation (PRD §61.7) — Magnus / Kutta–Joukowski

Prescribed `Γ`, N = 160, `ρ = U = a = 1`.

| Quantity | Observed | Bound |
| --- | --- | --- |
| Lift vs `−ρU∞Γ` (Γ = −3) | 2.98719 vs 3 (−0.43 %) | 0.5 % |
| `Cp` RMS vs `1 − (−2 sin θ + Γ/2πa)²` (Γ = −2) | 3.88e-3 | 5e-3 |
| lift consistency `|L_p − L_KJ| / max(|L_KJ|, q∞c)` | 4.3e-3 | 2e-2 |
| `CD` | < 1e-6 | 1e-6 |
| Stagnation-point angles vs `asin(Γ/4πUa)` (N = 200) | < 0.05 rad | 0.05 rad |
| Circulation sweep, 5 points Γ ∈ [−4, 4]: lift vs KJ | < 1 % | 1 % |

With circulation the midpoint solution is no longer exact (the uniform vortex
sheet on chords differs from one on the circle at O(1/N²)), hence the finite
`Cp` RMS. The lift-consistency diagnostic reports exactly this discrepancy and
the solver warns above 1e-2.

*Revised 2026-10-03:* the diagnostic was originally normalised by `q∞c`, which
inflates the apparent error by a factor of CL on high-lift bodies (this case has
CL = 3, so 0.43 % relative read as 1.28 %; the Magnus example, CL = 4, tripped the
1 % warning at −0.43 % actual error). It is now `|ΔL| / max(|L_KJ|, q∞c)`:
unchanged for CL < 1, a true relative error above.

## Airfoils (PRD §61.8, §82) — comparison with XFOIL (inviscid) and theory

N = 160, cosine chordwise stations, closed trailing edge (a 16.5° wedge, not a
cusp).

| Case | Quantity | Observed | Reference | Bound |
| --- | --- | --- | --- | --- |
| NACA 0012, α = 5° | `CL` | 0.6025 | XFOIL ≈ 0.600; thin-airfoil 2πα = 0.548 | 0.57–0.63 |
| NACA 0012, α = 5° | `CD` | 2.2e-4 | 0 (d'Alembert) | 5e-3 |
| NACA 0012, α = 5° | `Cm_c/4` | −0.0065 | ≈ 0 (symmetric) | ±0.01 |
| NACA 0012, α = 5° | lift consistency | < 1e-2 | — | 1e-2 |
| NACA 0012 | `dCL/dα` from α = ±4° | 6.907 /rad | XFOIL ≈ 6.9; 2π = 6.283 | 6.5–7.3 |
| NACA 0012 | `CL(−α) = −CL(α)` | < 1e-9 | exact | 1e-9 |
| NACA 2412, α = 0° | `CL` | 0.2554 | XFOIL ≈ 0.25 | 0.22–0.29 |
| NACA 2412, α = 0° | `Cm_c/4` | −0.0543 | XFOIL ≈ −0.053 | −0.08 to −0.03 |
| NACA 2412 | zero-lift angle | −2.12° | thin-airfoil −2.1°, XFOIL ≈ −2.2° | −2.6° to −1.7° |
| Any | `CL` invariance under ρ×2, U×3, c×2; `L` × 36 | < 1e-6; < 1e-3 | exact | 1e-6; 1e-3 |

**Sign convention note (found by this suite).** `Cm` is reported with the
aerodynamic nose-up-positive convention. Because lift is defined to the left of
the freestream and the nose is upstream, nose-up is a *clockwise* rotation for
every flow angle, so `Cm = −M_z/(q∞c²)` where `M_z` is the mathematical
counter-clockwise moment. The first run reported `+0.0543` for the NACA 2412 —
right magnitude, wrong sign — which is how the convention came to be pinned down
and tested for flow-angle independence.

## Joukowski section (true cusp) — documented first-order behaviour

Exact: `CL = 8πR sin(α + β)/chord`, `R = √((c+dx)² + dy²)`, `β = atan2(dy, c+dx)`,
with `dx = 0.08`, `dy = 0.05`, `c = 1` (exact `CL(0°) = 0.3124`).

| Panels | `CL` by pressure integration | `CL` by circulation (−Γ/q∞c) |
| --- | --- | --- |
| 100 | 0.2771 (−11.3 %) | 0.3005 (−3.8 %) |
| 200 | 0.2904 (−7.1 %) | 0.3048 (−2.4 %) |
| 400 | 0.2993 (−4.2 %) | 0.3078 (−1.5 %) |
| 800 | 0.3049 (−2.4 %) | 0.3098 (−0.8 %) |

Bounds: pressure-lift error strictly decreasing over N = 100/200/400; at N = 400
pressure `CL` within 5 % and circulation `CL` within 2 % (2.5 % at α = 4°).

**Why this case is harder than the NACA sections (PRD §61.8 asks for this to be
documented).** A Joukowski section has a true cusp: its thickness vanishes like
`s^{3/2}` towards the trailing edge, so within ~1 % of chord the upper and lower
surfaces are closer together than their own panel length (5e-6 apart at 0.12 %
chord in the test geometry). A *source* distribution cannot represent a
near-zero-thickness lifting sliver well — that needs vortex or doublet panels —
so the pressure-integrated lift converges only at first order, while the
circulation fixed by the Kutta condition (a global quantity) converges faster.
A NACA 4-digit section with the closed-TE coefficient is a 16.5° wedge, not a
cusp, which is why it reaches 0.4 % at N = 160.

Trimming the sliver was tried and rejected: the retained points on the two
surfaces are not symmetric, so the new trailing-edge bisector tilts and the
Kutta condition responds with up to −44 % lift for a 2 % chord trim. Cambered
lift is far more sensitive to trailing-edge *direction* than to chord.

Consequences built into the product: the `lift_consistency` diagnostic exposes
the discrepancy on every solve, the solver warns when it exceeds 1 % of `q∞c`
and suggests more panels, and the help text states that imported cusped
sections need ≥ 400 panels. A linear-strength vortex-panel formulation (as in
XFOIL) would remove the limitation and is recorded as future work.

## Multiple bodies (PRD §13)

N = 100–120 per body.

| Configuration | Observed | Bound |
| --- | --- | --- |
| Tandem cylinders, gap 1 radius: drag on front / rear | −0.3472 / +0.3472 | front < −1e-3, rear > 1e-3, sum < 1e-9 |
| Tandem: lift on each | < 1e-9 | 1e-9 |
| Side-by-side cylinders, gap 1 radius: lift on upper / lower | −0.6608 / +0.6608 | upper < −1e-3, lower > 1e-3, sum < 1e-9 |
| Biplane of NACA 0012 at α = 4°, gap 1 chord: `CL` isolated / upper / lower | 0.4821 / 0.3681 / 0.4406 | each < isolated; sum < 2× isolated |
| Distant second body (200 radii): change in σ | < 1e-3 | 1e-3 |
| Rigid rotation of body + freestream: `V_t` per panel | < 1e-9 | 1e-9 |

Tandem bodies repel (slow, high-pressure gap); side-by-side bodies attract
(fast, low-pressure gap); stacked wings lose lift to each other's downwash. All
three signs are the classical potential-flow results, and both symmetric pairs
cancel to round-off, confirming that the global multi-body matrix is assembled
without bias.

## Linear algebra

| Check | Observed | Bound |
| --- | --- | --- |
| Random diagonally dominant 120×120: relative residual | < 1e-13 | 1e-13 |
| Flow-tangency residual on an 80-panel cylinder | < 1e-12 | 1e-12 |
| Hager `cond₁` of `diag(1, 1e-6)` (true 1e6) | ≤ 1e6, > 1e5 | lower bound by construction |
| Hager `cond₁` of Hilbert H₃, H₅, H₇ | strictly increasing; H₇ > 1e7 | — |

## Rendering approximations (never used for reported numbers)

| Approximation | Measured effect | Bound |
| --- | --- | --- |
| Far-field point-singularity substitution at 12× size ratio | relative velocity error < 1e-2 outside 1.3 radii | 1e-2 |
| Vortex core softening `r² → r² + a²` | `∫ω dA` recovers `Γ` to 2e-4 | 2e-4 |
| Near-wall mask of 0.75 panel lengths | hides log-singular band at panel vertices; nearest unmasked `Cp` on a 0.1-cell grid −2.375 vs exact −2.33 at r = 1.1 | — |

## Forces on singularities — Lagally theorem (added 2026-10-03)

`crates/flow-core/src/lagally.rs` (9 tests) and `crates/solver/tests/analytical.rs`.

| Check | Observed | Bound |
| --- | --- | --- |
| Vortex in a uniform stream vs. `ρU∞|Γ|`, perpendicular to the flow | < 1e-12 | 1e-12 |
| Source / sink in a stream vs. `∓ρΛU∞` | < 1e-12 | 1e-12 |
| Two sources, two vortices: Newton's third law | < 1e-12 | 1e-12 |
| Two sources attract with `ρΛ_AΛ_B/(2πd)` | < 1e-12 | 1e-12 |
| Doublet in a uniform stream (the analytic cylinder) | < 1e-6 | 1e-6 |
| Doublet vs. source–sink pair limit (ε = 1e-3, external field only) | < 1e-4 rel. | 1e-4 |

**Against an exact solution and against the panel method** — still fluid, a
non-circulating unit cylinder, N = 240 panels. Exact values from the Milne-Thomson
circle theorem (images at `a²/c` and at the centre):

| Singularity | Lagally on the singularity | Pressure force on the cylinder | Exact | Bound |
| --- | --- | --- | --- | --- |
| Source Λ = 3 at (2, 0.6) | 0.24637 (+0.56 %) | 0.24380 (−0.49 %) | 0.24500 | 1 % each |
| Vortex Γ = 4 at (−1.8, 1) | 0.46059 (+0.56 %) | 0.45576 (−0.50 %) | 0.45803 | 1 % each |
| Doublet κ = 2 at (2.2, −0.4) | balance within 1.3 % | | — | 2 % |

The two sides are independent computations — the velocity induced at a point by
the panels vs. surface pressure integrated over them — and they **bracket** the
exact value from opposite sides. Directions are exact (towards the centre for the
source, radial for the vortex). The momentum imbalance converges at **first order**:
5.0 % at 60 panels, 1.24 % at 240 (2.0 % at the UI's default 120, asserted < 3 % in
`e2e/forces.spec.ts`). For comparison, the isolated-body results above converge at
second order; the extra error here comes from resolving a strong, localised
disturbance on the side of the cylinder facing the singularity.
