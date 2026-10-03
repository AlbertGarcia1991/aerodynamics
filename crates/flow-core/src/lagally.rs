//! Forces on elementary singularities — the Lagally theorem.
//!
//! A singularity has no surface, so pressure integration cannot give its force.
//! Potential-flow theory instead gives it from the velocity `V` induced at the
//! singularity by **everything else** (freestream, other elements, bodies):
//!
//! ```text
//! vortex  Γ :  F = ρ V × Γẑ = ρΓ (V_y, −V_x)      (Kutta–Joukowski on the vortex)
//! source  Λ :  F = −ρ Λ V                          (sink: Λ → −Λ)
//! doublet κ :  F = ρ κ (ê·∇) V                     (ê = doublet axis)
//! ```
//!
//! Signs follow this crate's conventions (`Γ > 0` counter-clockwise), so a
//! clockwise vortex in a stream along `+x` is pushed in `+y`: `L = −ρU∞Γ`.
//!
//! The doublet result is the limit of a source at `−εê` and a sink at `+εê`
//! with `κ = 2εΛ` (the pair's mutual forces cancel), which is why it depends on
//! the velocity *gradient*: in a uniform stream a doublet — the analytic
//! cylinder — feels no force, as d'Alembert requires.
//!
//! # Interpretation
//!
//! This is the force needed to **hold the singularity fixed** in the flow. A
//! free vortex or source in a real fluid would instead be carried along. It is
//! the right reading for a *bound* singularity (the idea behind lifting-line
//! theory) and a useful teaching view, not the force on a physical object.

use crate::elements::Element;
use crate::field::FlowField;
use aeroflow_geometry::Vec2;

/// Force on one element, with the external velocity it was computed from.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ElementForce {
    /// Force per unit span [N/m].
    pub force: Vec2,
    /// Velocity induced at the element by everything else [m/s].
    pub external_velocity: Vec2,
}

/// Lagally force on `field.elements[index]`, or `None` for an element without
/// a location (a uniform stream) or with a non-finite result (e.g. two
/// singularities placed on top of each other).
pub fn element_force(field: &FlowField, index: usize, density: f64) -> Option<ElementForce> {
    let e = field.elements.get(index)?;
    let pos = e.position()?;
    let v = field.velocity_excluding_element(pos, index);
    let force = match *e {
        Element::UniformFlow { .. } => return None,
        Element::Vortex { circulation, .. } => Vec2::new(v.y, -v.x) * (density * circulation),
        Element::Source { strength, .. } => v * (-density * strength),
        Element::Sink { strength, .. } => v * (density * strength),
        Element::Doublet {
            strength,
            orientation,
            ..
        } => {
            // Directional derivative of the external velocity along the axis,
            // by central differences. The step scales with the distance to the
            // nearest other singularity so it stays well inside the smooth region.
            let axis = Vec2::from_angle(orientation);
            let h = derivative_step(field, index, pos);
            let dv = (field.velocity_excluding_element(pos + axis * h, index)
                - field.velocity_excluding_element(pos - axis * h, index))
                / (2.0 * h);
            dv * (density * strength)
        }
    };
    (force.is_finite() && v.is_finite()).then_some(ElementForce {
        force,
        external_velocity: v,
    })
}

/// Forces on every element, aligned with `field.elements`.
pub fn element_forces(field: &FlowField, density: f64) -> Vec<Option<ElementForce>> {
    (0..field.elements.len())
        .map(|i| element_force(field, i, density))
        .collect()
}

