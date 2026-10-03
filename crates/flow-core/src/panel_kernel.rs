//! Analytic influence of constant-strength source and vortex panels.
//!
//! This is the one place the boundary-element mathematics lives. It is in
//! `flow-core` rather than in `panel-method` because a constant-strength panel
//! *is* an elementary analytic solution — and placing it here is what keeps the
//! crate graph acyclic: field evaluation needs panel contributions, while the
//! panel solver needs elementary contributions for its right-hand side.
//!
//! # Derivation
//!
//! Work in the panel's local frame: `ξ` along the tangent from the panel start,
//! `η` along the *left* normal. For a counter-clockwise body contour the
//! exterior lies at `η < 0`.
//!
//! A unit-strength source sheet (`σ = 1`, strength per unit length) gives
//!
//! ```text
//! u_ξ = (1/2π)·ln(r₁/r₂)
//! u_η = (1/2π)·β
//! ```
//!
//! where `r₁`, `r₂` are the distances to the panel's start and end points and
//! `β = θ₂ − θ₁` is the angle the panel subtends at the field point:
//!
//! ```text
//! β = atan2( η·L , ξ(ξ−L) + η² )
//! ```
//!
//! A unit-strength vortex sheet (`γ = 1`) is the *same field rotated by +90°*:
//!
//! ```text
//! u_ξ = −(1/2π)·β
//! u_η =  (1/2π)·ln(r₁/r₂)
//! ```
//!
//! That symmetry is why one geometric kernel yields both influence
//! coefficients, and it halves the work of assembling the influence matrix.
//!
//! # Self-influence
//!
//! At a panel's own midpoint approached from the exterior (`η → 0⁻`,
//! `0 < ξ < L`): `ln(r₁/r₂) = 0` and `β = −π`. Hence
//!
//! ```text
//! source:  V·n̂ = +1/2      V·t̂ = 0
//! vortex:  V·n̂ = 0         V·t̂ = +1/2
//! ```
//!
//! with `n̂` the outward normal (`= −η̂`). The vortex result is the familiar
//! `γ/2` tangential-velocity jump across a vortex sheet, split symmetrically.

use aeroflow_geometry::{Panel, Vec2};
use std::f64::consts::PI;

const INV_2PI: f64 = 1.0 / (2.0 * PI);

/// The two scalars from which every panel influence coefficient follows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelKernel {
    /// `(1/2π)·ln(r₁/r₂)`.
    pub log_term: f64,
    /// `(1/2π)·β`, with `β` the subtended angle.
    pub angle_term: f64,
}

/// Self-influence constants, exact in the limit `η → 0⁻`.
pub const SELF_SOURCE_NORMAL: f64 = 0.5;
pub const SELF_SOURCE_TANGENT: f64 = 0.0;
pub const SELF_VORTEX_NORMAL: f64 = 0.0;
pub const SELF_VORTEX_TANGENT: f64 = 0.5;

impl PanelKernel {
    /// Evaluate the kernel for `panel` at the global point `p`.
    ///
    /// Deliberately free of `sqrt`: `ln(r₁/r₂) = ½·ln(r₁²/r₂²)`. The influence
    /// matrix evaluates this `O(N²)` times per solve and the field evaluator
    /// `O(N·M)` times per frame, so one `ln` and one `atan2` per call is the
    /// budget.
    #[inline]
    pub fn evaluate(panel: &Panel, p: Vec2) -> Self {
        let d = p - panel.start;
        let xi = d.dot(panel.tangent);
        let eta = d.dot(panel.tangent.perp_left());
        Self::from_local(xi, eta, panel.length)
    }

    /// Evaluate from already-projected local coordinates.
    #[inline]
    pub fn from_local(xi: f64, eta: f64, length: f64) -> Self {
        let xi_l = xi - length;
        let eta2 = eta * eta;
        let r1_sq = xi * xi + eta2;
        let r2_sq = xi_l * xi_l + eta2;

        // Field point sitting exactly on an endpoint: `ln` would diverge.
        // Clamp to a floor relative to the panel length so the result stays
        // finite and the sign stays meaningful.
        let floor = (length * length * 1e-24).max(f64::MIN_POSITIVE);
        let r1_sq = r1_sq.max(floor);
        let r2_sq = r2_sq.max(floor);

        let log_term = 0.5 * (r1_sq / r2_sq).ln() * INV_2PI;

        // The subtended angle. On the panel line (η == 0) `atan2` would depend
        // on the sign of a zero, so resolve the limit explicitly: approaching
        // from the exterior (η → 0⁻) gives −π on the panel and 0 on its
        // extensions.
        let angle_term = if eta == 0.0 {
            if xi > 0.0 && xi < length {
                -PI * INV_2PI
            } else {
                0.0
            }
        } else {
            (eta * length).atan2(xi * xi_l + eta2) * INV_2PI
        };

        Self {
            log_term,
            angle_term,
        }
    }

