//! Surface quantities and aerodynamic force integration (PRD §23–§27).
//!
//! # Force convention
//!
//! Pressure acts inward on the surface, so the resultant is
//!
//! ```text
//! F = −∮ (p − p∞) n̂ ds        with n̂ the OUTWARD normal
//! ```
//!
//! Discretised over panels, using `p − p∞ = q∞·Cp`:
//!
//! ```text
//! Fx = −q∞ Σ Cp_i n_x,i L_i
//! Fy = −q∞ Σ Cp_i n_y,i L_i
//! ```
//!
//! Forces are **per unit span** (N/m), as they must be for a 2D solver
//! (PRD §69); the UI labels them accordingly.
//!
//! # Self-check
//!
//! Lift is computed twice by independent routes: pressure integration, and the
//! Kutta–Joukowski theorem `L = −ρU∞Γ` applied to the body's bound
//! circulation (negative because this crate takes `Γ > 0` counter-clockwise).
//! The two must agree; their difference is reported as a diagnostic, because a
//! sign or assembly error in the influence matrix shows up there long before it
//! becomes visible in a plot.

use crate::conditions::FlowConditions;
use aeroflow_geometry::{Panel, Polygon, Vec2};

/// Reference quantities for non-dimensionalisation (PRD §26).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ReferenceValues {
    /// Reference length `c` [m] — the chord for an airfoil.
    pub chord: f64,
    /// Reference area per unit span [m²/m] = chord by default.
    pub area: f64,
    /// Moment reference point [m].
    pub point: Vec2,
}

impl ReferenceValues {
    /// Derive defaults from the geometry: chord = maximum vertex separation,
    /// moment reference at the quarter-chord measured from the leading edge.
    pub fn from_polygon(poly: &Polygon) -> Self {
        let (i, j, chord) = poly.diameter().unwrap_or((0, 0, 1.0));
        let (a, b) = (poly.points[i], poly.points[j]);
        // The leading edge is the end furthest downstream in the *reversed*
        // sense; without knowing the flow direction yet, take the point with
        // the smaller x as the leading edge, which is right for conventional
        // geometry files.
        let (le, te) = if a.x <= b.x { (a, b) } else { (b, a) };
        let quarter = le.lerp(te, 0.25);
        Self {
            chord: if chord > 0.0 { chord } else { 1.0 },
            area: if chord > 0.0 { chord } else { 1.0 },
            point: quarter,
        }
    }
}

/// Which side of the chord line a surface point lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Surface {
    Upper,
    Lower,
}

/// One sampled point on a body surface — a panel midpoint (PRD §28).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SurfacePoint {
    pub position: Vec2,
    pub normal: Vec2,
    pub tangent: Vec2,
    pub panel_length: f64,
    /// Arc length from the first panel start.
    pub arc_length: f64,
    /// Chordwise station `x/c`, measured from the leading edge along the chord.
    pub x_over_c: f64,
    pub surface: Surface,
    /// Tangential velocity along [`SurfacePoint::tangent`] [m/s].
    pub tangential_velocity: f64,
    /// Residual normal velocity [m/s]. Should be ~0; it is the solver's own
    /// accuracy measure.
    pub normal_velocity: f64,
    pub pressure: f64,
    pub cp: Option<f64>,
    /// Solved constant source strength on this panel [m/s].
    pub source_strength: f64,
}

/// Integrated aerodynamic results for one body (PRD §24).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct BodyForces {
    /// Cartesian force per unit span [N/m].
    pub fx: f64,
    pub fy: f64,
    /// Wind-axis force per unit span [N/m].
    pub lift: f64,
    pub drag: f64,
    /// Pitching moment about [`ReferenceValues::point`] [N·m/m], **positive
    /// nose-up** — the aerodynamic convention, so a cambered section reports a
    /// negative `Cm` as in every airfoil data sheet.
    ///
    /// Lift is defined to the *left* of the freestream and the nose points
    /// upstream, so "nose-up" is a clockwise rotation whatever the flow angle.
    /// In this `+x`/`+y` frame that makes the pitching moment `−M_z`, where
    /// `M_z = Σ (r − r_ref) × dF` is the mathematical counter-clockwise moment.
    pub moment: f64,
    /// Coefficients; `None` when they cannot be normalised (`U∞ = 0` or
    /// `c = 0`), which the UI surfaces as a warning rather than showing `inf`.
    pub cl: Option<f64>,
    pub cd: Option<f64>,
    pub cm: Option<f64>,
    /// Bound circulation [m²/s], counter-clockwise positive.
    pub circulation: f64,
    /// Independent lift estimate `L = −ρU∞Γ` [N/m].
    pub lift_kutta_joukowski: f64,
    /// `|L_pressure − L_KJ| / max(|L_KJ|, q∞c)` — dimensionless agreement
    /// between the two independent lift routes. A relative error when lift is
    /// large (a spinning cylinder reaches CL ≈ 4), and an error in units of CL
    /// when lift is small, so neither case is misreported.
    pub lift_consistency: f64,
    pub reference: ReferenceValues,
}