fn derivative_step(field: &FlowField, skip: usize, p: Vec2) -> f64 {
    let mut nearest = f64::INFINITY;
    for (i, e) in field.elements.iter().enumerate() {
        if i != skip {
            if let Some(q) = e.position() {
                nearest = nearest.min(q.distance(p));
            }
        }
    }
    for b in &field.bodies {
        nearest = nearest.min(b.polygon.distance_to_boundary(p));
    }
    if !nearest.is_finite() {
        nearest = 1.0;
    }
    (nearest * 1e-4).max(1e-9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conditions::FlowConditions;
    use std::f64::consts::PI;

    fn stream(u: f64, alpha: f64) -> FlowField {
        FlowField::new(FlowConditions {
            velocity: u,
            angle: alpha,
            density: 1.3,
            pressure: 0.0,
        })
    }

    #[test]
    fn vortex_in_a_uniform_stream_obeys_kutta_joukowski() {
        let mut f = stream(4.0, 0.0);
        f.elements.push(Element::Vortex {
            position: Vec2::new(0.3, -0.2),
            circulation: -2.5,
        });
        let r = element_force(&f, 0, 1.3).unwrap();
        // Clockwise Γ in a +x stream: lift ρU|Γ| upwards, no drag.
        assert!((r.force.y - 1.3 * 4.0 * 2.5).abs() < 1e-12, "{:?}", r.force);
        assert!(r.force.x.abs() < 1e-12);
        assert!((r.external_velocity - Vec2::new(4.0, 0.0)).norm() < 1e-12);
    }

    #[test]
    fn vortex_force_is_perpendicular_to_the_local_flow_at_any_angle() {
        let mut f = stream(3.0, 0.6);
        f.elements.push(Element::Vortex {
            position: Vec2::ZERO,
            circulation: 1.7,
        });
        let r = element_force(&f, 0, 1.3).unwrap();
        assert!(r.force.dot(f.conditions.freestream()).abs() < 1e-12);
        assert!((r.force.norm() - 1.3 * 3.0 * 1.7).abs() < 1e-12);
    }

    #[test]
    fn source_is_pushed_upstream_and_sink_downstream() {
        let mut f = stream(2.0, 0.0);
        f.elements.push(Element::Source {
            position: Vec2::ZERO,
            strength: 3.0,
        });
        let src = element_force(&f, 0, 1.3).unwrap().force;
        assert!(
            (src.x + 1.3 * 3.0 * 2.0).abs() < 1e-12 && src.y.abs() < 1e-12,
            "{src:?}"
        );

        let mut g = stream(2.0, 0.0);
        g.elements.push(Element::Sink {
            position: Vec2::ZERO,
            strength: 3.0,
        });
        let snk = element_force(&g, 0, 1.3).unwrap().force;
        assert!(
            (snk.x - 1.3 * 3.0 * 2.0).abs() < 1e-12 && snk.y.abs() < 1e-12,
            "{snk:?}"
        );
    }

    #[test]
    fn two_sources_attract_with_equal_and_opposite_forces() {
        // Potential-flow sources attract (in-phase Bjerknes force).
        let mut f = stream(0.0, 0.0);
        f.elements.push(Element::Source {
            position: Vec2::new(-1.0, 0.0),
            strength: 2.0,
        });
        f.elements.push(Element::Source {
            position: Vec2::new(1.5, 0.5),
            strength: 3.0,
        });
        let a = element_force(&f, 0, 1.3).unwrap().force;
        let b = element_force(&f, 1, 1.3).unwrap().force;
        assert!((a + b).norm() < 1e-12, "Newton's third law: {a:?} {b:?}");
        let ab = Vec2::new(2.5, 0.5);
        assert!(a.dot(ab) > 0.0, "A must be pulled towards B");
        // Magnitude ρΛ_AΛ_B/(2πd).
        let expected = 1.3 * 2.0 * 3.0 / (2.0 * PI * ab.norm());
        assert!((a.norm() - expected).abs() < 1e-12);
    }

    #[test]
    fn two_vortices_obey_newtons_third_law() {
        let mut f = stream(0.0, 0.0);
        f.elements.push(Element::Vortex {
            position: Vec2::new(0.0, 0.0),
            circulation: 2.0,
        });
        f.elements.push(Element::Vortex {
            position: Vec2::new(1.0, 0.7),
            circulation: -1.2,
        });
        let a = element_force(&f, 0, 1.3).unwrap().force;
        let b = element_force(&f, 1, 1.3).unwrap().force;
        assert!((a + b).norm() < 1e-12, "{a:?} {b:?}");
    }

    #[test]
    fn doublet_in_a_uniform_stream_feels_no_force() {
        let mut f = stream(5.0, 0.3);
        f.elements.push(Element::Doublet {
            position: Vec2::new(0.2, 0.1),
            strength: 7.0,
            orientation: 0.3,
        });
        let r = element_force(&f, 0, 1.3).unwrap();
        assert!(r.force.norm() < 1e-6, "{:?}", r.force);
    }

    #[test]
    fn doublet_force_equals_the_limit_of_a_source_sink_pair() {
        // Doublet near a vortex vs. an explicit tiny source–sink pair (κ = 2εΛ).
        let pos = Vec2::new(0.4, -0.3);
        let (kappa, beta) = (0.9, 0.7);
        let vortex = Element::Vortex {
            position: Vec2::new(-0.8, 0.5),
            circulation: 3.0,
        };

        let mut f = stream(1.5, 0.2);
        f.elements.push(vortex);
        f.elements.push(Element::Doublet {
            position: pos,
            strength: kappa,
            orientation: beta,
        });
        let doublet = element_force(&f, 1, 1.3).unwrap().force;

        // Reference: a source at −εê and a sink at +εê with Λ = κ/2ε, each
        // feeling only the *external* field (their mutual forces are equal and
        // opposite and cancel exactly). Including them numerically would bury
        // the answer under ~1e13 N/m of cancelling terms at small ε.
        let eps = 1e-3;
        let lambda = kappa / (2.0 * eps);
        let axis = Vec2::from_angle(beta);
        let mut ext = stream(1.5, 0.2);
        ext.elements.push(vortex);
        let pair = ext.velocity(pos - axis * eps) * (-1.3 * lambda)
            + ext.velocity(pos + axis * eps) * (1.3 * lambda);

        assert!(
            (doublet - pair).norm() < 1e-4 * (1.0 + pair.norm()),
            "doublet {doublet:?} vs pair {pair:?}"
        );
        assert!(
            doublet.norm() > 1e-3,
            "test must exercise a non-trivial force"
        );
    }

    #[test]
    fn uniform_flow_elements_have_no_force() {
        let mut f = stream(1.0, 0.0);
        f.elements.push(Element::UniformFlow {
            velocity: 2.0,
            direction: 1.0,
        });
        assert!(element_force(&f, 0, 1.3).is_none());
    }

    #[test]
    fn coincident_singularities_report_no_force_instead_of_nan() {
        let mut f = stream(1.0, 0.0);
        f.elements.push(Element::Source {
            position: Vec2::ZERO,
            strength: 1.0,
        });
        f.elements.push(Element::Vortex {
            position: Vec2::ZERO,
            circulation: 1.0,
        });
        let r = element_forces(&f, 1.3);
        assert!(r.iter().all(|x| x.is_none_or(|e| e.force.is_finite())));
    }
}
