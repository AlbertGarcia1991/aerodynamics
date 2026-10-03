//! # aeroflow-panel-method
//!
//! Hess–Smith constant-strength source/vortex panel method for 2D potential
//! flow around one or more closed bodies (PRD §12, §13, DEC-001).
//!
//! # Formulation
//!
//! Each of the `N` panels (across all bodies) carries an unknown constant
//! source strength `σⱼ`. Each body `b` carries **one** vortex strength `γ_b`,
//! uniform over all its panels. The unknowns satisfy:
//!
//! * `N` flow-tangency conditions, one per panel midpoint `i`:
//!
//!   ```text
//!   Σⱼ Aᵢⱼ σⱼ + Σ_b (Σ_{j∈b} Bᵢⱼ) γ_b = −V_ambient(mᵢ)·n̂ᵢ
//!   ```
//!
//!   where `Aᵢⱼ`/`Bᵢⱼ` are the normal velocities induced at midpoint `i` by a
//!   unit source / unit vortex on panel `j`, and `V_ambient` is the freestream
//!   plus every elementary element.
//!
//! * One **Kutta condition** per body whose circulation is unknown, imposed as
//!   equal and opposite tangential velocity on the two panels adjacent to the
//!   trailing edge (panels `0` and `N_b − 1` of that body):
//!
//!   ```text
//!   V_t(m₀) + V_t(m_{N_b−1}) = 0
//!   ```
//!
//! Bodies with a *known* circulation (`CirculationMode::None` or
//! `CirculationMode::Prescribed`) contribute no unknown and no Kutta row; their
//! vortex influence is precomputed into the right-hand side.
//!
//! # Multi-body coupling
//!
//! All bodies share one global matrix: every panel influences every midpoint,
//! so mutual interference is solved exactly rather than superposed (PRD §13).
//!
//! # Reuse
//!
//! [`PanelSystem::assemble`] performs the `O(N²)` influence loop and the `O(N³)`
//! LU factorisation once. [`PanelSystem::solve`] is then `O(N²)` per
//! right-hand side, which is what makes angle-of-attack sweeps interactive.

// Dense numerical kernels read most clearly as explicit index loops over
// matrices and panel arrays; the iterator rewrites clippy suggests obscure them.
#![allow(clippy::needless_range_loop)]

use aeroflow_flow_core::panel_kernel::{
    PanelKernel, SELF_SOURCE_NORMAL, SELF_SOURCE_TANGENT, SELF_VORTEX_NORMAL, SELF_VORTEX_TANGENT,
};
use aeroflow_flow_core::FlowField;
use aeroflow_geometry::{Panel, Polygon, Vec2};
use aeroflow_linalg::{condition_number_1, LinalgError, Lu, Matrix};

/// `x > 0` and finite. Spelled out because the obvious rewrite of
/// `!(x > 0.0)` as `x <= 0.0` is *false* for NaN and would accept a NaN length.
#[inline]
fn positive_finite(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Largest acceptable length ratio between neighbouring panels before a
/// warning is raised — the usual panel-method guidance is to keep it below 2–3.
pub const MAX_ADJACENT_LENGTH_RATIO: f64 = 4.0;

/// How a body's circulation is determined (PRD §12.1 "optional
/// circulation/Kutta condition").
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "mode", rename_all = "camelCase"))]
pub enum CirculationMode {
    /// Solve for the circulation that makes the flow leave the sharp trailing
    /// edge smoothly. Requires the contour to start at the trailing edge.
    Kutta,
    /// Zero circulation: a non-lifting body. The right choice for a smooth
    /// shape with no sharp edge, such as a cylinder at rest.
    None,
    /// Fixed total circulation `Γ` [m²/s], counter-clockwise positive. Models
    /// a spinning cylinder (Magnus effect) or lets the user explore lift
    /// directly.
    Prescribed { circulation: f64 },
}

