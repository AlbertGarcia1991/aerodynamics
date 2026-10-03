//! Elementary analytic potential-flow solutions (PRD §7).
//!
//! # Sign and strength conventions
//!
//! These are fixed here and must not be redefined anywhere else in the
//! application (PRD §7, §14.1, §68).
//!
//! | Element | Parameter | Symbol | Unit | Convention |
//! | --- | --- | --- | --- | --- |
//! | Uniform flow | speed, direction | `U`, `θ` | m/s, rad | `θ` counter-clockwise from `+x` |
//! | Source | volumetric strength | `Λ` | m²/s | `Λ > 0` emits fluid |
//! | Sink | volumetric strength | `Λ` | m²/s | exposed separately; stored as `−Λ` source |
//! | Vortex | circulation | `Γ` | m²/s | **`Γ > 0` is counter-clockwise** |
//! | Doublet | strength, axis | `κ`, `β` | m³/s, rad | `κ > 0` with `β` along the flow forms a cylinder |
//!
//! ## On the circulation sign
//!
//! `Γ > 0` is taken as counter-clockwise, matching the mathematical convention
//! and the vorticity definition `ω_z = ∂v/∂x − ∂u/∂y`. Classical aerodynamics
//! texts frequently define `Γ` positive *clockwise* so that the
//! Kutta–Joukowski theorem reads `L = ρU∞Γ`. Under the convention used here
//! that theorem is
//!
//! ```text
//! L = −ρ U∞ Γ
//! ```
//!
//! so a *clockwise* circulation (negative `Γ`) produces positive lift. The UI
//! states this explicitly next to the circulation input.

use aeroflow_geometry::Vec2;
use std::f64::consts::PI;

const INV_2PI: f64 = 1.0 / (2.0 * PI);

/// Discriminant used by the UI and the serialisation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ElementKind {
    UniformFlow,
    Source,
    Sink,
    Vortex,
    Doublet,
}

impl ElementKind {
    pub const ALL: [ElementKind; 5] = [
        ElementKind::UniformFlow,
        ElementKind::Source,
        ElementKind::Sink,
        ElementKind::Vortex,
        ElementKind::Doublet,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ElementKind::UniformFlow => "uniformFlow",
            ElementKind::Source => "source",
            ElementKind::Sink => "sink",
            ElementKind::Vortex => "vortex",
            ElementKind::Doublet => "doublet",
        }
    }
}

/// One elementary flow singularity.
///
/// Variants carry their own parameters so that a scene is fully described by a
/// list of these values, which is exactly what gets serialised to
/// `.aeroflow.json` and sent across the WASM boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase"))]
pub enum Element {
    /// An additional uniform stream, superposed on the global freestream.
    /// Useful for demonstrating superposition directly.
    UniformFlow {
        velocity: f64,
        direction: f64,
    },
    Source {
        position: Vec2,
        strength: f64,
    },
    Sink {
        position: Vec2,
        strength: f64,
    },
    Vortex {
        position: Vec2,
        circulation: f64,
    },
    Doublet {
        position: Vec2,
        strength: f64,
        orientation: f64,
    },
}

impl Element {
    pub fn kind(&self) -> ElementKind {
        match self {
            Element::UniformFlow { .. } => ElementKind::UniformFlow,
            Element::Source { .. } => ElementKind::Source,
            Element::Sink { .. } => ElementKind::Sink,
            Element::Vortex { .. } => ElementKind::Vortex,
            Element::Doublet { .. } => ElementKind::Doublet,
        }
    }

    /// Anchor point, or `None` for a uniform stream (which has no location).
    pub fn position(&self) -> Option<Vec2> {
        match *self {
            Element::UniformFlow { .. } => None,
            Element::Source { position, .. }
            | Element::Sink { position, .. }
            | Element::Vortex { position, .. }
            | Element::Doublet { position, .. } => Some(position),
        }
    }

    pub fn set_position(&mut self, p: Vec2) {
        match self {
            Element::UniformFlow { .. } => {}
            Element::Source { position, .. }
            | Element::Sink { position, .. }
            | Element::Vortex { position, .. }
            | Element::Doublet { position, .. } => *position = p,
        }
    }