impl BodyForces {
    pub fn zero(reference: ReferenceValues) -> Self {
        Self {
            fx: 0.0,
            fy: 0.0,
            lift: 0.0,
            drag: 0.0,
            moment: 0.0,
            cl: None,
            cd: None,
            cm: None,
            circulation: 0.0,
            lift_kutta_joukowski: 0.0,
            lift_consistency: 0.0,
            reference,
        }
    }
}

/// Chord frame used to report `x/c` and upper/lower surface.
#[derive(Debug, Clone, Copy)]
pub struct ChordFrame {
    pub leading_edge: Vec2,
    pub trailing_edge: Vec2,
    /// Contour vertex indices of the leading and trailing edge.
    pub leading_index: usize,
    pub trailing_index: usize,
    pub chord: f64,
    /// Unit vector leading edge → trailing edge.
    pub axis: Vec2,
}

impl ChordFrame {
    /// Build the chord frame from the contour's two most separated vertices.
    ///
    /// The upstream end (lower projection onto the freestream direction) is the
    /// leading edge, which makes `x/c` run with the flow regardless of how the
    /// geometry was authored or rotated.
    pub fn new(poly: &Polygon, flow_direction: Vec2) -> Self {
        let (i, j, chord) = poly.diameter().unwrap_or((0, 0, 0.0));
        let (a, b) = (poly.points[i], poly.points[j]);
        let dir = if flow_direction.norm() > 0.0 {
            flow_direction.normalized()
        } else {
            Vec2::X
        };
        let (le, te, li, ti) = if a.dot(dir) <= b.dot(dir) {
            (a, b, i, j)
        } else {
            (b, a, j, i)
        };
        Self {
            leading_edge: le,
            trailing_edge: te,
            leading_index: li,
            trailing_index: ti,
            chord: if chord > 0.0 { chord } else { 1.0 },
            axis: (te - le).normalized(),
        }
    }

    #[inline]
    pub fn station(&self, p: Vec2) -> f64 {
        (p - self.leading_edge).dot(self.axis) / self.chord
    }

    /// Surface of panel `i` of an `n`-panel counter-clockwise contour, decided
    /// by topology: the arc from the trailing edge round to the leading edge is
    /// the upper surface, the rest the lower.
    ///
    /// Unlike [`Self::surface`] this stays correct for strongly cambered or
    /// reflexed shapes, whose surfaces can both lie on one side of the straight
    /// chord line (which makes upper and lower interleave in a Cp-vs-x/c plot).
    pub fn surface_of_panel(&self, i: usize, n: usize) -> Surface {
        if n == 0 {
            return Surface::Upper;
        }
        let from_te = (i + n - self.trailing_index % n) % n;
        let te_to_le = (self.leading_index + n - self.trailing_index) % n;
        if from_te < te_to_le {
            Surface::Upper
        } else {
            Surface::Lower
        }
    }

    /// Side of the chord line. Positive cross product means the point lies to
    /// the left of leading→trailing, i.e. the upper surface. Only reliable for
    /// shapes whose surfaces do not cross the chord line; see
    /// [`Self::surface_of_panel`].
    #[inline]
    pub fn surface(&self, p: Vec2) -> Surface {
        if self.axis.cross(p - self.leading_edge) >= 0.0 {
            Surface::Upper
        } else {
            Surface::Lower
        }
    }
}