/// One body as the solver sees it.
#[derive(Debug, Clone)]
pub struct BodySpec {
    /// Counter-clockwise contour; index 0 must be the trailing edge when the
    /// mode is [`CirculationMode::Kutta`].
    pub polygon: Polygon,
    pub panels: Vec<Panel>,
    pub circulation: CirculationMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PanelError {
    NoBodies,
    /// A body has fewer than three panels.
    TooFewPanels {
        body: usize,
        count: usize,
    },
    /// A panel has zero length; its index within the body is given.
    ZeroLengthPanel {
        body: usize,
        panel: usize,
    },
    Singular {
        pivot_index: usize,
    },
    NonFinite,
    Linalg(LinalgError),
}

impl std::fmt::Display for PanelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PanelError::NoBodies => write!(f, "No bodies to solve."),
            PanelError::TooFewPanels { body, count } => write!(
                f,
                "Body {} has only {count} panel(s); at least three are required.",
                body + 1
            ),
            PanelError::ZeroLengthPanel { body, panel } => write!(
                f,
                "Panel {} of body {} has zero length. Remove the duplicated point or re-panelise.",
                panel + 1,
                body + 1
            ),
            PanelError::Singular { pivot_index } => write!(
                f,
                "The panel system is singular (pivot {pivot_index}). Check for overlapping bodies or a degenerate contour."
            ),
            PanelError::NonFinite => write!(
                f,
                "The influence matrix contains non-finite values; the geometry is probably degenerate."
            ),
            PanelError::Linalg(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PanelError {}

impl From<LinalgError> for PanelError {
    fn from(e: LinalgError) -> Self {
        match e {
            LinalgError::Singular { pivot_index } => PanelError::Singular { pivot_index },
            LinalgError::NonFinite => PanelError::NonFinite,
            other => PanelError::Linalg(other),
        }
    }
}

/// Machine-readable warning codes (PRD §71).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum WarningCode {
    PoorlyConditioned,
    TangencyResidualHigh,
    NonUniformPanels,
    NetOutflowNonZero,
    KuttaWithoutSharpEdge,
    BodiesOverlap,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PanelWarning {
    pub code: WarningCode,
    pub message: String,
    /// Body index the warning concerns, if any (PRD §45: identify the object).
    pub body: Option<usize>,
}

/// Numerical diagnostics for one solve (PRD §44).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PanelDiagnostics {
    pub panel_count: usize,
    pub body_count: usize,
    /// Size of the linear system: panels plus unknown circulations.
    pub unknown_count: usize,
    /// Hager estimate of `cond₁`.
    pub condition_number: f64,
    /// `‖Mx − b‖₂ / ‖b‖₂` of the linear solve.
    pub relative_residual: f64,
    /// Largest leftover normal velocity at any panel midpoint [m/s] — the
    /// physical statement of how well flow tangency was satisfied.
    pub max_normal_velocity: f64,
    pub min_panel_length: f64,
    pub max_panel_length: f64,
    pub warnings: Vec<PanelWarning>,
}

/// Solved singularity strengths and surface velocities for one body.
#[derive(Debug, Clone)]
pub struct SolvedBody {
    pub source_strengths: Vec<f64>,
    pub vortex_strength: f64,
    /// Tangential velocity at each panel midpoint, along the panel tangent.
    pub tangential_velocity: Vec<f64>,
    /// Residual normal velocity at each midpoint; ≈ 0 when the solve is good.
    pub normal_velocity: Vec<f64>,
    /// Bound circulation `γ·perimeter` [m²/s], counter-clockwise positive.
    pub circulation: f64,
    /// `Σ σⱼ Lⱼ` — must vanish for a closed body; reported as a self-check.
    pub net_source_outflow: f64,
}

#[derive(Debug, Clone)]
pub struct PanelSolution {
    pub bodies: Vec<SolvedBody>,
    pub diagnostics: PanelDiagnostics,
}

/// Per-body bookkeeping inside the global system.
#[derive(Debug, Clone, Copy)]
struct BodyLayout {
    /// Global panel index range `[start, end)`.
    start: usize,
    end: usize,
    /// Column of this body's `γ` in the matrix, if unknown.
    gamma_column: Option<usize>,
    /// Row of this body's Kutta condition, if any.
    kutta_row: Option<usize>,
    /// Known `γ` for fixed-circulation bodies (0 otherwise).
    gamma_fixed: f64,
    perimeter: f64,
}

/// The assembled and factorised panel system for a fixed geometry.
#[derive(Debug, Clone)]
pub struct PanelSystem {
    panels: Vec<Panel>,
    layouts: Vec<BodyLayout>,
    n: usize,
    k: usize,
    matrix: Matrix,
    lu: Lu,
    /// `N × N` tangential influence of unit sources.
    at: Matrix,
    /// `N × M` tangential influence of unit vortex strength on body `b`.
    bt: Matrix,
    /// Right-hand-side contributions of the fixed circulations, length `N + K`.
    rhs_fixed: Vec<f64>,
    condition_number: f64,
    min_panel_length: f64,
    max_panel_length: f64,
    geometry_warnings: Vec<PanelWarning>,
}

impl PanelSystem {
    /// Build the influence matrix for `bodies` and factorise it.
    pub fn assemble(bodies: &[BodySpec]) -> Result<Self, PanelError> {
        if bodies.is_empty() {
            return Err(PanelError::NoBodies);
        }

        // Flatten panels and lay out the unknowns.
        let mut panels: Vec<Panel> = Vec::new();
        let mut layouts: Vec<BodyLayout> = Vec::with_capacity(bodies.len());
        let mut k = 0usize;
        for (b, body) in bodies.iter().enumerate() {
            if body.panels.len() < 3 {
                return Err(PanelError::TooFewPanels {
                    body: b,
                    count: body.panels.len(),
                });
            }
            for (j, p) in body.panels.iter().enumerate() {
                if !positive_finite(p.length) || !p.mid.is_finite() {
                    return Err(PanelError::ZeroLengthPanel { body: b, panel: j });
                }
            }
            let start = panels.len();
            panels.extend_from_slice(&body.panels);
            let end = panels.len();
            let perimeter: f64 = body.panels.iter().map(|p| p.length).sum();
            let (gamma_column, kutta_row, gamma_fixed) = match body.circulation {
                CirculationMode::Kutta => {
                    k += 1;
                    (Some(k - 1), Some(k - 1), 0.0)
                }
                CirculationMode::None => (None, None, 0.0),
                CirculationMode::Prescribed { circulation } => {
                    (None, None, circulation / perimeter)
                }
            };
            layouts.push(BodyLayout {
                start,
                end,
                gamma_column,
                kutta_row,
                gamma_fixed,
                perimeter,
            });
        }
        let n = panels.len();
        let m = bodies.len();
        let size = n + k;

        // Resolve column/row offsets now that N is known.
        for l in &mut layouts {
            l.gamma_column = l.gamma_column.map(|c| n + c);
            l.kutta_row = l.kutta_row.map(|r| n + r);
        }
        // Body index of each global panel.
        let mut body_of = vec![0usize; n];
        for (b, l) in layouts.iter().enumerate() {
            for j in l.start..l.end {
                body_of[j] = b;
            }
        }

        let mut matrix = Matrix::zeros(size, size);
        let mut at = Matrix::zeros(n, n);
        let mut bt = Matrix::zeros(n, m);
        let mut rhs_fixed = vec![0.0; size];

        // Kutta reference panels: (row, +1 weight) for first and last panel of
        // each Kutta body. Stored per global panel for the inner loop.
        let mut kutta_ref: Vec<Option<usize>> = vec![None; n];
        for l in &layouts {
            if let Some(row) = l.kutta_row {
                kutta_ref[l.start] = Some(row);
                kutta_ref[l.end - 1] = Some(row);
            }
        }

        // Single O(N²) pass filling normal and tangential influence.
        for i in 0..n {
            let pi = &panels[i];
            let n_i = pi.normal;
            let t_i = pi.tangent;
            let kutta_row_i = kutta_ref[i];

            for j in 0..n {
                let pj = &panels[j];
                let (a_n, a_t, b_n, b_t) = if i == j {
                    (
                        SELF_SOURCE_NORMAL,
                        SELF_SOURCE_TANGENT,
                        SELF_VORTEX_NORMAL,
                        SELF_VORTEX_TANGENT,
                    )
                } else {
                    let kern = PanelKernel::evaluate(pj, pi.mid);
                    let vs = kern.source_velocity(pj);
                    let vv = kern.vortex_velocity(pj);
                    (vs.dot(n_i), vs.dot(t_i), vv.dot(n_i), vv.dot(t_i))
                };

                let bj = body_of[j];
                let lj = layouts[bj];

                // Tangency row i.
                matrix[(i, j)] += a_n;
                match lj.gamma_column {
                    Some(col) => matrix[(i, col)] += b_n,
                    None => rhs_fixed[i] -= lj.gamma_fixed * b_n,
                }
                // Tangential influence, kept for surface velocities.
                at[(i, j)] = a_t;
                bt[(i, bj)] += b_t;

                // Kutta row referencing panel i (if i is a TE panel).
                if let Some(row) = kutta_row_i {
                    matrix[(row, j)] += a_t;
                    match lj.gamma_column {
                        Some(col) => matrix[(row, col)] += b_t,
                        None => rhs_fixed[row] -= lj.gamma_fixed * b_t,
                    }
                }
            }
        }

        if !matrix.is_finite() {
            return Err(PanelError::NonFinite);
        }
        let lu = Lu::factor(&matrix)?;
        let condition_number = condition_number_1(&matrix, &lu);

        let (min_panel_length, max_panel_length) =
            panels.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), p| {
                (lo.min(p.length), hi.max(p.length))
            });