    /// Signed source strength `Λ`: a sink is stored as a positive magnitude in
    /// the UI but behaves as `−Λ` (PRD §7, "sink exposed separately").
    #[inline]
    fn effective_source_strength(&self) -> f64 {
        match *self {
            Element::Source { strength, .. } => strength,
            Element::Sink { strength, .. } => -strength,
            _ => 0.0,
        }
    }

    /// Net bound circulation contributed by this element [m²/s],
    /// counter-clockwise positive.
    #[inline]
    pub fn circulation(&self) -> f64 {
        match *self {
            Element::Vortex { circulation, .. } => circulation,
            _ => 0.0,
        }
    }

    /// Net volumetric outflow contributed by this element [m²/s].
    ///
    /// A scene whose total is non-zero cannot be closed by any streamline at
    /// infinity, which the solver reports as a modelling warning.
    #[inline]
    pub fn net_outflow(&self) -> f64 {
        self.effective_source_strength()
    }

    /// Induced velocity at `p`.
    ///
    /// `core` softens the singularity by replacing `r²` with `r² + core²`. Pass
    /// `0.0` for the exact analytic field — which is what the solver does
    /// everywhere that feeds a numerical result. A non-zero `core` is used only
    /// for field *rendering*, so that a pixel landing on a singularity does not
    /// blow the colour scale away (PRD §71's "never silently produce an invalid
    /// result" applies: the renderer flags when softening is active).
    pub fn velocity(&self, p: Vec2, core: f64) -> Vec2 {
        match *self {
            Element::UniformFlow {
                velocity,
                direction,
            } => Vec2::from_angle(direction) * velocity,

            Element::Source { position, .. } | Element::Sink { position, .. } => {
                let lambda = self.effective_source_strength();
                let r = p - position;
                let r2 = r.norm_sq() + core * core;
                if r2 <= 0.0 {
                    return Vec2::ZERO;
                }
                r * (lambda * INV_2PI / r2)
            }

            Element::Vortex {
                position,
                circulation,
            } => {
                let r = p - position;
                let r2 = r.norm_sq() + core * core;
                if r2 <= 0.0 {
                    return Vec2::ZERO;
                }
                // Γ > 0 ⇒ counter-clockwise: V = Γ/(2πr²) · (−ry, rx)
                Vec2::new(-r.y, r.x) * (circulation * INV_2PI / r2)
            }

            Element::Doublet {
                position,
                strength,
                orientation,
            } => {
                let r = (p - position).rotate(-orientation);
                let r2 = r.norm_sq() + core * core;
                if r2 <= 0.0 {
                    return Vec2::ZERO;
                }
                let k = strength * INV_2PI / (r2 * r2);
                // Local frame (axis along +x):
                //   u = (κ/2π)(ỹ² − x̃²)/r⁴,  v = −(κ/2π)(2x̃ỹ)/r⁴
                let local = Vec2::new((r.y * r.y - r.x * r.x) * k, -2.0 * r.x * r.y * k);
                local.rotate(orientation)
            }
        }
    }

    /// Velocity potential `φ`, with `∇φ = V`.
    ///
    /// `φ` is multivalued for a vortex (it jumps by `Γ` across a branch cut);
    /// the branch is the one implied by `atan2`.
    pub fn potential(&self, p: Vec2) -> f64 {
        match *self {
            Element::UniformFlow {
                velocity,
                direction,
            } => {
                let d = Vec2::from_angle(direction);
                velocity * (p.x * d.x + p.y * d.y)
            }
            Element::Source { position, .. } | Element::Sink { position, .. } => {
                let r = (p - position).norm();
                if r <= 0.0 {
                    return f64::NEG_INFINITY;
                }
                self.effective_source_strength() * INV_2PI * r.ln()
            }
            Element::Vortex {
                position,
                circulation,
            } => {
                let r = p - position;
                circulation * INV_2PI * r.y.atan2(r.x)
            }
            Element::Doublet {
                position,
                strength,
                orientation,
            } => {
                let r = (p - position).rotate(-orientation);
                let r2 = r.norm_sq();
                if r2 <= 0.0 {
                    return 0.0;
                }
                strength * INV_2PI * r.x / r2
            }
        }
    }

