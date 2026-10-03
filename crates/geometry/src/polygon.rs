//! Closed-contour (polygon) operations.
//!
//! A [`Polygon`] stores the vertices of a *closed* contour **without** repeating
//! the first point at the end. The canonical internal orientation is
//! **counter-clockwise**, which makes [`Vec2::perp_right`] of each edge tangent
//! the outward normal.

use crate::vec2::{Bounds, Vec2};

/// Winding direction of a closed contour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winding {
    CounterClockwise,
    Clockwise,
    /// Degenerate: zero enclosed area.
    Degenerate,
}

/// A closed polygon. The edge list is `p[0]→p[1], …, p[n-1]→p[0]`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polygon {
    pub points: Vec<Vec2>,
}

impl Polygon {
    pub fn new(points: Vec<Vec2>) -> Self {
        Self { points }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Shoelace signed area. Positive for counter-clockwise winding.
    pub fn signed_area(&self) -> f64 {
        let n = self.points.len();
        if n < 3 {
            return 0.0;
        }
        let mut acc = 0.0;
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + 1) % n];
            acc += a.cross(b);
        }
        0.5 * acc
    }

    pub fn area(&self) -> f64 {
        self.signed_area().abs()
    }

    pub fn winding(&self) -> Winding {
        let a = self.signed_area();
        // Scale the degeneracy threshold by the contour size so it is unit-agnostic.
        let scale = self
            .bounds()
            .map(|b| b.diagonal())
            .unwrap_or(1.0)
            .max(1e-300);
        if a > 1e-12 * scale * scale {
            Winding::CounterClockwise
        } else if a < -1e-12 * scale * scale {
            Winding::Clockwise
        } else {
            Winding::Degenerate
        }
    }

    /// Reverse the point order in place.
    pub fn reverse(&mut self) {
        self.points.reverse();
    }

    /// Force counter-clockwise winding (the canonical solver orientation).
    /// Returns `true` if the contour was reversed.
    pub fn make_counter_clockwise(&mut self) -> bool {
        if self.winding() == Winding::Clockwise {
            self.reverse();
            true
        } else {
            false
        }
    }

    pub fn perimeter(&self) -> f64 {
        let n = self.points.len();
        if n < 2 {
            return 0.0;
        }
        (0..n)
            .map(|i| self.points[i].distance(self.points[(i + 1) % n]))
            .sum()
    }

    /// Area centroid (not the vertex average) — the natural moment reference.
    /// Falls back to the vertex average for degenerate contours.
    pub fn centroid(&self) -> Vec2 {
        let n = self.points.len();
        if n == 0 {
            return Vec2::ZERO;
        }
        let a = self.signed_area();
        if a.abs() < 1e-300 {
            let sum = self.points.iter().fold(Vec2::ZERO, |acc, p| acc + *p);
            return sum / n as f64;
        }
        let mut c = Vec2::ZERO;
        for i in 0..n {
            let p = self.points[i];
            let q = self.points[(i + 1) % n];
            let w = p.cross(q);
            c = c + (p + q) * w;
        }
        c / (6.0 * a)
    }

    pub fn bounds(&self) -> Option<Bounds> {
        Bounds::from_points(&self.points)
    }

    /// The two vertices furthest apart, and their separation.
    ///
    /// For an airfoil these are the trailing and leading edge, so the separation
    /// is the chord. `O(n²)` but `n` is a few hundred at most, and it runs once
    /// per import rather than per solve.
    pub fn diameter(&self) -> Option<(usize, usize, f64)> {
        let n = self.points.len();
        if n < 2 {
            return None;
        }
        let mut best = (0usize, 0usize, -1.0f64);
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.points[i].distance(self.points[j]);
                if d > best.2 {
                    best = (i, j, d);
                }
            }
        }
        Some(best)
    }

    /// Crossing-number point-in-polygon test. Robust for points off the boundary;
    /// boundary points are reported inconsistently by construction, which is
    /// acceptable because the solver only uses this to mask field samples.
    pub fn contains_point(&self, p: Vec2) -> bool {
        let n = self.points.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[j];
            if (a.y > p.y) != (b.y > p.y) {
                let t = (p.y - a.y) / (b.y - a.y);
                if p.x < a.x + t * (b.x - a.x) {
                    inside = !inside;
                }
            }
            j = i;
        }
        inside
    }

    /// Shortest distance from `p` to the contour (always positive).
    pub fn distance_to_boundary(&self, p: Vec2) -> f64 {
        let n = self.points.len();
        if n == 0 {
            return f64::INFINITY;
        }
        if n == 1 {
            return p.distance(self.points[0]);
        }
        let mut best = f64::INFINITY;
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + 1) % n];
            best = best.min(distance_point_segment(p, a, b));
        }
        best
    }

    /// Apply an affine transform: scale about the local origin, then rotate by
    /// `angle` (counter-clockwise), then translate by `offset`.
    pub fn transformed(&self, offset: Vec2, angle: f64, scale: f64) -> Polygon {
        let (s, c) = angle.sin_cos();
        Polygon::new(
            self.points
                .iter()
                .map(|p| {
                    let sx = p.x * scale;
                    let sy = p.y * scale;
                    Vec2::new(sx * c - sy * s, sx * s + sy * c) + offset
                })
                .collect(),
        )
    }

    /// Cumulative arc length at each vertex, closing back to the first point.
    /// Length is `n + 1`; the last entry is the perimeter.
    pub fn cumulative_arc_length(&self) -> Vec<f64> {
        let n = self.points.len();
        let mut s = Vec::with_capacity(n + 1);
        s.push(0.0);
        let mut acc = 0.0;
        for i in 0..n {
            acc += self.points[i].distance(self.points[(i + 1) % n]);
            s.push(acc);
        }
        s
    }
}