        let mut geometry_warnings = Vec::new();
        // Abrupt length changes between *neighbouring* panels degrade accuracy;
        // a smooth grading (cosine clustering spans 300× at 1000 panels) does
        // not. So the criterion is the worst adjacent ratio, per body.
        for (b, l) in layouts.iter().enumerate() {
            let n_b = l.end - l.start;
            let mut worst = (1.0f64, 0usize);
            for k in 0..n_b {
                let a = panels[l.start + k].length;
                let c = panels[l.start + (k + 1) % n_b].length;
                let r = a.max(c) / a.min(c);
                if r > worst.0 {
                    worst = (r, k);
                }
            }
            if worst.0 > MAX_ADJACENT_LENGTH_RATIO {
                geometry_warnings.push(PanelWarning {
                    code: WarningCode::NonUniformPanels,
                    message: format!(
                        "Body {}: panels {} and {} differ in length by {:.1}×. Abrupt changes between neighbouring panels reduce accuracy; re-panelise with a cosine or uniform distribution.",
                        b + 1, worst.1 + 1, (worst.1 + 1) % n_b + 1, worst.0
                    ),
                    body: Some(b),
                });
            }
        }
        if condition_number > 1.0e8 {
            geometry_warnings.push(PanelWarning {
                code: WarningCode::PoorlyConditioned,
                message: format!(
                    "The panel system is poorly conditioned (cond₁ ≈ {condition_number:.2e}). Results may be inaccurate; check for very small panels or nearly touching bodies."
                ),
                body: None,
            });
        }
        for (b, body) in bodies.iter().enumerate() {
            if body.circulation == CirculationMode::Kutta {
                let te = aeroflow_geometry::trailing_edge::turn_angle_at(&body.polygon, 0);
                if te < aeroflow_geometry::SHARP_CORNER_THRESHOLD {
                    geometry_warnings.push(PanelWarning {
                        code: WarningCode::KuttaWithoutSharpEdge,
                        message: format!(
                            "Body {} has a Kutta condition but no sharp trailing edge at its first point (turn {:.1}°). The computed circulation is not physically determined; use zero or prescribed circulation.",
                            b + 1, te.to_degrees()
                        ),
                        body: Some(b),
                    });
                }
            }
        }
        // Overlap: any panel midpoint of one body inside another body.
        'outer: for (a, la) in layouts.iter().enumerate() {
            for (b, body_b) in bodies.iter().enumerate() {
                if a == b {
                    continue;
                }
                for j in la.start..la.end {
                    if body_b.polygon.contains_point(panels[j].mid) {
                        geometry_warnings.push(PanelWarning {
                            code: WarningCode::BodiesOverlap,
                            message: format!(
                                "Body {} overlaps body {}. The flow between intersecting bodies is not meaningful.",
                                a + 1, b + 1
                            ),
                            body: Some(a),
                        });
                        break 'outer;
                    }
                }
            }
        }

        Ok(Self {
            panels,
            layouts,
            n,
            k,
            matrix,
            lu,
            at,
            bt,
            rhs_fixed,
            condition_number,
            min_panel_length,
            max_panel_length,
            geometry_warnings,
        })
    }

    #[inline]
    pub fn panel_count(&self) -> usize {
        self.n
    }

    #[inline]
    pub fn unknown_count(&self) -> usize {
        self.n + self.k
    }

    #[inline]
    pub fn body_count(&self) -> usize {
        self.layouts.len()
    }

    pub fn condition_number(&self) -> f64 {
        self.condition_number
    }

    /// Solve for the given ambient flow (freestream + elementary elements).
    /// `ambient.bodies` is ignored.
    pub fn solve(&self, ambient: &FlowField) -> Result<PanelSolution, PanelError> {
        let n = self.n;
        let size = n + self.k;

        // Right-hand side.
        let mut rhs = self.rhs_fixed.clone();
        let mut v_amb_t = vec![0.0; n];
        for i in 0..n {
            let p = &self.panels[i];
            let v = ambient.ambient_velocity(p.mid, 0.0);
            rhs[i] -= v.dot(p.normal);
            v_amb_t[i] = v.dot(p.tangent);
        }
        for l in &self.layouts {
            if let Some(row) = l.kutta_row {
                rhs[row] -= v_amb_t[l.start] + v_amb_t[l.end - 1];
            }
        }

        let x = self.lu.solve(&rhs)?;
        if x.iter().any(|v| !v.is_finite()) {
            return Err(PanelError::NonFinite);
        }

        // Residual: the tangency rows of (Mx − b) are the leftover normal
        // velocities, which is exactly the physical accuracy measure we want.
        let mx = self.matrix.mul_vec(&x);
        let mut res_sq = 0.0;
        let mut rhs_sq = 0.0;
        let mut normal_velocity = vec![0.0; n];
        for r in 0..size {
            let d = mx[r] - rhs[r];
            res_sq += d * d;
            rhs_sq += rhs[r] * rhs[r];
            if r < n {
                normal_velocity[r] = d;
            }
        }
        let relative_residual = if rhs_sq > 0.0 {
            (res_sq / rhs_sq).sqrt()
        } else {
            res_sq.sqrt()
        };

        // Per-body gammas.
        let gammas: Vec<f64> = self
            .layouts
            .iter()
            .map(|l| l.gamma_column.map_or(l.gamma_fixed, |c| x[c]))
            .collect();

        // Tangential velocities.
        let mut tangential = vec![0.0; n];
        for i in 0..n {
            let mut vt = v_amb_t[i];
            let row = self.at.row(i);
            for j in 0..n {
                vt += row[j] * x[j];
            }
            for (b, g) in gammas.iter().enumerate() {
                vt += self.bt[(i, b)] * g;
            }
            tangential[i] = vt;
        }

        let mut bodies = Vec::with_capacity(self.layouts.len());
        let mut max_normal_velocity: f64 = 0.0;
        for (b, l) in self.layouts.iter().enumerate() {
            let sigma: Vec<f64> = x[l.start..l.end].to_vec();
            let net_source_outflow: f64 =
                (l.start..l.end).map(|j| x[j] * self.panels[j].length).sum();
            for v in &normal_velocity[l.start..l.end] {
                max_normal_velocity = max_normal_velocity.max(v.abs());
            }
            bodies.push(SolvedBody {
                source_strengths: sigma,
                vortex_strength: gammas[b],
                tangential_velocity: tangential[l.start..l.end].to_vec(),
                normal_velocity: normal_velocity[l.start..l.end].to_vec(),
                circulation: gammas[b] * l.perimeter,
                net_source_outflow,
            });
        }

        // Warnings for this solve.
        let mut warnings = self.geometry_warnings.clone();
        let u_ref = ambient.conditions.velocity.abs().max(
            ambient
                .elements
                .iter()
                .map(|e| e.velocity(Vec2::ZERO, 1.0).norm())
                .fold(0.0, f64::max),
        );
        if u_ref > 0.0 && max_normal_velocity / u_ref > 1e-6 {
            warnings.push(PanelWarning {
                code: WarningCode::TangencyResidualHigh,
                message: format!(
                    "Flow tangency is satisfied only to {:.2e}·U∞ at the worst panel. The linear solve lost precision; check conditioning.",
                    max_normal_velocity / u_ref
                ),
                body: None,
            });
        }
        for (b, sb) in bodies.iter().enumerate() {
            let scale = u_ref * self.layouts[b].perimeter;
            if scale > 0.0 && sb.net_source_outflow.abs() / scale > 1e-2 {
                warnings.push(PanelWarning {
                    code: WarningCode::NetOutflowNonZero,
                    message: format!(
                        "Body {} has a net source outflow of {:.2e}·U∞·perimeter. A closed body should have none; the discretisation may be too coarse.",
                        b + 1,
                        sb.net_source_outflow / scale
                    ),
                    body: Some(b),
                });
            }
        }

        Ok(PanelSolution {
            bodies,
            diagnostics: PanelDiagnostics {
                panel_count: n,
                body_count: self.layouts.len(),
                unknown_count: size,
                condition_number: self.condition_number,
                relative_residual,
                max_normal_velocity,
                min_panel_length: self.min_panel_length,
                max_panel_length: self.max_panel_length,
                warnings,
            },
        })
    }
}