/// Build the surface-quantity table for one body.
///
/// `tangential_velocity[i]` and `normal_velocity[i]` come from the panel
/// solver, which already knows the influence coefficients at each midpoint.
#[allow(clippy::too_many_arguments)]
pub fn surface_points(
    poly: &Polygon,
    panels: &[Panel],
    source_strengths: &[f64],
    tangential_velocity: &[f64],
    normal_velocity: &[f64],
    conditions: &FlowConditions,
) -> Vec<SurfacePoint> {
    let frame = ChordFrame::new(poly, conditions.freestream());
    let mut out = Vec::with_capacity(panels.len());
    let mut arc = 0.0;
    for (i, panel) in panels.iter().enumerate() {
        let vt = tangential_velocity.get(i).copied().unwrap_or(0.0);
        let vn = normal_velocity.get(i).copied().unwrap_or(0.0);
        let speed = (vt * vt + vn * vn).sqrt();
        out.push(SurfacePoint {
            position: panel.mid,
            normal: panel.normal,
            tangent: panel.tangent,
            panel_length: panel.length,
            arc_length: arc + 0.5 * panel.length,
            x_over_c: frame.station(panel.mid),
            surface: frame.surface_of_panel(i, panels.len()),
            tangential_velocity: vt,
            normal_velocity: vn,
            pressure: conditions.pressure_at_speed(speed),
            cp: conditions.cp_at_speed(speed),
            source_strength: source_strengths.get(i).copied().unwrap_or(0.0),
        });
        arc += panel.length;
    }
    out
}

/// Integrate surface pressure into forces, moment and coefficients.
pub fn integrate_forces(
    surface: &[SurfacePoint],
    conditions: &FlowConditions,
    reference: ReferenceValues,
    circulation: f64,
) -> BodyForces {
    let q = conditions.dynamic_pressure();

    // F = −∮ (p − p∞) n̂ ds. Using the pressure difference rather than the
    // absolute pressure matters: ∮ p∞ n̂ ds is zero for a closed contour in
    // exact arithmetic, but subtracting it first avoids losing precision to
    // the ~10⁵ Pa ambient term.
    let mut fx = 0.0;
    let mut fy = 0.0;
    let mut moment = 0.0;
    for sp in surface {
        let dp = sp.pressure - conditions.pressure;
        let w = -dp * sp.panel_length;
        let dfx = w * sp.normal.x;
        let dfy = w * sp.normal.y;
        fx += dfx;
        fy += dfy;
        let r = sp.position - reference.point;
        moment += r.x * dfy - r.y * dfx;
    }

    // `moment` so far is the counter-clockwise M_z; the aerodynamic pitching
    // moment is nose-up positive, i.e. clockwise — see the field docs.
    let moment = -moment;

    let (lift, drag) = conditions.to_wind_axes(Vec2::new(fx, fy));
    let denom = q * reference.chord;
    let normalise = |v: f64| if denom > 0.0 { Some(v / denom) } else { None };

    // Kutta–Joukowski with Γ > 0 counter-clockwise gives L = −ρU∞Γ.
    let lift_kj = -conditions.density * conditions.velocity * circulation;
    let scale = lift_kj.abs().max(denom);
    let lift_consistency = if scale > 0.0 {
        (lift - lift_kj).abs() / scale
    } else {
        0.0
    };

    BodyForces {
        fx,
        fy,
        lift,
        drag,
        moment,
        cl: normalise(lift),
        cd: normalise(drag),
        cm: if denom * reference.chord > 0.0 {
            Some(moment / (denom * reference.chord))
        } else {
            None
        },
        circulation,
        lift_kutta_joukowski: lift_kj,
        lift_consistency,
        reference,
    }
}

/// Sum per-body results into scene totals (PRD §24 "TOTAL" block).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct TotalForces {
    pub fx: f64,
    pub fy: f64,
    pub lift: f64,
    pub drag: f64,
    pub moment: f64,
    pub circulation: f64,
}

