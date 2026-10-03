//! Parametric geometry generation.
//!
//! PRD §32 lists geometry creation as a future feature, but §57 requires the
//! bundled examples to open "without uploading anything" and §81/§82 mandate a
//! cylinder and a NACA 0012 showcase. Those are not reachable with an
//! import-only pipeline, so a small generator is part of the MVP.
//!
//! All generators emit a closed, counter-clockwise contour with no repeated
//! closing point, matching the canonical solver orientation.

use crate::polygon::Polygon;
use crate::vec2::Vec2;
use std::f64::consts::PI;

/// Points on a circle of radius `r` centred at the origin, counter-clockwise.
pub fn circle(radius: f64, n: usize) -> Polygon {
    let n = n.max(8);
    Polygon::new(
        (0..n)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / n as f64;
                Vec2::new(radius * t.cos(), radius * t.sin())
            })
            .collect(),
    )
}

/// Ellipse with semi-axes `a` (x) and `b` (y), counter-clockwise.
///
/// Points are spaced by eccentric anomaly, which already clusters them towards
/// the high-curvature ends — exactly where a panel method needs them.
pub fn ellipse(a: f64, b: f64, n: usize) -> Polygon {
    let n = n.max(8);
    Polygon::new(
        (0..n)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / n as f64;
                Vec2::new(a * t.cos(), b * t.sin())
            })
            .collect(),
    )
}

/// Trailing-edge treatment for generated airfoils.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum TrailingEdge {
    /// Thickness coefficient −0.1036: `y_t(1) = 0`, so the contour closes to a
    /// cusp. Required for a panel method, which needs a closed body.
    Closed,
    /// The textbook coefficient −0.1015, which leaves a finite TE gap. The gap
    /// is bridged by a straight closing panel.
    Open,
}

/// A NACA 4-digit section, e.g. `2412` → `m = 0.02, p = 0.4, t = 0.12`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Naca4 {
    /// Maximum camber as a fraction of chord.
    pub max_camber: f64,
    /// Chordwise position of maximum camber, fraction of chord.
    pub camber_position: f64,
    /// Maximum thickness as a fraction of chord.
    pub thickness: f64,
    pub chord: f64,
    pub trailing_edge: TrailingEdge,
}

impl Default for Naca4 {
    fn default() -> Self {
        // NACA 0012: the canonical validation and demonstration section.
        Self {
            max_camber: 0.0,
            camber_position: 0.4,
            thickness: 0.12,
            chord: 1.0,
            trailing_edge: TrailingEdge::Closed,
        }
    }
}

impl Naca4 {
    /// Parse a 4-digit designation such as `"0012"`, `"2412"` or `"naca 4412"`.
    pub fn parse(code: &str) -> Option<Self> {
        let digits: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() != 4 {
            return None;
        }
        let m = digits[0..1].parse::<f64>().ok()? / 100.0;
        let p = digits[1..2].parse::<f64>().ok()? / 10.0;
        let t = digits[2..4].parse::<f64>().ok()? / 100.0;
        if t <= 0.0 {
            return None;
        }
        Some(Self {
            max_camber: m,
            // A symmetric section has no camber position; keep the standard 0.4
            // so the camber formulas never divide by zero.
            camber_position: if p > 0.0 { p } else { 0.4 },
            thickness: t,
            chord: 1.0,
            trailing_edge: TrailingEdge::Closed,
        })
    }

    /// Half-thickness distribution `y_t(x)` for `x` in `[0, 1]`.
    fn half_thickness(&self, x: f64) -> f64 {
        let a4 = match self.trailing_edge {
            TrailingEdge::Closed => -0.1036,
            TrailingEdge::Open => -0.1015,
        };
        5.0 * self.thickness
            * (0.2969 * x.max(0.0).sqrt() - 0.1260 * x - 0.3516 * x * x
                + 0.2843 * x * x * x
                + a4 * x * x * x * x)
    }

    /// Mean camber line and its slope at `x` in `[0, 1]`.
    fn camber(&self, x: f64) -> (f64, f64) {
        let m = self.max_camber;
        let p = self.camber_position;
        if m == 0.0 || p <= 0.0 || p >= 1.0 {
            return (0.0, 0.0);
        }
        if x < p {
            let yc = m / (p * p) * (2.0 * p * x - x * x);
            let dy = 2.0 * m / (p * p) * (p - x);
            (yc, dy)
        } else {
            let q = 1.0 - p;
            let yc = m / (q * q) * ((1.0 - 2.0 * p) + 2.0 * p * x - x * x);
            let dy = 2.0 * m / (q * q) * (p - x);
            (yc, dy)
        }
    }