    /// Stream function `ψ`, with `u = ∂ψ/∂y`, `v = −∂ψ/∂x`.
    ///
    /// `ψ` is multivalued for a source (it jumps by `Λ` across a branch cut),
    /// which is physical: a flow with net outflow has no single-valued stream
    /// function. Streamlines are therefore traced by integrating the velocity
    /// field rather than by contouring `ψ` (PRD DEC-002).
    pub fn stream_function(&self, p: Vec2) -> f64 {
        match *self {
            Element::UniformFlow {
                velocity,
                direction,
            } => {
                let d = Vec2::from_angle(direction);
                velocity * (p.y * d.x - p.x * d.y)
            }
            Element::Source { position, .. } | Element::Sink { position, .. } => {
                let r = p - position;
                self.effective_source_strength() * INV_2PI * r.y.atan2(r.x)
            }
            Element::Vortex {
                position,
                circulation,
            } => {
                let r = (p - position).norm();
                if r <= 0.0 {
                    return f64::INFINITY;
                }
                -circulation * INV_2PI * r.ln()
            }
            Element::Doublet {
                position,
                strength,
                orientation,
            } => {
                let r = (p - position).rotate(-orientation);
                let r2 = r.norm_sq();
                if r2 <= 0.0 {
                    return 0.0;
                }
                -strength * INV_2PI * r.y / r2
            }
        }
    }

    pub fn is_finite(&self) -> bool {
        match *self {
            Element::UniformFlow {
                velocity,
                direction,
            } => velocity.is_finite() && direction.is_finite(),
            Element::Source { position, strength } | Element::Sink { position, strength } => {
                position.is_finite() && strength.is_finite()
            }
            Element::Vortex {
                position,
                circulation,
            } => position.is_finite() && circulation.is_finite(),
            Element::Doublet {
                position,
                strength,
                orientation,
            } => position.is_finite() && strength.is_finite() && orientation.is_finite(),
        }
    }