    /// Velocity from a unit-strength **source** panel, in local components
    /// `(ξ, η)`.
    #[inline]
    pub fn source_local(&self) -> (f64, f64) {
        (self.log_term, self.angle_term)
    }

    /// Velocity from a unit-strength **vortex** panel, in local components
    /// `(ξ, η)` — the source field rotated by +90°.
    #[inline]
    pub fn vortex_local(&self) -> (f64, f64) {
        (-self.angle_term, self.log_term)
    }

    /// Global velocity from a unit-strength source panel.
    #[inline]
    pub fn source_velocity(&self, panel: &Panel) -> Vec2 {
        let (a, b) = self.source_local();
        panel.to_global_vector(a, b)
    }

    /// Global velocity from a unit-strength vortex panel.
    #[inline]
    pub fn vortex_velocity(&self, panel: &Panel) -> Vec2 {
        let (a, b) = self.vortex_local();
        panel.to_global_vector(a, b)
    }
}

/// The two line integrals needed for `φ` and `ψ` of a unit-strength panel.
///
/// ```text
/// I₁ = ∫ ln r ds = ξ·ln r₁ − (ξ−L)·ln r₂ − L + η·β
/// I₂ = ∫ θ   ds = ξ·θ₁ − (ξ−L)·θ₂ + η·ln(r₁/r₂)
/// ```
///
/// Then, with the `1/2π` folded in:
///
/// ```text
/// source:  φ = σ·I₁/2π,   ψ =  σ·I₂/2π
/// vortex:  φ = γ·I₂/2π,   ψ = −γ·I₁/2π
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelPotentials {
    /// `I₁/2π`.
    pub i1: f64,
    /// `I₂/2π`.
    pub i2: f64,
}

impl PanelPotentials {
    pub fn evaluate(panel: &Panel, p: Vec2) -> Self {
        let d = p - panel.start;
        let xi = d.dot(panel.tangent);
        let eta = d.dot(panel.tangent.perp_left());
        let l = panel.length;
        let xi_l = xi - l;
        let eta2 = eta * eta;

        let floor = (l * l * 1e-24).max(f64::MIN_POSITIVE);
        let r1_sq = (xi * xi + eta2).max(floor);
        let r2_sq = (xi_l * xi_l + eta2).max(floor);
        let ln_r1 = 0.5 * r1_sq.ln();
        let ln_r2 = 0.5 * r2_sq.ln();
        let log_ratio = ln_r1 - ln_r2;

        let (theta1, theta2, beta) = if eta == 0.0 {
            if xi > 0.0 && xi < l {
                // Exterior limit η → 0⁻.
                (-PI, 0.0, -PI)
            } else {
                (0.0, 0.0, 0.0)
            }
        } else {
            let t1 = eta.atan2(xi);
            let t2 = eta.atan2(xi_l);
            (t1, t2, t2 - t1)
        };

        let i1 = xi * ln_r1 - xi_l * ln_r2 - l + eta * beta;
        let i2 = xi * theta1 - xi_l * theta2 + eta * log_ratio;

        Self {
            i1: i1 * INV_2PI,
            i2: i2 * INV_2PI,
        }
    }