/// Distance from point `p` to segment `a→b`.
pub fn distance_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.norm_sq();
    if l2 <= 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Do open segments `p1→p2` and `p3→p4` cross? Shared endpoints and collinear
/// touching do **not** count, so adjacent polygon edges are not false positives.
pub fn segments_properly_intersect(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    let d1 = (p2 - p1).cross(p3 - p1);
    let d2 = (p2 - p1).cross(p4 - p1);
    let d3 = (p4 - p3).cross(p1 - p3);
    let d4 = (p4 - p3).cross(p2 - p3);
    // Strict sign change on both sides means a proper crossing.
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_ccw() -> Polygon {
        Polygon::new(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ])
    }

    #[test]
    fn signed_area_positive_for_ccw() {
        assert!((square_ccw().signed_area() - 1.0).abs() < 1e-12);
        assert_eq!(square_ccw().winding(), Winding::CounterClockwise);
    }

    #[test]
    fn selig_airfoil_ordering_is_counter_clockwise() {
        // TE → upper surface → LE → lower surface → TE is the common airfoil
        // file ordering; confirm our canonical orientation matches it so that
        // typical imports are not reversed.
        let p = Polygon::new(vec![
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 0.1),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.5, -0.1),
        ]);
        assert_eq!(p.winding(), Winding::CounterClockwise);
    }

    #[test]
    fn make_counter_clockwise_reverses_only_when_needed() {
        let mut p = square_ccw();
        assert!(!p.make_counter_clockwise());
        p.reverse();
        assert!(p.make_counter_clockwise());
        assert_eq!(p.winding(), Winding::CounterClockwise);
    }

    #[test]
    fn centroid_of_square_is_its_center() {
        let c = square_ccw().centroid();
        assert!((c.x - 0.5).abs() < 1e-12 && (c.y - 0.5).abs() < 1e-12);
    }

    #[test]
    fn contains_point_inside_and_outside() {
        let s = square_ccw();
        assert!(s.contains_point(Vec2::new(0.5, 0.5)));
        assert!(!s.contains_point(Vec2::new(1.5, 0.5)));
        assert!(!s.contains_point(Vec2::new(-0.1, 0.5)));
    }

    #[test]
    fn diameter_of_square_is_the_diagonal() {
        let (_, _, d) = square_ccw().diameter().unwrap();
        assert!((d - 2f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn perimeter_closes_the_contour() {
        assert!((square_ccw().perimeter() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn proper_intersection_ignores_shared_endpoints() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        let c = Vec2::new(1.0, 1.0);
        // Adjacent edges share b — not a self-intersection.
        assert!(!segments_properly_intersect(a, b, b, c));
        // A genuine X crossing.
        assert!(segments_properly_intersect(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 0.0)
        ));
    }

    #[test]
    fn transform_rotates_counter_clockwise_about_origin() {
        let p = Polygon::new(vec![Vec2::new(1.0, 0.0)]);
        let t = p.transformed(Vec2::ZERO, std::f64::consts::FRAC_PI_2, 1.0);
        assert!(t.points[0].y > 0.99 && t.points[0].x.abs() < 1e-12);
    }
}