    /// The doublet strength that, superposed on a uniform stream of speed `u`,
    /// produces a circular streamline of radius `a`: `κ = 2π·u·a²`.
    ///
    /// Exposed because it turns the abstract doublet strength into something a
    /// user can reason about, and it is how the bundled cylinder example is
    /// constructed analytically.
    pub fn doublet_strength_for_cylinder(u: f64, a: f64) -> f64 {
        2.0 * PI * u * a * a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    fn src(l: f64) -> Element {
        Element::Source {
            position: Vec2::ZERO,
            strength: l,
        }
    }
    fn vtx(g: f64) -> Element {
        Element::Vortex {
            position: Vec2::ZERO,
            circulation: g,
        }
    }

    #[test]
    fn uniform_flow_velocity_matches_its_definition() {
        let e = Element::UniformFlow {
            velocity: 5.0,
            direction: 0.0,
        };
        let v = e.velocity(Vec2::new(3.0, -2.0), 0.0);
        assert!((v.x - 5.0).abs() < TOL && v.y.abs() < TOL);

        let e = Element::UniformFlow {
            velocity: 2.0,
            direction: PI / 2.0,
        };
        let v = e.velocity(Vec2::ZERO, 0.0);
        assert!(v.x.abs() < 1e-15 && (v.y - 2.0).abs() < TOL);
    }

    /// PRD §61.2: the source field must match the analytical radial solution
    /// `V_r = Λ/(2πr)`, purely radial.
    #[test]
    fn source_is_purely_radial_with_the_analytic_magnitude() {
        let lambda = 3.7;
        let e = src(lambda);
        for (r, theta) in [(0.5, 0.0), (2.0, 1.1), (7.5, -2.3), (1.0, PI)] {
            let p = Vec2::from_angle(theta) * r;
            let v = e.velocity(p, 0.0);
            let expected = lambda / (2.0 * PI * r);
            // Radial component equals the analytic value…
            assert!((v.dot(p.normalized()) - expected).abs() < 1e-12);
            // …and the tangential component is exactly zero.
            assert!(v.dot(p.normalized().perp_left()).abs() < 1e-14);
        }
    }

    /// PRD §61.3: a sink is the source solution with the opposite sign.
    #[test]
    fn sink_is_the_negated_source() {
        let p = Vec2::new(1.3, -0.7);
        let s = src(2.0).velocity(p, 0.0);
        let k = Element::Sink {
            position: Vec2::ZERO,
            strength: 2.0,
        }
        .velocity(p, 0.0);
        assert!((s + k).norm() < 1e-15);
        assert!(
            Element::Sink {
                position: Vec2::ZERO,
                strength: 2.0
            }
            .net_outflow()
                < 0.0
        );
    }

    /// PRD §61.4: `Vθ = Γ/(2πr)` with the documented counter-clockwise sign.
    #[test]
    fn vortex_is_purely_tangential_and_counter_clockwise_for_positive_gamma() {
        let gamma = 4.2;
        let e = vtx(gamma);
        for (r, theta) in [(0.3, 0.0), (1.0, 0.6), (5.0, 2.9)] {
            let p = Vec2::from_angle(theta) * r;
            let v = e.velocity(p, 0.0);
            let t_hat = p.normalized().perp_left(); // counter-clockwise tangent
            assert!((v.dot(t_hat) - gamma / (2.0 * PI * r)).abs() < 1e-12);
            assert!(
                v.dot(p.normalized()).abs() < 1e-14,
                "radial component non-zero"
            );
        }
        // Explicit check at (r, 0): the induced velocity points +y.
        let v = e.velocity(Vec2::new(1.0, 0.0), 0.0);
        assert!(v.y > 0.0 && v.x.abs() < 1e-15);
    }

    #[test]
    fn circulation_integral_around_a_vortex_recovers_gamma() {
        // ∮ V·dl around a circle enclosing the vortex must equal Γ.
        let gamma = -2.5;
        let e = vtx(gamma);
        let n = 20_000;
        let r = 1.7;
        let mut acc = 0.0;
        for i in 0..n {
            let t = 2.0 * PI * (i as f64 + 0.5) / n as f64;
            let p = Vec2::from_angle(t) * r;
            let dl = Vec2::from_angle(t).perp_left() * (2.0 * PI * r / n as f64);
            acc += e.velocity(p, 0.0).dot(dl);
        }
        assert!(
            (acc - gamma).abs() < 1e-8,
            "circulation = {acc}, expected {gamma}"
        );
    }

    #[test]
    fn source_flux_through_an_enclosing_circle_recovers_lambda() {
        let lambda = 6.1;
        let e = src(lambda);
        let n = 20_000;
        let r = 0.9;
        let mut acc = 0.0;
        for i in 0..n {
            let t = 2.0 * PI * (i as f64 + 0.5) / n as f64;
            let nrm = Vec2::from_angle(t);
            acc += e.velocity(nrm * r, 0.0).dot(nrm) * (2.0 * PI * r / n as f64);
        }
        assert!((acc - lambda).abs() < 1e-8, "flux = {acc}");
    }

    /// The doublet + uniform stream must reproduce the analytic cylinder:
    /// stagnation at `(±a, 0)` and `2U` at the crown.
    #[test]
    fn doublet_plus_uniform_flow_forms_a_cylinder() {
        let u = 3.0;
        let a = 1.4;
        let kappa = Element::doublet_strength_for_cylinder(u, a);
        let d = Element::Doublet {
            position: Vec2::ZERO,
            strength: kappa,
            orientation: 0.0,
        };
        let stream = Element::UniformFlow {
            velocity: u,
            direction: 0.0,
        };

        let total = |p: Vec2| d.velocity(p, 0.0) + stream.velocity(p, 0.0);

        // Stagnation points on the axis.
        for x in [a, -a] {
            let v = total(Vec2::new(x, 0.0));
            assert!(v.norm() < 1e-12, "stagnation failed at x = {x}: {v:?}");
        }
        // Crown: speed is exactly 2U, tangential.
        let v = total(Vec2::new(0.0, a));
        assert!((v.x - 2.0 * u).abs() < 1e-12, "{v:?}");
        assert!(v.y.abs() < 1e-14);
        // The surface must be a streamline: no normal velocity anywhere on it.
        for i in 0..64 {
            let t = 2.0 * PI * i as f64 / 64.0;
            let n_hat = Vec2::from_angle(t);
            assert!(
                total(n_hat * a).dot(n_hat).abs() < 1e-12,
                "normal flow at θ = {t}"
            );
        }
    }

    #[test]
    fn rotated_doublet_rotates_the_cylinder_stagnation_points() {
        let u = 2.0;
        let a = 1.0;
        let beta = 0.7;
        let kappa = Element::doublet_strength_for_cylinder(u, a);
        let d = Element::Doublet {
            position: Vec2::ZERO,
            strength: kappa,
            orientation: beta,
        };
        let stream = Element::UniformFlow {
            velocity: u,
            direction: beta,
        };
        let v = d.velocity(Vec2::from_angle(beta) * a, 0.0) + stream.velocity(Vec2::ZERO, 0.0);
        assert!(
            v.norm() < 1e-12,
            "stagnation point did not rotate with the doublet: {v:?}"
        );
    }

    /// `∇φ = V` must hold for every element, checked by central differences.
    #[test]
    fn potential_gradient_equals_velocity() {
        let h = 1e-6;
        let elements = [
            Element::UniformFlow {
                velocity: 2.0,
                direction: 0.4,
            },
            src(1.5),
            Element::Sink {
                position: Vec2::new(0.2, -0.1),
                strength: 0.8,
            },
            vtx(1.1),
            Element::Doublet {
                position: Vec2::new(-0.3, 0.2),
                strength: 0.9,
                orientation: 0.5,
            },
        ];
        let probes = [
            Vec2::new(1.0, 0.7),
            Vec2::new(-2.0, 1.3),
            Vec2::new(0.5, -1.9),
        ];
        for e in &elements {
            for p in &probes {
                let dphi_dx = (e.potential(*p + Vec2::new(h, 0.0))
                    - e.potential(*p - Vec2::new(h, 0.0)))
                    / (2.0 * h);
                let dphi_dy = (e.potential(*p + Vec2::new(0.0, h))
                    - e.potential(*p - Vec2::new(0.0, h)))
                    / (2.0 * h);
                let v = e.velocity(*p, 0.0);
                assert!(
                    (dphi_dx - v.x).abs() < 1e-5 && (dphi_dy - v.y).abs() < 1e-5,
                    "{:?} at {:?}: grad phi = ({dphi_dx}, {dphi_dy}), V = {v:?}",
                    e.kind(),
                    p
                );
            }
        }
    }

    /// `u = ∂ψ/∂y`, `v = −∂ψ/∂x` must hold for every element.
    #[test]
    fn stream_function_derivatives_equal_velocity() {
        let h = 1e-6;
        let elements = [
            Element::UniformFlow {
                velocity: 2.0,
                direction: 0.4,
            },
            src(1.5),
            vtx(1.1),
            Element::Doublet {
                position: Vec2::new(-0.3, 0.2),
                strength: 0.9,
                orientation: 0.5,
            },
        ];
        // Probes kept clear of the branch cut along −x from each singularity.
        let probes = [
            Vec2::new(1.0, 0.7),
            Vec2::new(2.0, 1.3),
            Vec2::new(1.5, -1.9),
        ];
        for e in &elements {
            for p in &probes {
                let dpsi_dy = (e.stream_function(*p + Vec2::new(0.0, h))
                    - e.stream_function(*p - Vec2::new(0.0, h)))
                    / (2.0 * h);
                let dpsi_dx = (e.stream_function(*p + Vec2::new(h, 0.0))
                    - e.stream_function(*p - Vec2::new(h, 0.0)))
                    / (2.0 * h);
                let v = e.velocity(*p, 0.0);
                assert!(
                    (dpsi_dy - v.x).abs() < 1e-5 && (-dpsi_dx - v.y).abs() < 1e-5,
                    "{:?} at {:?}: (dψ/dy, −dψ/dx) = ({dpsi_dy}, {})",
                    e.kind(),
                    p,
                    -dpsi_dx
                );
            }
        }
    }

    /// PRD §61.5: superposition. A source and an equal sink separated along `x`
    /// place a stagnation point nowhere on the axis between them, but the
    /// combined field must equal the sum of the parts everywhere.
    #[test]
    fn superposition_of_source_and_sink_is_additive() {
        let a = Element::Source {
            position: Vec2::new(-1.0, 0.0),
            strength: 2.0,
        };
        let b = Element::Sink {
            position: Vec2::new(1.0, 0.0),
            strength: 2.0,
        };
        let p = Vec2::new(0.3, 1.7);
        let sum = a.velocity(p, 0.0) + b.velocity(p, 0.0);
        // On the perpendicular bisector the x-components reinforce and the
        // y-components cancel exactly.
        let mirror = Vec2::new(0.0, 1.7);
        let sym = a.velocity(mirror, 0.0) + b.velocity(mirror, 0.0);
        assert!(
            sym.y.abs() < 1e-15,
            "y should cancel on the bisector: {sym:?}"
        );
        assert!(sym.x > 0.0);
        assert!(sum.is_finite());
    }

    /// A source/sink pair of equal strength has a closed dividing streamline:
    /// a Rankine oval. On the oval the normal velocity must vanish.
    #[test]
    fn rankine_oval_dividing_streamline_has_no_normal_flow() {
        // U∞ + source at −b + sink at +b. The stagnation points lie at
        // x = ±√(b² + Λb/(πU)).
        let u = 1.0;
        let lambda = 2.0;
        let b = 1.0;
        let s = Element::Source {
            position: Vec2::new(-b, 0.0),
            strength: lambda,
        };
        let k = Element::Sink {
            position: Vec2::new(b, 0.0),
            strength: lambda,
        };
        let f = Element::UniformFlow {
            velocity: u,
            direction: 0.0,
        };
        let vel = |p: Vec2| s.velocity(p, 0.0) + k.velocity(p, 0.0) + f.velocity(p, 0.0);

        let x_stag = (b * b + lambda * b / (PI * u)).sqrt();
        let v = vel(Vec2::new(x_stag, 0.0));
        assert!(v.norm() < 1e-12, "stagnation at x = {x_stag} gave {v:?}");
        let v = vel(Vec2::new(-x_stag, 0.0));
        assert!(v.norm() < 1e-12);
    }

    #[test]
    fn core_softening_keeps_the_field_finite_at_the_singularity() {
        let e = vtx(1.0);
        assert!(
            !e.velocity(Vec2::ZERO, 0.0).is_finite() || e.velocity(Vec2::ZERO, 0.0) == Vec2::ZERO
        );
        let v = e.velocity(Vec2::new(1e-14, 0.0), 0.1);
        assert!(v.is_finite() && v.norm() < 10.0, "{v:?}");
        // Far from the core, softening is negligible.
        let exact = e.velocity(Vec2::new(10.0, 0.0), 0.0);
        let soft = e.velocity(Vec2::new(10.0, 0.0), 0.01);
        assert!((exact - soft).norm() / exact.norm() < 1e-5);
    }

    #[test]
    fn non_finite_parameters_are_detected() {
        assert!(!src(f64::NAN).is_finite());
        assert!(!Element::Vortex {
            position: Vec2::new(f64::INFINITY, 0.0),
            circulation: 1.0
        }
        .is_finite());
        assert!(src(1.0).is_finite());
    }

    #[test]
    fn uniform_flow_has_no_position() {
        let mut e = Element::UniformFlow {
            velocity: 1.0,
            direction: 0.0,
        };
        assert!(e.position().is_none());
        e.set_position(Vec2::new(5.0, 5.0)); // must be a no-op, not a panic
        assert!(e.position().is_none());
    }

    #[test]
    fn set_position_moves_located_elements() {
        let mut e = src(1.0);
        e.set_position(Vec2::new(2.0, 3.0));
        assert_eq!(e.position().unwrap(), Vec2::new(2.0, 3.0));
    }
}