    /// Generate the section contour with `n` points, ordered trailing edge →
    /// upper surface → leading edge → lower surface → trailing edge
    /// (counter-clockwise, which matches the canonical solver orientation and
    /// the common Selig file convention).
    ///
    /// Chordwise stations use cosine spacing, clustering points at the leading
    /// edge where curvature is highest.
    pub fn generate(&self, n: usize) -> Polygon {
        // Exactly `n` points: `upper` stations including both the trailing and
        // leading edge, then `lower` interior stations excluding both, so the
        // closed contour has no duplicates. For even `n` the two surfaces use
        // identical cosine stations, keeping symmetric sections symmetric.
        let n = n.max(16);
        let upper = n / 2 + 1;
        let lower = n - upper;
        let mut pts: Vec<Vec2> = Vec::with_capacity(n);

        // Upper surface: trailing edge (x = 1) back to leading edge (x = 0).
        for i in 0..upper {
            let beta = PI * i as f64 / (upper - 1) as f64; // 0 → π
            let x = 0.5 * (1.0 + beta.cos()); // 1 → 0
            pts.push(self.surface_point(x, true));
        }
        // Lower surface: leading edge forward to the trailing edge, interior only.
        for i in 1..=lower {
            let beta = PI * i as f64 / (lower + 1) as f64;
            let x = 0.5 * (1.0 - beta.cos()); // 0 → 1
            pts.push(self.surface_point(x, false));
        }

        let mut poly = Polygon::new(pts.iter().map(|p| *p * self.chord).collect());
        // Generation order is already counter-clockwise; assert it cheaply and
        // repair rather than trusting the derivation.
        poly.make_counter_clockwise();
        poly
    }

    fn surface_point(&self, x: f64, upper: bool) -> Vec2 {
        let yt = self.half_thickness(x);
        let (yc, dyc) = self.camber(x);
        let theta = dyc.atan();
        let (s, c) = theta.sin_cos();
        if upper {
            Vec2::new(x - yt * s, yc + yt * c)
        } else {
            Vec2::new(x + yt * s, yc - yt * c)
        }
    }
}

/// A Joukowski airfoil from the conformal map `z = ζ + c²/ζ`.
///
/// Useful as an *analytical* validation target: lift and the full surface
/// velocity are known in closed form, so a panel solution can be checked
/// against theory on a cambered, lifting shape rather than only on a cylinder.
#[derive(Debug, Clone, Copy)]
pub struct Joukowski {
    /// Circle-centre offset along `−x`; controls thickness.
    pub dx: f64,
    /// Circle-centre offset along `+y`; controls camber.
    pub dy: f64,
    /// Map parameter. The circle must pass through `ζ = c` for a sharp TE.
    pub c: f64,
}

impl Default for Joukowski {
    fn default() -> Self {
        Self {
            dx: 0.1,
            dy: 0.0,
            c: 1.0,
        }
    }
}

impl Joukowski {
    /// Radius of the pre-image circle, fixed by the sharp trailing-edge
    /// requirement that the circle passes through `ζ = c`.
    pub fn radius(&self) -> f64 {
        ((self.c + self.dx).powi(2) + self.dy * self.dy).sqrt()
    }

    pub fn generate(&self, n: usize) -> Polygon {
        let n = n.max(16);
        let r = self.radius();
        let centre = (-self.dx, self.dy);
        let c2 = self.c * self.c;
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            // Start at the trailing edge (ζ = c) and sweep counter-clockwise.
            // The sharp trailing edge is the map's image of ζ = +c, which sits
            // *below* the circle centre when dy > 0 — hence the negated dy.
            let t0 = (-self.dy).atan2(self.c + self.dx);
            let t = t0 + 2.0 * PI * i as f64 / n as f64;
            let zx = centre.0 + r * t.cos();
            let zy = centre.1 + r * t.sin();
            let den = zx * zx + zy * zy;
            if den <= 0.0 {
                continue;
            }
            // z = ζ + c²/ζ  with  c²/ζ = c²·conj(ζ)/|ζ|²
            pts.push(Vec2::new(zx + c2 * zx / den, zy - c2 * zy / den));
        }
        let mut poly = Polygon::new(pts);
        poly.make_counter_clockwise();
        poly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::Winding;

    #[test]
    fn circle_area_converges_to_pi_r_squared() {
        let p = circle(2.0, 400);
        assert!((p.area() - PI * 4.0).abs() / (PI * 4.0) < 1e-4);
        assert_eq!(p.winding(), Winding::CounterClockwise);
    }

    #[test]
    fn ellipse_area_is_pi_a_b() {
        let p = ellipse(3.0, 1.0, 600);
        assert!((p.area() - PI * 3.0).abs() / (PI * 3.0) < 1e-4);
    }

