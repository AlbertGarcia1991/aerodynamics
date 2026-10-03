//! Panel (boundary element) construction.
//!
//! A [`Panel`] is one straight segment of a discretised body boundary. The
//! solver evaluates the flow-tangency condition at [`Panel::mid`] and uses
//! [`Panel::normal`] as the **outward** normal, which holds because panels are
//! always built from a counter-clockwise contour (see [`crate::polygon`]).

use crate::polygon::Polygon;
use crate::vec2::{Bounds, Vec2};

/// One straight boundary element.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Panel {
    pub start: Vec2,
    pub end: Vec2,
    /// Collocation point: the geometric midpoint.
    pub mid: Vec2,
    pub length: f64,
    /// Unit vector `start → end`.
    pub tangent: Vec2,
    /// Outward unit normal (`tangent` rotated −90°).
    pub normal: Vec2,
    /// Panel inclination: `atan2` of the tangent, radians.
    pub angle: f64,
}

impl Panel {
    pub fn new(start: Vec2, end: Vec2) -> Self {
        let d = end - start;
        let length = d.norm();
        let tangent = d.normalized();
        Self {
            start,
            end,
            mid: (start + end) * 0.5,
            length,
            tangent,
            normal: tangent.perp_right(),
            angle: d.angle(),
        }
    }

    /// Project a global point into the panel's local frame:
    /// `ξ` along [`Panel::tangent`] from [`Panel::start`], `η` along the
    /// *left* normal (so the body exterior is at `η < 0`).
    #[inline]
    pub fn to_local(&self, p: Vec2) -> (f64, f64) {
        let d = p - self.start;
        (d.dot(self.tangent), d.dot(self.tangent.perp_left()))
    }

    /// Map a local-frame vector back to global coordinates.
    #[inline]
    pub fn to_global_vector(&self, xi: f64, eta: f64) -> Vec2 {
        self.tangent * xi + self.tangent.perp_left() * eta
    }
}

/// A body's complete discretisation plus the derived quality metrics the
/// solver reports as diagnostics (PRD §44).
#[derive(Debug, Clone)]
pub struct Panelisation {
    pub panels: Vec<Panel>,
    pub perimeter: f64,
    pub min_length: f64,
    pub max_length: f64,
    /// `max_length / min_length` — a plain measure of distribution uniformity.
    pub length_ratio: f64,
    /// Largest turn between consecutive panel tangents, radians. A sharp
    /// trailing edge shows up here.
    pub max_turn_angle: f64,
    pub bounds: Bounds,
    pub area: f64,
}

impl Panelisation {
    /// Build panels from a closed contour. The contour is expected to already be
    /// counter-clockwise; see [`Polygon::make_counter_clockwise`].
    pub fn from_polygon(poly: &Polygon) -> Option<Self> {
        let n = poly.points.len();
        if n < 3 {
            return None;
        }
        let panels: Vec<Panel> = (0..n)
            .map(|i| Panel::new(poly.points[i], poly.points[(i + 1) % n]))
            .collect();

        let mut min_length = f64::INFINITY;
        let mut max_length: f64 = 0.0;
        let mut perimeter = 0.0;
        for p in &panels {
            min_length = min_length.min(p.length);
            max_length = max_length.max(p.length);
            perimeter += p.length;
        }

        // Turn angle between panel i and panel i+1, wrapped to (−π, π].
        let mut max_turn_angle: f64 = 0.0;
        for i in 0..n {
            let a = panels[i].tangent;
            let b = panels[(i + 1) % n].tangent;
            let turn = b.cross(a).atan2(b.dot(a)).abs();
            max_turn_angle = max_turn_angle.max(turn);
        }

        Some(Self {
            panels,
            perimeter,
            min_length,
            max_length,
            length_ratio: if min_length > 0.0 {
                max_length / min_length
            } else {
                f64::INFINITY
            },
            max_turn_angle,
            bounds: poly.bounds()?,
            area: poly.area(),
        })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }

    /// Arc length to each panel midpoint, measured from the first panel start.
    pub fn midpoint_arc_lengths(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.panels.len());
        let mut acc = 0.0;
        for p in &self.panels {
            out.push(acc + 0.5 * p.length);
            acc += p.length;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn panel_normal_points_outward_for_ccw_square() {
        let poly = Polygon::new(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ]);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        // Bottom panel: outward normal is −y.
        assert!((pz.panels[0].normal - Vec2::new(0.0, -1.0)).norm() < 1e-12);
        // Right panel: outward normal is +x.
        assert!((pz.panels[1].normal - Vec2::new(1.0, 0.0)).norm() < 1e-12);
        // Every normal must point away from the centroid.
        let c = poly.centroid();
        for p in &pz.panels {
            assert!((p.mid - c).dot(p.normal) > 0.0, "normal points inward");
        }
    }

    #[test]
    fn circle_normals_are_radial_and_outward() {
        let n = 64;
        let pts: Vec<Vec2> = (0..n)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / n as f64;
                Vec2::new(t.cos(), t.sin())
            })
            .collect();
        let pz = Panelisation::from_polygon(&Polygon::new(pts)).unwrap();
        for p in &pz.panels {
            let radial = p.mid.normalized();
            assert!(p.normal.dot(radial) > 0.99, "normal not radially outward");
        }
        // An inscribed 64-gon is slightly short of 2π: the deficit is
        // 2π·π²/(6n²) ≈ 2.5e-3, so bound it just above that.
        let deficit = 2.0 * PI - pz.perimeter;
        assert!(deficit > 0.0 && deficit < 3e-3, "deficit = {deficit}");
    }

    #[test]
    fn local_frame_places_exterior_at_negative_eta() {
        // Bottom edge of a CCW square: exterior is below (−y).
        let panel = Panel::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0));
        let (_, eta) = panel.to_local(Vec2::new(0.5, -0.3));
        assert!(eta < 0.0, "exterior should be at eta < 0");
        let (xi, eta_in) = panel.to_local(Vec2::new(0.5, 0.3));
        assert!(eta_in > 0.0);
        assert!((xi - 0.5).abs() < 1e-12);
    }

    #[test]
    fn sharp_trailing_edge_registers_as_a_large_turn() {
        // A thin wedge has a near-180° turn at its tip.
        let poly = Polygon::new(vec![
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.05),
            Vec2::new(-0.2, 0.0),
            Vec2::new(0.0, -0.05),
        ]);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        assert!(pz.max_turn_angle > 2.0, "turn = {}", pz.max_turn_angle);
    }

    #[test]
    fn to_global_vector_round_trips_local_components() {
        let panel = Panel::new(Vec2::new(1.0, 2.0), Vec2::new(3.0, 5.0));
        let v = panel.to_global_vector(2.0, -1.0);
        assert!((v.dot(panel.tangent) - 2.0).abs() < 1e-12);
        assert!((v.dot(panel.tangent.perp_left()) + 1.0).abs() < 1e-12);
    }
}
