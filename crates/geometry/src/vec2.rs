//! Two-dimensional vector arithmetic in `f64`.
//!
//! Coordinate conventions for the whole application (PRD §14.1):
//! `+x` points right, `+y` points up, positive angles are counter-clockwise.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A point or vector in the simulation plane.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Unit vector at `angle` radians counter-clockwise from `+x`.
    #[inline]
    pub fn from_angle(angle: f64) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    #[inline]
    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }

    /// Scalar cross product `self × o = sx·oy − sy·ox` (the `z` component of the
    /// 3D cross product). Positive when `o` is counter-clockwise from `self`.
    #[inline]
    pub fn cross(self, o: Vec2) -> f64 {
        self.x * o.y - self.y * o.x
    }

    #[inline]
    pub fn norm_sq(self) -> f64 {
        self.dot(self)
    }

    #[inline]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Unit vector in the same direction; returns [`Vec2::ZERO`] for a zero vector
    /// so callers never propagate NaN.
    #[inline]
    pub fn normalized(self) -> Self {
        let n = self.norm();
        if n > 0.0 {
            self / n
        } else {
            Self::ZERO
        }
    }

    /// Rotate counter-clockwise by `angle` radians.
    #[inline]
    pub fn rotate(self, angle: f64) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// Rotate counter-clockwise by `angle` about `pivot`.
    #[inline]
    pub fn rotate_about(self, pivot: Vec2, angle: f64) -> Self {
        (self - pivot).rotate(angle) + pivot
    }

    /// Rotate by +90° (counter-clockwise): the *left* normal of this direction.
    #[inline]
    pub fn perp_left(self) -> Self {
        Self::new(-self.y, self.x)
    }

    /// Rotate by −90° (clockwise): the *right* normal of this direction.
    ///
    /// For a counter-clockwise polygon traversed start→end, this is the
    /// **outward** normal — the convention used throughout the solver.
    #[inline]
    pub fn perp_right(self) -> Self {
        Self::new(self.y, -self.x)
    }

    #[inline]
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }

    #[inline]
    pub fn distance(self, o: Vec2) -> f64 {
        (self - o).norm()
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    #[inline]
    pub fn lerp(self, o: Vec2, t: f64) -> Self {
        self + (o - self) * t
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f64) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Div<f64> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn div(self, s: f64) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    #[inline]
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl Bounds {
    pub fn from_points(points: &[Vec2]) -> Option<Self> {
        let first = *points.first()?;
        let mut b = Bounds {
            min: first,
            max: first,
        };
        for p in &points[1..] {
            b.min.x = b.min.x.min(p.x);
            b.min.y = b.min.y.min(p.y);
            b.max.x = b.max.x.max(p.x);
            b.max.y = b.max.y.max(p.y);
        }
        Some(b)
    }

    #[inline]
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    #[inline]
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    #[inline]
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    #[inline]
    pub fn diagonal(&self) -> f64 {
        (self.max - self.min).norm()
    }

    /// Grow the box by `margin` in every direction.
    #[inline]
    pub fn expanded(&self, margin: f64) -> Self {
        Bounds {
            min: self.min - Vec2::new(margin, margin),
            max: self.max + Vec2::new(margin, margin),
        }
    }

    #[inline]
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn union(&self, o: &Bounds) -> Self {
        Bounds {
            min: Vec2::new(self.min.x.min(o.min.x), self.min.y.min(o.min.y)),
            max: Vec2::new(self.max.x.max(o.max.x), self.max.y.max(o.max.y)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perp_right_is_outward_for_ccw_traversal() {
        // Bottom edge of a counter-clockwise unit square runs +x; outward is −y.
        let tangent = Vec2::X;
        assert_eq!(tangent.perp_right(), Vec2::new(0.0, -1.0));
        // Left edge runs −y; outward is −x.
        let tangent = Vec2::new(0.0, -1.0);
        assert_eq!(tangent.perp_right(), Vec2::new(-1.0, 0.0));
    }

    #[test]
    fn rotation_is_counter_clockwise() {
        let r = Vec2::X.rotate(std::f64::consts::FRAC_PI_2);
        assert!((r.x - 0.0).abs() < 1e-12);
        assert!((r.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn normalized_zero_vector_is_zero_not_nan() {
        assert_eq!(Vec2::ZERO.normalized(), Vec2::ZERO);
    }

    #[test]
    fn cross_sign_follows_ccw() {
        assert!(Vec2::X.cross(Vec2::Y) > 0.0);
        assert!(Vec2::Y.cross(Vec2::X) < 0.0);
    }
}