    #[inline]
    pub fn source_potential(&self) -> f64 {
        self.i1
    }
    #[inline]
    pub fn source_stream_function(&self) -> f64 {
        self.i2
    }
    #[inline]
    pub fn vortex_potential(&self) -> f64 {
        self.i2
    }
    #[inline]
    pub fn vortex_stream_function(&self) -> f64 {
        -self.i1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> Panel {
        Panel::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0))
    }

    /// Numerically integrate point singularities along the panel and compare
    /// against the closed form. This is the single most important test in the
    /// crate: the whole solver rests on these two formulas.
    fn quadrature_velocity(panel: &Panel, p: Vec2, n: usize, vortex: bool) -> Vec2 {
        let ds = panel.length / n as f64;
        let mut v = Vec2::ZERO;
        for i in 0..n {
            let s = (i as f64 + 0.5) * ds;
            let q = panel.start + panel.tangent * s;
            let r = p - q;
            let r2 = r.norm_sq();
            if r2 <= 0.0 {
                continue;
            }
            // Unit source: Λ/(2π)·r̂/r with Λ = ds. Unit vortex: Γ = ds, CCW.
            v = v + if vortex {
                Vec2::new(-r.y, r.x) * (ds * INV_2PI / r2)
            } else {
                r * (ds * INV_2PI / r2)
            };
        }
        v
    }

    #[test]
    fn source_panel_matches_quadrature() {
        let pl = panel();
        for p in [
            Vec2::new(1.0, 0.5),
            Vec2::new(-1.0, 0.3),
            Vec2::new(3.0, -0.8),
            Vec2::new(0.7, -2.0),
            Vec2::new(1.0, 1e-3),
        ] {
            let exact = PanelKernel::evaluate(&pl, p).source_velocity(&pl);
            let num = quadrature_velocity(&pl, p, 400_000, false);
            assert!(
                (exact - num).norm() < 1e-7 * (1.0 + num.norm()),
                "at {p:?}: exact {exact:?} vs quadrature {num:?}"
            );
        }
    }

    #[test]
    fn vortex_panel_matches_quadrature() {
        let pl = panel();
        for p in [
            Vec2::new(1.0, 0.5),
            Vec2::new(-1.0, 0.3),
            Vec2::new(3.0, -0.8),
            Vec2::new(0.7, -2.0),
        ] {
            let exact = PanelKernel::evaluate(&pl, p).vortex_velocity(&pl);
            let num = quadrature_velocity(&pl, p, 400_000, true);
            assert!(
                (exact - num).norm() < 1e-7 * (1.0 + num.norm()),
                "at {p:?}: exact {exact:?} vs quadrature {num:?}"
            );
        }
    }

    #[test]
    fn vortex_field_is_the_source_field_rotated_by_ninety_degrees() {
        let pl = Panel::new(Vec2::new(-0.3, 0.2), Vec2::new(1.1, 0.9));
        let p = Vec2::new(0.6, -0.4);
        let k = PanelKernel::evaluate(&pl, p);
        let s = k.source_velocity(&pl);
        let v = k.vortex_velocity(&pl);
        assert!((v - s.perp_left()).norm() < 1e-14, "{s:?} vs {v:?}");
    }

    #[test]
    fn self_influence_matches_the_documented_limits() {
        let pl = panel();
        // Approach the midpoint from the exterior (η < 0 for a CCW contour).
        let k = PanelKernel::from_local(pl.length / 2.0, -1e-12, pl.length);
        let (st, sn_eta) = k.source_local();
        let (vt, vn_eta) = k.vortex_local();
        // n̂ = −η̂, so V·n̂ = −u_η.
        assert!(
            (st - SELF_SOURCE_TANGENT).abs() < 1e-9,
            "source tangent {st}"
        );
        assert!(
            (-sn_eta - SELF_SOURCE_NORMAL).abs() < 1e-9,
            "source normal {}",
            -sn_eta
        );
        assert!(
            (vt - SELF_VORTEX_TANGENT).abs() < 1e-9,
            "vortex tangent {vt}"
        );
        assert!(
            (-vn_eta - SELF_VORTEX_NORMAL).abs() < 1e-9,
            "vortex normal {}",
            -vn_eta
        );
    }

    #[test]
    fn exactly_on_the_panel_resolves_to_the_exterior_limit() {
        let pl = panel();
        let k = PanelKernel::from_local(1.0, 0.0, pl.length);
        assert!((k.angle_term * 2.0 * PI + PI).abs() < 1e-12);
        assert!(k.log_term.abs() < 1e-12);
        // On the panel's extension the subtended angle is zero.
        let k = PanelKernel::from_local(5.0, 0.0, pl.length);
        assert!(k.angle_term.abs() < 1e-15);
    }

    #[test]
    fn normal_velocity_jumps_by_sigma_across_a_source_panel() {
        let pl = panel();
        let h = 1e-8;
        let above = PanelKernel::from_local(1.0, h, pl.length).source_local().1;
        let below = PanelKernel::from_local(1.0, -h, pl.length).source_local().1;
        assert!(
            (above - below - 1.0).abs() < 1e-6,
            "jump = {}",
            above - below
        );
    }

    #[test]
    fn tangential_velocity_jumps_by_gamma_across_a_vortex_panel() {
        let pl = panel();
        let h = 1e-8;
        let above = PanelKernel::from_local(1.0, h, pl.length).vortex_local().0;
        let below = PanelKernel::from_local(1.0, -h, pl.length).vortex_local().0;
        // V_t(below) − V_t(above) = γ for CCW-positive circulation.
        assert!(
            (below - above - 1.0).abs() < 1e-6,
            "jump = {}",
            below - above
        );
    }

    #[test]
    fn far_field_reduces_to_a_point_singularity_of_equal_total_strength() {
        let pl = panel();
        let total = pl.length; // ∫σ ds with σ = 1
        let p = Vec2::new(500.0, 300.0);
        let k = PanelKernel::evaluate(&pl, p);

        let r = p - pl.mid;
        let point_source = r * (total * INV_2PI / r.norm_sq());
        assert!((k.source_velocity(&pl) - point_source).norm() / point_source.norm() < 1e-5);

        let point_vortex = Vec2::new(-r.y, r.x) * (total * INV_2PI / r.norm_sq());
        assert!((k.vortex_velocity(&pl) - point_vortex).norm() / point_vortex.norm() < 1e-5);
    }

    #[test]
    fn potentials_gradient_reproduces_the_velocity() {
        let pl = Panel::new(Vec2::new(0.1, -0.2), Vec2::new(1.4, 0.7));
        let h = 1e-6;
        for p in [
            Vec2::new(0.9, -1.2),
            Vec2::new(-0.8, 0.9),
            Vec2::new(2.4, 0.4),
        ] {
            let grad = |f: &dyn Fn(Vec2) -> f64| {
                Vec2::new(
                    (f(p + Vec2::new(h, 0.0)) - f(p - Vec2::new(h, 0.0))) / (2.0 * h),
                    (f(p + Vec2::new(0.0, h)) - f(p - Vec2::new(0.0, h))) / (2.0 * h),
                )
            };
            let src_phi = |q: Vec2| PanelPotentials::evaluate(&pl, q).source_potential();
            let v = PanelKernel::evaluate(&pl, p).source_velocity(&pl);
            assert!((grad(&src_phi) - v).norm() < 1e-5, "source ∇φ at {p:?}");

            let vtx_phi = |q: Vec2| PanelPotentials::evaluate(&pl, q).vortex_potential();
            let v = PanelKernel::evaluate(&pl, p).vortex_velocity(&pl);
            assert!((grad(&vtx_phi) - v).norm() < 1e-5, "vortex ∇φ at {p:?}");
        }
    }

    #[test]
    fn stream_function_derivatives_reproduce_the_velocity() {
        let pl = Panel::new(Vec2::new(0.1, -0.2), Vec2::new(1.4, 0.7));
        let h = 1e-6;
        // Stay off the branch cut of the source stream function.
        for p in [Vec2::new(0.9, -1.2), Vec2::new(2.4, 0.4)] {
            for vortex in [false, true] {
                let psi = |q: Vec2| {
                    let pp = PanelPotentials::evaluate(&pl, q);
                    if vortex {
                        pp.vortex_stream_function()
                    } else {
                        pp.source_stream_function()
                    }
                };
                let u = (psi(p + Vec2::new(0.0, h)) - psi(p - Vec2::new(0.0, h))) / (2.0 * h);
                let v = -(psi(p + Vec2::new(h, 0.0)) - psi(p - Vec2::new(h, 0.0))) / (2.0 * h);
                let k = PanelKernel::evaluate(&pl, p);
                let exact = if vortex {
                    k.vortex_velocity(&pl)
                } else {
                    k.source_velocity(&pl)
                };
                assert!(
                    (Vec2::new(u, v) - exact).norm() < 1e-5,
                    "vortex={vortex} at {p:?}: ({u}, {v}) vs {exact:?}"
                );
            }
        }
    }

    #[test]
    fn i1_matches_the_closed_form_at_the_midpoint() {
        let pl = panel();
        let l = pl.length;
        let pp = PanelPotentials::evaluate(&pl, pl.mid);
        // ∫₀ᴸ ln|L/2 − s| ds = L·ln(L/2) − L
        let expected = (l * (l / 2.0).ln() - l) * INV_2PI;
        assert!((pp.i1 - expected).abs() < 1e-12, "{} vs {expected}", pp.i1);
    }

    #[test]
    fn potentials_are_finite_at_panel_endpoints() {
        let pl = panel();
        for p in [pl.start, pl.end, pl.mid] {
            let pp = PanelPotentials::evaluate(&pl, p);
            assert!(pp.i1.is_finite() && pp.i2.is_finite(), "{p:?} -> {pp:?}");
            let k = PanelKernel::evaluate(&pl, p);
            assert!(k.log_term.is_finite() && k.angle_term.is_finite());
        }
    }

    #[test]
    fn kernel_is_invariant_under_rigid_motion() {
        let a = Panel::new(Vec2::new(0.0, 0.0), Vec2::new(1.5, 0.0));
        let p = Vec2::new(0.4, -0.9);
        let ka = PanelKernel::evaluate(&a, p);

        let angle = 0.83;
        let shift = Vec2::new(-3.0, 2.0);
        let b = Panel::new(a.start.rotate(angle) + shift, a.end.rotate(angle) + shift);
        let kb = PanelKernel::evaluate(&b, p.rotate(angle) + shift);

        assert!((ka.log_term - kb.log_term).abs() < 1e-14);
        assert!((ka.angle_term - kb.angle_term).abs() < 1e-14);
    }
}