    #[test]
    fn naca_parse_reads_camber_and_thickness() {
        let a = Naca4::parse("2412").unwrap();
        assert!((a.max_camber - 0.02).abs() < 1e-12);
        assert!((a.camber_position - 0.4).abs() < 1e-12);
        assert!((a.thickness - 0.12).abs() < 1e-12);
        assert!(Naca4::parse("naca 0012").is_some());
        assert!(Naca4::parse("abc").is_none());
        assert!(Naca4::parse("12345").is_none());
        // Zero thickness is not a body.
        assert!(Naca4::parse("0000").is_none());
    }

    #[test]
    fn naca0012_max_thickness_is_twelve_percent() {
        let a = Naca4::default();
        // y_t peaks near x/c = 0.3 for a 4-digit section.
        let peak = (0..=1000)
            .map(|i| 2.0 * a.half_thickness(i as f64 / 1000.0))
            .fold(0.0f64, f64::max);
        assert!((peak - 0.12).abs() < 2e-3, "peak t/c = {peak}");
    }

    #[test]
    fn closed_trailing_edge_actually_closes() {
        let a = Naca4::default();
        assert!(a.half_thickness(1.0).abs() < 1e-12);
        let open = Naca4 {
            trailing_edge: TrailingEdge::Open,
            ..Default::default()
        };
        assert!(open.half_thickness(1.0).abs() > 1e-4);
    }

    #[test]
    fn naca_contour_is_counter_clockwise_and_closed() {
        let p = Naca4::default().generate(160);
        assert_eq!(p.winding(), Winding::CounterClockwise);
        // No duplicate points anywhere on the closed contour.
        let n = p.len();
        for i in 0..n {
            let d = p.points[i].distance(p.points[(i + 1) % n]);
            assert!(d > 1e-9, "duplicate at index {i}");
        }
    }

    #[test]
    fn naca_emits_exactly_the_requested_point_count() {
        for n in [16usize, 17, 120, 160, 999, 1000] {
            assert_eq!(Naca4::default().generate(n).len(), n, "n = {n}");
        }
    }

    #[test]
    fn symmetric_naca_with_even_count_is_mirror_symmetric() {
        let p = Naca4::default().generate(120);
        let sum_y: f64 = p.points.iter().map(|q| q.y).sum();
        assert!(sum_y.abs() < 1e-12, "sum y = {sum_y}");
    }

    #[test]
    fn naca_starts_at_the_trailing_edge() {
        let p = Naca4::default().generate(120);
        assert!((p.points[0].x - 1.0).abs() < 1e-9);
        assert!(p.points[0].y.abs() < 1e-9);
    }

    #[test]
    fn cambered_naca_has_positive_mean_camber() {
        let a = Naca4 {
            max_camber: 0.04,
            camber_position: 0.4,
            ..Default::default()
        };
        let (yc, _) = a.camber(0.4);
        assert!((yc - 0.04).abs() < 1e-12, "camber peak = {yc}");
        // Camber line is continuous at x = p.
        let (lo, _) = a.camber(0.4 - 1e-9);
        let (hi, _) = a.camber(0.4 + 1e-9);
        assert!((lo - hi).abs() < 1e-7);
        // Slope is continuous at x = p as well (it is zero there).
        let (_, dlo) = a.camber(0.4 - 1e-9);
        let (_, dhi) = a.camber(0.4 + 1e-9);
        assert!((dlo - dhi).abs() < 1e-7);
    }

    #[test]
    fn chord_scaling_scales_the_contour() {
        let a = Naca4 {
            chord: 2.5,
            ..Default::default()
        };
        let b = a.generate(100).bounds().unwrap();
        assert!((b.width() - 2.5).abs() < 1e-6);
    }

    #[test]
    fn joukowski_is_closed_ccw_and_has_a_sharp_trailing_edge() {
        let j = Joukowski {
            dx: 0.1,
            dy: 0.08,
            c: 1.0,
        };
        let p = j.generate(240);
        assert_eq!(p.winding(), Winding::CounterClockwise);
        // The map sends ζ = c to z = 2c: the trailing-edge point.
        let te_hit = p
            .points
            .iter()
            .any(|q| (q.x - 2.0).abs() < 1e-6 && q.y.abs() < 1e-6);
        assert!(te_hit, "trailing edge at z = 2c not present");
        assert!(p.area() > 0.0);
    }

    #[test]
    fn symmetric_joukowski_is_symmetric_about_y_eq_0() {
        let p = Joukowski {
            dx: 0.1,
            dy: 0.0,
            c: 1.0,
        }
        .generate(200);
        let sum_y: f64 = p.points.iter().map(|q| q.y).sum();
        assert!(sum_y.abs() < 1e-9, "sum y = {sum_y}");
    }
}
