//! Global flow conditions and the Bernoulli relations derived from them.

use aeroflow_geometry::Vec2;

/// Freestream conditions (PRD §14). SI units throughout (PRD §69).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct FlowConditions {
    /// Freestream speed `U∞` [m/s].
    pub velocity: f64,
    /// Angle of attack `α` [rad], positive counter-clockwise (PRD §14.1).
    pub angle: f64,
    /// Density `ρ` [kg/m³].
    pub density: f64,
    /// Reference static pressure `p∞` [Pa].
    pub pressure: f64,
}

impl Default for FlowConditions {
    fn default() -> Self {
        Self {
            velocity: 10.0,
            angle: 0.0,
            density: 1.225, // ISA sea level
            pressure: 101_325.0,
        }
    }
}

impl FlowConditions {
    /// Freestream velocity vector `U∞·(cos α, sin α)`.
    ///
    /// A positive `α` tilts the *flow* upwards, which is equivalent to pitching
    /// the body nose-up and produces positive lift on a conventional section.
    #[inline]
    pub fn freestream(&self) -> Vec2 {
        Vec2::from_angle(self.angle) * self.velocity
    }

    /// Dynamic pressure `q∞ = ½ρU∞²` [Pa].
    #[inline]
    pub fn dynamic_pressure(&self) -> f64 {
        0.5 * self.density * self.velocity * self.velocity
    }

    /// Static pressure from Bernoulli: `p = p∞ + ½ρ(U∞² − V²)`.
    ///
    /// Well defined even at `U∞ = 0`, unlike `Cp`.
    #[inline]
    pub fn pressure_at_speed(&self, speed: f64) -> f64 {
        self.pressure + 0.5 * self.density * (self.velocity * self.velocity - speed * speed)
    }

    /// Pressure coefficient `Cp = 1 − (V/U∞)²` (PRD §20).
    ///
    /// Returns `None` when `U∞ = 0`: `Cp` has no meaning without a freestream
    /// to normalise against, and the PRD requires that case be surfaced as a
    /// warning rather than silently producing `inf` (PRD §71).
    #[inline]
    pub fn cp_at_speed(&self, speed: f64) -> Option<f64> {
        if self.velocity.abs() <= 0.0 {
            None
        } else {
            let ratio = speed / self.velocity;
            Some(1.0 - ratio * ratio)
        }
    }

    /// Transform a Cartesian force into wind axes.
    ///
    /// Lift is perpendicular to the freestream (positive to its left, i.e. "up"
    /// for `α = 0`); drag is parallel to it (positive downstream).
    #[inline]
    pub fn to_wind_axes(&self, force: Vec2) -> (f64, f64) {
        let (s, c) = self.angle.sin_cos();
        let drag = force.x * c + force.y * s;
        let lift = -force.x * s + force.y * c;
        (lift, drag)
    }

    pub fn is_valid(&self) -> bool {
        self.velocity.is_finite()
            && self.angle.is_finite()
            && self.density.is_finite()
            && self.density > 0.0
            && self.pressure.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn freestream_at_zero_alpha_is_purely_streamwise() {
        let c = FlowConditions {
            velocity: 7.0,
            angle: 0.0,
            ..Default::default()
        };
        let v = c.freestream();
        assert!((v.x - 7.0).abs() < 1e-12 && v.y.abs() < 1e-12);
    }

    #[test]
    fn positive_alpha_tilts_the_flow_upwards() {
        let c = FlowConditions {
            velocity: 1.0,
            angle: 0.2,
            ..Default::default()
        };
        assert!(c.freestream().y > 0.0);
    }

    #[test]
    fn wind_axes_reduce_to_cartesian_at_zero_alpha() {
        let c = FlowConditions {
            angle: 0.0,
            ..Default::default()
        };
        let (l, d) = c.to_wind_axes(Vec2::new(3.0, 5.0));
        assert!((d - 3.0).abs() < 1e-12 && (l - 5.0).abs() < 1e-12);
    }

    #[test]
    fn wind_axes_rotate_with_alpha() {
        // At α = 90° the freestream points along +y, so a +y force is pure drag.
        let c = FlowConditions {
            angle: FRAC_PI_2,
            ..Default::default()
        };
        let (l, d) = c.to_wind_axes(Vec2::new(0.0, 1.0));
        assert!((d - 1.0).abs() < 1e-12, "drag = {d}");
        assert!(l.abs() < 1e-12, "lift = {l}");
    }

    #[test]
    fn wind_axes_preserve_force_magnitude() {
        let c = FlowConditions {
            angle: 0.37,
            ..Default::default()
        };
        let f = Vec2::new(-2.0, 4.5);
        let (l, d) = c.to_wind_axes(f);
        assert!(((l * l + d * d).sqrt() - f.norm()).abs() < 1e-12);
    }

    #[test]
    fn cp_is_one_at_a_stagnation_point_and_zero_in_the_freestream() {
        let c = FlowConditions {
            velocity: 12.0,
            ..Default::default()
        };
        assert!((c.cp_at_speed(0.0).unwrap() - 1.0).abs() < 1e-12);
        assert!(c.cp_at_speed(12.0).unwrap().abs() < 1e-12);
        // Twice the freestream speed (top of a cylinder) gives Cp = −3.
        assert!((c.cp_at_speed(24.0).unwrap() + 3.0).abs() < 1e-12);
    }

    #[test]
    fn cp_is_undefined_without_a_freestream() {
        let c = FlowConditions {
            velocity: 0.0,
            ..Default::default()
        };
        assert!(c.cp_at_speed(5.0).is_none());
        // Pressure, however, remains well defined.
        assert!(c.pressure_at_speed(5.0).is_finite());
    }

    #[test]
    fn pressure_follows_bernoulli() {
        let c = FlowConditions {
            velocity: 10.0,
            density: 2.0,
            pressure: 100.0,
            angle: 0.0,
        };
        // Stagnation: p = p∞ + ½ρU² = 100 + 100 = 200.
        assert!((c.pressure_at_speed(0.0) - 200.0).abs() < 1e-12);
        // Freestream speed recovers p∞.
        assert!((c.pressure_at_speed(10.0) - 100.0).abs() < 1e-12);
    }

    #[test]
    fn cp_and_pressure_are_consistent() {
        let c = FlowConditions::default();
        let v = 17.3;
        let p = c.pressure_at_speed(v);
        let cp = c.cp_at_speed(v).unwrap();
        assert!(((p - c.pressure) / c.dynamic_pressure() - cp).abs() < 1e-12);
    }

    #[test]
    fn invalid_conditions_are_rejected() {
        assert!(!FlowConditions {
            density: 0.0,
            ..Default::default()
        }
        .is_valid());
        assert!(!FlowConditions {
            density: -1.0,
            ..Default::default()
        }
        .is_valid());
        assert!(!FlowConditions {
            velocity: f64::NAN,
            ..Default::default()
        }
        .is_valid());
        assert!(FlowConditions::default().is_valid());
    }
}