/// Convenience: assemble and solve in one call.
pub fn solve(bodies: &[BodySpec], ambient: &FlowField) -> Result<PanelSolution, PanelError> {
    PanelSystem::assemble(bodies)?.solve(ambient)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeroflow_flow_core::FlowConditions;
    use aeroflow_geometry::{shapes, Panelisation};
    use std::f64::consts::PI;

    fn body(poly: Polygon, mode: CirculationMode) -> BodySpec {
        let pz = Panelisation::from_polygon(&poly).unwrap();
        BodySpec {
            polygon: poly,
            panels: pz.panels,
            circulation: mode,
        }
    }

    fn stream(u: f64, alpha: f64) -> FlowField {
        FlowField::new(FlowConditions {
            velocity: u,
            angle: alpha,
            density: 1.0,
            pressure: 0.0,
        })
    }

    #[test]
    fn empty_body_list_is_an_error() {
        assert_eq!(
            PanelSystem::assemble(&[]).unwrap_err(),
            PanelError::NoBodies
        );
    }

    #[test]
    fn too_few_panels_is_an_error() {
        let poly = Polygon::new(vec![Vec2::ZERO, Vec2::X]);
        let spec = BodySpec {
            polygon: poly,
            panels: vec![],
            circulation: CirculationMode::None,
        };
        assert!(matches!(
            PanelSystem::assemble(&[spec]),
            Err(PanelError::TooFewPanels { body: 0, count: 0 })
        ));
    }

    #[test]
    fn zero_length_panel_is_reported_with_its_index() {
        let mut poly = shapes::circle(1.0, 16);
        // Duplicating points[5] makes panel 5 (points[5] → points[6]) degenerate.
        poly.points.insert(5, poly.points[5]);
        let b = body(poly, CirculationMode::None);
        assert!(matches!(
            PanelSystem::assemble(&[b]),
            Err(PanelError::ZeroLengthPanel { body: 0, panel: 5 })
        ));
    }

    #[test]
    fn nan_panel_length_is_rejected_not_accepted() {
        let mut b = body(shapes::circle(1.0, 16), CirculationMode::None);
        b.panels[3].length = f64::NAN;
        assert!(matches!(
            PanelSystem::assemble(&[b]),
            Err(PanelError::ZeroLengthPanel { body: 0, panel: 3 })
        ));
    }

    #[test]
    fn layout_counts_unknowns_correctly() {
        let a = body(
            shapes::Naca4::default().generate(40),
            CirculationMode::Kutta,
        );
        let b = body(shapes::circle(1.0, 24), CirculationMode::None);
        let c = body(
            shapes::circle(0.5, 16),
            CirculationMode::Prescribed { circulation: 1.0 },
        );
        let na = a.panels.len();
        let sys = PanelSystem::assemble(&[a, b, c]).unwrap();
        assert_eq!(sys.panel_count(), na + 24 + 16);
        // Only the Kutta body adds an unknown.
        assert_eq!(sys.unknown_count(), na + 24 + 16 + 1);
        assert_eq!(sys.body_count(), 3);
    }

    /// Flow tangency must be satisfied to round-off at every panel.
    #[test]
    fn tangency_residual_is_at_round_off() {
        let b = body(shapes::circle(1.0, 80), CirculationMode::None);
        let sol = solve(&[b], &stream(1.0, 0.0)).unwrap();
        assert!(
            sol.diagnostics.relative_residual < 1e-12,
            "{}",
            sol.diagnostics.relative_residual
        );
        assert!(
            sol.diagnostics.max_normal_velocity < 1e-12,
            "{}",
            sol.diagnostics.max_normal_velocity
        );
        assert!(
            sol.diagnostics.warnings.is_empty(),
            "{:?}",
            sol.diagnostics.warnings
        );
    }

    /// A closed body neither emits nor absorbs fluid.
    #[test]
    fn net_source_outflow_vanishes_for_a_closed_body() {
        let b = body(shapes::circle(1.0, 64), CirculationMode::None);
        let sol = solve(&[b], &stream(1.0, 0.3)).unwrap();
        assert!(
            sol.bodies[0].net_source_outflow.abs() < 1e-10,
            "{}",
            sol.bodies[0].net_source_outflow
        );
    }

    #[test]
    fn prescribed_circulation_is_returned_exactly() {
        let gamma = -2.5;
        let b = body(
            shapes::circle(1.0, 64),
            CirculationMode::Prescribed { circulation: gamma },
        );
        let sol = solve(&[b], &stream(1.0, 0.0)).unwrap();
        assert!((sol.bodies[0].circulation - gamma).abs() < 1e-12);
    }

    #[test]
    fn kutta_condition_is_satisfied_at_the_trailing_edge() {
        let b = body(
            shapes::Naca4::default().generate(120),
            CirculationMode::Kutta,
        );
        let sol = solve(&[b], &stream(1.0, 5.0_f64.to_radians())).unwrap();
        let vt = &sol.bodies[0].tangential_velocity;
        let n = vt.len();
        assert!(
            (vt[0] + vt[n - 1]).abs() < 1e-10,
            "V_t,0 + V_t,N-1 = {}",
            vt[0] + vt[n - 1]
        );
        // Lifting at positive α: clockwise (negative) circulation.
        assert!(sol.bodies[0].circulation < 0.0);
    }

    #[test]
    fn symmetric_airfoil_at_zero_alpha_has_no_circulation() {
        let b = body(
            shapes::Naca4::default().generate(120),
            CirculationMode::Kutta,
        );
        let sol = solve(&[b], &stream(1.0, 0.0)).unwrap();
        assert!(
            sol.bodies[0].circulation.abs() < 1e-9,
            "{}",
            sol.bodies[0].circulation
        );
    }

    #[test]
    fn kutta_on_a_smooth_body_raises_a_warning() {
        let b = body(shapes::circle(1.0, 64), CirculationMode::Kutta);
        let sol = solve(&[b], &stream(1.0, 0.0)).unwrap();
        assert!(sol
            .diagnostics
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::KuttaWithoutSharpEdge && w.body == Some(0)));
    }

    #[test]
    fn overlapping_bodies_raise_a_warning() {
        let a = body(shapes::circle(1.0, 32), CirculationMode::None);
        let b = body(
            shapes::circle(1.0, 32).transformed(Vec2::new(0.5, 0.0), 0.0, 1.0),
            CirculationMode::None,
        );
        let sol = solve(&[a, b], &stream(1.0, 0.0)).unwrap();
        assert!(sol
            .diagnostics
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::BodiesOverlap));
    }

    #[test]
    fn smooth_cosine_grading_is_not_flagged_but_an_abrupt_jump_is() {
        // 1000-panel NACA: global ratio ~300×, adjacent ratio ~1 — no warning.
        let smooth = body(
            shapes::Naca4::default().generate(1000),
            CirculationMode::Kutta,
        );
        let sol = solve(&[smooth], &stream(1.0, 0.05)).unwrap();
        assert!(
            !sol.diagnostics
                .warnings
                .iter()
                .any(|w| w.code == WarningCode::NonUniformPanels),
            "{:?}",
            sol.diagnostics.warnings
        );

        // A coarse square with one side finely subdivided: a 10× jump.
        let mut pts = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)];
        for i in 1..=10 {
            pts.push(Vec2::new(1.0, i as f64 / 10.0));
        }
        pts.push(Vec2::new(0.0, 1.0));
        let jumpy = body(Polygon::new(pts), CirculationMode::None);
        let sol = solve(&[jumpy], &stream(1.0, 0.0)).unwrap();
        let w = sol
            .diagnostics
            .warnings
            .iter()
            .find(|w| w.code == WarningCode::NonUniformPanels)
            .expect("jump not flagged");
        assert_eq!(w.body, Some(0));
    }

    #[test]
    fn factorisation_is_reused_across_right_hand_sides() {
        let b = body(
            shapes::Naca4::default().generate(80),
            CirculationMode::Kutta,
        );
        let sys = PanelSystem::assemble(std::slice::from_ref(&b)).unwrap();
        for deg in [-4.0f64, 0.0, 3.0, 8.0] {
            let amb = stream(1.0, deg.to_radians());
            let reused = sys.solve(&amb).unwrap();
            let fresh = solve(std::slice::from_ref(&b), &amb).unwrap();
            assert!((reused.bodies[0].circulation - fresh.bodies[0].circulation).abs() < 1e-12);
        }
    }

    /// Two distant bodies must each behave almost as if alone, while two close
    /// bodies must influence each other — the coupling has to be real.
    #[test]
    fn multi_body_coupling_is_present_and_decays_with_distance() {
        let single = body(shapes::circle(1.0, 64), CirculationMode::None);
        let alone = solve(std::slice::from_ref(&single), &stream(1.0, 0.0)).unwrap();
        let sigma_alone = alone.bodies[0].source_strengths.clone();

        let far = body(
            shapes::circle(1.0, 64).transformed(Vec2::new(200.0, 0.0), 0.0, 1.0),
            CirculationMode::None,
        );
        let with_far = solve(&[single.clone(), far], &stream(1.0, 0.0)).unwrap();
        let diff_far: f64 = with_far.bodies[0]
            .source_strengths
            .iter()
            .zip(&sigma_alone)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        assert!(diff_far < 1e-3, "distant body changed σ by {diff_far}");

        let near = body(
            shapes::circle(1.0, 64).transformed(Vec2::new(2.5, 0.0), 0.0, 1.0),
            CirculationMode::None,
        );
        let with_near = solve(&[single, near], &stream(1.0, 0.0)).unwrap();
        let diff_near: f64 = with_near.bodies[0]
            .source_strengths
            .iter()
            .zip(&sigma_alone)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        assert!(
            diff_near > 0.05,
            "nearby body should perturb the solution: {diff_near}"
        );
        assert!(with_near.diagnostics.max_normal_velocity < 1e-10);
    }

    #[test]
    fn solution_is_invariant_under_rigid_rotation_of_the_whole_scene() {
        // Rotate the body and the freestream together: surface speeds must
        // match panel for panel.
        let poly = shapes::Naca4 {
            max_camber: 0.02,
            ..Default::default()
        }
        .generate(100);
        let a = solve(
            &[body(poly.clone(), CirculationMode::Kutta)],
            &stream(1.0, 0.1),
        )
        .unwrap();
        let rot = 0.7;
        let b = solve(
            &[body(
                poly.transformed(Vec2::ZERO, rot, 1.0),
                CirculationMode::Kutta,
            )],
            &stream(1.0, 0.1 + rot),
        )
        .unwrap();
        for (x, y) in a.bodies[0]
            .tangential_velocity
            .iter()
            .zip(&b.bodies[0].tangential_velocity)
        {
            assert!((x - y).abs() < 1e-9, "{x} vs {y}");
        }
        assert!((a.bodies[0].circulation - b.bodies[0].circulation).abs() < 1e-9);
    }

    #[test]
    fn cylinder_surface_speed_matches_the_analytic_two_u_sin_theta() {
        let n = 128;
        let b = body(shapes::circle(1.0, n), CirculationMode::None);
        let sol = solve(std::slice::from_ref(&b), &stream(1.0, 0.0)).unwrap();
        let mut rms = 0.0;
        for (i, p) in b.panels.iter().enumerate() {
            let theta = p.mid.angle();
            // Counter-clockwise tangent speed on a cylinder: V_θ = −2U sinθ.
            let expected = -2.0 * theta.sin();
            let d = sol.bodies[0].tangential_velocity[i] - expected;
            rms += d * d;
        }
        rms = (rms / n as f64).sqrt();
        assert!(rms < 2e-3, "RMS surface speed error {rms}");
        let _ = PI;
    }
}