pub fn total_forces(bodies: &[BodyForces]) -> TotalForces {
    let mut t = TotalForces::default();
    for b in bodies {
        t.fx += b.fx;
        t.fy += b.fy;
        t.lift += b.lift;
        t.drag += b.drag;
        t.moment += b.moment;
        t.circulation += b.circulation;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeroflow_geometry::{shapes, Panelisation};
    use std::f64::consts::PI;

    /// Build the surface table for the *analytic* cylinder solution, where
    /// `V_t = −2U sin θ` and `Cp = 1 − 4sin²θ`, and verify the integrated
    /// forces. This isolates the integration from the panel solver.
    fn analytic_cylinder_surface(
        radius: f64,
        n: usize,
        conditions: &FlowConditions,
        circulation: f64,
    ) -> (Polygon, Vec<SurfacePoint>) {
        let poly = shapes::circle(radius, n);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let u = conditions.velocity;
        let alpha = conditions.angle;
        let mut tangential = Vec::with_capacity(pz.len());
        for p in &pz.panels {
            // θ measured from the freestream direction.
            let theta = p.mid.angle() - alpha;
            // Counter-clockwise tangent speed on a cylinder with circulation:
            // V_θ = −2U sinθ + Γ/(2πa)
            tangential.push(-2.0 * u * theta.sin() + circulation / (2.0 * PI * radius));
        }
        let normal = vec![0.0; pz.len()];
        let sigma = vec![0.0; pz.len()];
        let sp = surface_points(&poly, &pz.panels, &sigma, &tangential, &normal, conditions);
        (poly, sp)
    }

    #[test]
    fn symmetric_cylinder_produces_no_lift_and_no_drag() {
        let c = FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            density: 1.0,
            pressure: 0.0,
        };
        let (poly, sp) = analytic_cylinder_surface(1.0, 720, &c, 0.0);
        let r = ReferenceValues::from_polygon(&poly);
        let f = integrate_forces(&sp, &c, r, 0.0);
        let scale = c.dynamic_pressure() * r.chord;
        assert!(f.lift.abs() / scale < 1e-9, "CL = {:?}", f.cl);
        assert!(f.drag.abs() / scale < 1e-9, "CD = {:?} (d'Alembert)", f.cd);
    }

    /// PRD §61.7: a cylinder with circulation must obey Kutta–Joukowski.
    #[test]
    fn cylinder_with_circulation_matches_kutta_joukowski() {
        let c = FlowConditions {
            velocity: 2.0,
            angle: 0.0,
            density: 1.3,
            pressure: 0.0,
        };
        let gamma = -3.5; // clockwise ⇒ positive lift
        let (poly, sp) = analytic_cylinder_surface(1.0, 2000, &c, gamma);
        let r = ReferenceValues::from_polygon(&poly);
        let f = integrate_forces(&sp, &c, r, gamma);
        let expected = -c.density * c.velocity * gamma;
        assert!(expected > 0.0, "clockwise circulation should lift");
        assert!(
            (f.lift - expected).abs() / expected < 2e-3,
            "pressure lift {} vs KJ {expected}",
            f.lift
        );
        assert!(
            f.lift_consistency < 2e-3,
            "consistency {}",
            f.lift_consistency
        );
        // Drag stays ~0: d'Alembert's paradox holds with circulation too.
        assert!(
            f.drag.abs() / (c.dynamic_pressure() * r.chord) < 1e-6,
            "CD = {:?}",
            f.cd
        );
    }

    #[test]
    fn lift_consistency_is_relative_for_high_lift_bodies() {
        // A large circulation: CL ≈ 6. The consistency metric must report the
        // relative discrepancy, not inflate it by CL.
        let c = FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            density: 1.0,
            pressure: 0.0,
        };
        let gamma = -6.0;
        let (poly, sp) = analytic_cylinder_surface(1.0, 400, &c, gamma);
        let r = ReferenceValues::from_polygon(&poly);
        let f = integrate_forces(&sp, &c, r, gamma);
        let relative = (f.lift - f.lift_kutta_joukowski).abs() / f.lift_kutta_joukowski.abs();
        assert!(
            (f.lift_consistency - relative).abs() < 1e-12,
            "{} vs {relative}",
            f.lift_consistency
        );
    }

    #[test]
    fn force_integration_is_invariant_to_the_ambient_pressure() {
        let gamma = -2.0;
        let mut a = FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            density: 1.0,
            pressure: 0.0,
        };
        let (poly, sp_a) = analytic_cylinder_surface(1.0, 400, &a, gamma);
        let r = ReferenceValues::from_polygon(&poly);
        let fa = integrate_forces(&sp_a, &a, r, gamma);

        // A realistic ambient pressure is five orders of magnitude larger than
        // the dynamic pressure; the result must not drift.
        a.pressure = 101_325.0;
        let (_, sp_b) = analytic_cylinder_surface(1.0, 400, &a, gamma);
        let fb = integrate_forces(&sp_b, &a, r, gamma);
        assert!(
            (fa.lift - fb.lift).abs() < 1e-9,
            "{} vs {}",
            fa.lift,
            fb.lift
        );
        assert!((fa.drag - fb.drag).abs() < 1e-9);
    }

    #[test]
    fn lift_rotates_into_wind_axes_with_angle_of_attack() {
        // Same physical flow, expressed at α = 30°: the lift magnitude must be
        // unchanged because it is defined perpendicular to the freestream.
        let gamma = -4.0;
        let base = FlowConditions {
            velocity: 1.5,
            angle: 0.0,
            density: 1.0,
            pressure: 0.0,
        };
        let (poly, sp) = analytic_cylinder_surface(1.0, 1200, &base, gamma);
        let r = ReferenceValues::from_polygon(&poly);
        let l0 = integrate_forces(&sp, &base, r, gamma).lift;

        let tilted = FlowConditions {
            angle: PI / 6.0,
            ..base
        };
        let (poly2, sp2) = analytic_cylinder_surface(1.0, 1200, &tilted, gamma);
        let r2 = ReferenceValues::from_polygon(&poly2);
        let f2 = integrate_forces(&sp2, &tilted, r2, gamma);
        assert!(
            (f2.lift - l0).abs() / l0.abs() < 2e-3,
            "{} vs {l0}",
            f2.lift
        );
        assert!(f2.drag.abs() / (tilted.dynamic_pressure() * r2.chord) < 1e-6);
    }

    #[test]
    fn coefficients_are_none_without_a_freestream() {
        let c = FlowConditions {
            velocity: 0.0,
            density: 1.0,
            pressure: 0.0,
            angle: 0.0,
        };
        let poly = shapes::circle(1.0, 64);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let sp = surface_points(
            &poly,
            &pz.panels,
            &vec![0.0; pz.len()],
            &vec![0.0; pz.len()],
            &vec![0.0; pz.len()],
            &c,
        );
        let f = integrate_forces(&sp, &c, ReferenceValues::from_polygon(&poly), 0.0);
        assert!(f.cl.is_none() && f.cd.is_none() && f.cm.is_none());
        // Dimensional forces remain finite.
        assert!(f.lift.is_finite() && f.drag.is_finite());
    }

    #[test]
    fn chord_frame_identifies_leading_edge_from_the_flow_direction() {
        let poly = shapes::Naca4::default().generate(120);
        let f = ChordFrame::new(&poly, Vec2::X);
        assert!(f.leading_edge.x < 0.01, "LE at {:?}", f.leading_edge);
        assert!(f.trailing_edge.x > 0.99);
        assert!((f.chord - 1.0).abs() < 1e-6);
        assert!((f.station(f.leading_edge)).abs() < 1e-9);
        assert!((f.station(f.trailing_edge) - 1.0).abs() < 1e-9);

        // Reverse the flow and the roles swap.
        let g = ChordFrame::new(&poly, -Vec2::X);
        assert!(g.leading_edge.x > 0.99);
    }

    #[test]
    fn chord_frame_separates_upper_and_lower_surfaces() {
        let poly = shapes::Naca4::default().generate(160);
        let frame = ChordFrame::new(&poly, Vec2::X);
        let upper = poly
            .points
            .iter()
            .filter(|p| frame.surface(**p) == Surface::Upper)
            .count();
        let lower = poly.points.len() - upper;
        // A symmetric section splits almost evenly.
        assert!(
            (upper as i64 - lower as i64).abs() <= 4,
            "upper {upper}, lower {lower}"
        );
        // Points with y > 0 must all classify as upper.
        for p in poly.points.iter().filter(|p| p.y > 1e-3) {
            assert_eq!(frame.surface(*p), Surface::Upper);
        }
    }

    #[test]
    fn surfaces_of_a_strongly_cambered_shape_are_split_by_arc_not_by_chord_line() {
        // A thin crescent: both surfaces curve above the straight LE→TE line,
        // so the chord-line test calls everything "upper".
        let n = 80;
        let mut pts = Vec::new();
        for k in 0..=n {
            let x = 1.0 - k as f64 / n as f64; // TE → LE along the top
            pts.push(Vec2::new(x, 0.9 * (x * (1.0 - x)) * 4.0 * 0.5 + 0.01 * (x * (1.0 - x))));
        }
        for k in 1..n {
            let x = k as f64 / n as f64; // LE → TE along the bottom
            pts.push(Vec2::new(x, 0.9 * (x * (1.0 - x)) * 4.0 * 0.5 - 0.01 * (x * (1.0 - x))));
        }
        let poly = Polygon::new(pts);
        let frame = ChordFrame::new(&poly, Vec2::X);
        let m = poly.points.len();
        let chord_line_upper = poly
            .points
            .iter()
            .filter(|p| frame.surface(**p) == Surface::Upper)
            .count();
        assert!(chord_line_upper > m - 4, "the old rule lumps both surfaces together");
        let upper = (0..m)
            .filter(|&i| frame.surface_of_panel(i, m) == Surface::Upper)
            .count();
        assert!((upper as i64 - (m - upper) as i64).abs() <= 3, "upper {upper} of {m}");
    }

    #[test]
    fn upward_force_aft_of_the_reference_point_pitches_nose_down() {
        // An upward force applied downstream of the reference point lifts the
        // tail: nose-down, so the pitching moment must be negative.
        let c = FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            density: 2.0,
            pressure: 0.0,
        };
        // Craft one panel whose outward normal points down (−y) with Cp < 0,
        // so −Cp·n̂ points up.
        let sp = vec![SurfacePoint {
            position: Vec2::new(1.0, 0.0),
            normal: Vec2::new(0.0, -1.0),
            tangent: Vec2::X,
            panel_length: 1.0,
            arc_length: 0.0,
            x_over_c: 1.0,
            surface: Surface::Lower,
            tangential_velocity: 0.0,
            normal_velocity: 0.0,
            // Cp = +1 ⇒ p − p∞ = q > 0, pushing along −n̂ = +y.
            pressure: c.pressure + c.dynamic_pressure(),
            cp: Some(1.0),
            source_strength: 0.0,
        }];
        let r = ReferenceValues {
            chord: 1.0,
            area: 1.0,
            point: Vec2::ZERO,
        };
        let f = integrate_forces(&sp, &c, r, 0.0);
        assert!(f.fy > 0.0, "fy = {}", f.fy);
        assert!(f.moment < 0.0, "moment = {} should be nose-down", f.moment);
    }

    #[test]
    fn pitching_moment_convention_is_independent_of_flow_angle() {
        // Rotate the same configuration by 90°: force, lift and moment must be
        // unchanged because the convention is defined in wind axes.
        let make = |angle: f64| {
            let c = FlowConditions {
                velocity: 1.0,
                angle,
                density: 2.0,
                pressure: 0.0,
            };
            let pos = Vec2::new(1.0, 0.0).rotate(angle);
            let normal = Vec2::new(0.0, -1.0).rotate(angle);
            let sp = vec![SurfacePoint {
                position: pos,
                normal,
                tangent: normal.perp_left(),
                panel_length: 1.0,
                arc_length: 0.0,
                x_over_c: 1.0,
                surface: Surface::Lower,
                tangential_velocity: 0.0,
                normal_velocity: 0.0,
                pressure: c.pressure + c.dynamic_pressure(),
                cp: Some(1.0),
                source_strength: 0.0,
            }];
            let r = ReferenceValues {
                chord: 1.0,
                area: 1.0,
                point: Vec2::ZERO,
            };
            integrate_forces(&sp, &c, r, 0.0)
        };
        let a = make(0.0);
        let b = make(PI / 2.0);
        assert!((a.lift - b.lift).abs() < 1e-12);
        assert!(
            (a.moment - b.moment).abs() < 1e-12,
            "{} vs {}",
            a.moment,
            b.moment
        );
        assert!(a.moment < 0.0);
    }

    #[test]
    fn totals_sum_per_body_results() {
        let r = ReferenceValues {
            chord: 1.0,
            area: 1.0,
            point: Vec2::ZERO,
        };
        let a = BodyForces {
            fx: 1.0,
            fy: 2.0,
            lift: 2.0,
            drag: 1.0,
            moment: 0.5,
            circulation: -1.0,
            ..BodyForces::zero(r)
        };
        let b = BodyForces {
            fx: -0.5,
            fy: 1.0,
            lift: 1.0,
            drag: -0.5,
            moment: 0.25,
            circulation: -0.5,
            ..BodyForces::zero(r)
        };
        let t = total_forces(&[a, b]);
        assert!((t.fx - 0.5).abs() < 1e-12);
        assert!((t.fy - 3.0).abs() < 1e-12);
        assert!((t.lift - 3.0).abs() < 1e-12);
        assert!((t.circulation + 1.5).abs() < 1e-12);
    }

    #[test]
    fn reference_values_default_to_the_quarter_chord() {
        let poly = shapes::Naca4::default().generate(120);
        let r = ReferenceValues::from_polygon(&poly);
        assert!((r.chord - 1.0).abs() < 1e-6);
        assert!(
            (r.point.x - 0.25).abs() < 1e-3,
            "ref point at {:?}",
            r.point
        );
    }
}
