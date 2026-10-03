//! Trailing-edge detection and contour rotation.
//!
//! The Hess–Smith Kutta condition is imposed on the two panels adjacent to a
//! body's sharp trailing edge, which the formulation takes to be panels `0` and
//! `N−1`. That holds only if the contour *starts* at the trailing edge.
//! Conventional airfoil files do, but nothing guarantees it — a re-exported or
//! hand-edited file may start anywhere.
//!
//! So: find the sharpest corner, rotate the point list to begin there, and
//! report whether a sharp corner exists at all. A smooth body such as a
//! cylinder has none, in which case a Kutta condition is meaningless and the
//! solver falls back to zero circulation.

use crate::polygon::Polygon;
use crate::vec2::Vec2;
use std::f64::consts::PI;

/// A detected corner on a contour.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct CornerInfo {
    /// Vertex index on the contour as supplied.
    pub index: usize,
    /// How much the tangent turns at this vertex, radians in `[0, π]`.
    /// A cusp approaches `π`; a smooth point is near `0`.
    pub turn_angle: f64,
    /// Interior wedge angle, `π − turn_angle`. A cusp is `0`; a right-angle
    /// corner is `π/2`; a smooth point is `π`.
    pub included_angle: f64,
    pub position: Vec2,
}

/// Tangent turn above which a corner is sharp enough to carry a Kutta
/// condition: 30°, i.e. an included angle below 150°.
///
/// This separates real trailing edges from discretisation corners with a wide
/// margin: a 160-panel cylinder turns 2.25° per vertex, while a NACA 0012 with
/// a closed trailing edge turns by more than 170° there.
pub const SHARP_CORNER_THRESHOLD: f64 = 30.0 * PI / 180.0;

/// Turn angle at one vertex of a closed contour.
pub fn turn_angle_at(poly: &Polygon, index: usize) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let prev = poly.points[(index + n - 1) % n];
    let cur = poly.points[index];
    let next = poly.points[(index + 1) % n];
    let t_in = (cur - prev).normalized();
    let t_out = (next - cur).normalized();
    if t_in == Vec2::ZERO || t_out == Vec2::ZERO {
        return 0.0;
    }
    t_out.cross(t_in).atan2(t_out.dot(t_in)).abs()
}

/// The sharpest corner on the contour, if any vertex turns at all.
pub fn sharpest_corner(poly: &Polygon) -> Option<CornerInfo> {
    let n = poly.len();
    if n < 3 {
        return None;
    }
    let mut best: Option<CornerInfo> = None;
    for i in 0..n {
        let turn = turn_angle_at(poly, i);
        if best.is_none_or(|b| turn > b.turn_angle) {
            best = Some(CornerInfo {
                index: i,
                turn_angle: turn,
                included_angle: PI - turn,
                position: poly.points[i],
            });
        }
    }
    best
}

/// Result of preparing a contour for the panel solver.
#[derive(Debug, Clone)]
pub struct TrailingEdgeAnalysis {
    /// The contour rotated so index 0 is the trailing edge (unchanged when no
    /// sharp corner was found).
    pub polygon: Polygon,
    /// The detected trailing edge, if the contour has one.
    pub trailing_edge: Option<CornerInfo>,
    /// How far the point list was rotated.
    pub rotated_by: usize,
    /// Whether a Kutta condition can meaningfully be applied.
    pub kutta_applicable: bool,
    /// Second-sharpest corner, for diagnostics: when it is comparable to the
    /// sharpest, the detection is ambiguous and the user may want to override.
    pub runner_up_turn: f64,
}

/// Rotate `poly` so it begins at its sharpest corner and report whether that
/// corner is sharp enough to carry a Kutta condition.
pub fn analyse(poly: &Polygon) -> TrailingEdgeAnalysis {
    let n = poly.len();
    let Some(corner) = sharpest_corner(poly) else {
        return TrailingEdgeAnalysis {
            polygon: poly.clone(),
            trailing_edge: None,
            rotated_by: 0,
            kutta_applicable: false,
            runner_up_turn: 0.0,
        };
    };

    // Second sharpest, excluding the winner and its immediate neighbours (a
    // single sharp vertex inflates its neighbours' turn angles too).
    let mut runner_up = 0.0f64;
    for i in 0..n {
        let near = i == corner.index || (i + 1) % n == corner.index || (corner.index + 1) % n == i;
        if near {
            continue;
        }
        runner_up = runner_up.max(turn_angle_at(poly, i));
    }

    let sharp = corner.turn_angle >= SHARP_CORNER_THRESHOLD;
    if !sharp {
        return TrailingEdgeAnalysis {
            polygon: poly.clone(),
            trailing_edge: None,
            rotated_by: 0,
            kutta_applicable: false,
            runner_up_turn: runner_up,
        };
    }

    let rotated = rotate_to(poly, corner.index);
    TrailingEdgeAnalysis {
        polygon: rotated,
        trailing_edge: Some(CornerInfo { index: 0, ..corner }),
        rotated_by: corner.index,
        kutta_applicable: true,
        runner_up_turn: runner_up,
    }
}

/// Rotate the point list so that `start` becomes index 0, preserving order.
pub fn rotate_to(poly: &Polygon, start: usize) -> Polygon {
    let n = poly.len();
    if n == 0 || start.is_multiple_of(n) {
        return poly.clone();
    }
    let k = start % n;
    let mut pts = Vec::with_capacity(n);
    pts.extend_from_slice(&poly.points[k..]);
    pts.extend_from_slice(&poly.points[..k]);
    Polygon::new(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes;

    #[test]
    fn cylinder_has_no_sharp_corner_so_kutta_is_not_applicable() {
        let a = analyse(&shapes::circle(1.0, 160));
        assert!(!a.kutta_applicable);
        assert!(a.trailing_edge.is_none());
        assert_eq!(a.rotated_by, 0);
        // Turn per vertex of a 160-gon is 2π/160 = 2.25°.
        let c = sharpest_corner(&shapes::circle(1.0, 160)).unwrap();
        assert!(
            (c.turn_angle - 2.0 * PI / 160.0).abs() < 1e-9,
            "turn = {}",
            c.turn_angle.to_degrees()
        );
    }

    #[test]
    fn naca_trailing_edge_is_found_and_already_at_index_zero() {
        let poly = shapes::Naca4::default().generate(160);
        let a = analyse(&poly);
        assert!(a.kutta_applicable);
        let te = a.trailing_edge.unwrap();
        assert_eq!(te.index, 0);
        assert_eq!(
            a.rotated_by, 0,
            "generated contours already start at the TE"
        );
        assert!(
            (te.position.x - 1.0).abs() < 1e-9,
            "TE at {:?}",
            te.position
        );
        // A closed (cusped) trailing edge turns the tangent by nearly 180°.
        assert!(
            te.turn_angle.to_degrees() > 150.0,
            "{}",
            te.turn_angle.to_degrees()
        );
        assert!(te.included_angle.to_degrees() < 30.0);
    }

    #[test]
    fn rotated_airfoil_file_is_restored_to_start_at_the_trailing_edge() {
        let poly = shapes::Naca4::default().generate(160);
        let te_position = poly.points[0];
        // Simulate a file that begins somewhere on the upper surface.
        let shifted = rotate_to(&poly, 37);
        assert!(shifted.points[0].distance(te_position) > 0.1);

        let a = analyse(&shifted);
        assert!(a.kutta_applicable);
        assert!(
            a.polygon.points[0].distance(te_position) < 1e-12,
            "rotation did not restore the TE to index 0"
        );
        // Traversal order must be preserved, not just the starting point.
        let n = poly.len();
        let offset = (n - 37) % n;
        for i in 0..n {
            assert!(
                a.polygon.points[i].distance(poly.points[(i + offset + 37) % n]) < 1e-12
                    || a.polygon.points[i].distance(poly.points[i]) < 1e-12
            );
        }
    }

    #[test]
    fn rotation_preserves_winding_area_and_point_count() {
        let poly = shapes::Naca4 {
            max_camber: 0.04,
            ..Default::default()
        }
        .generate(120);
        let r = rotate_to(&poly, 53);
        assert_eq!(r.len(), poly.len());
        assert!((r.signed_area() - poly.signed_area()).abs() < 1e-12);
        assert!((r.perimeter() - poly.perimeter()).abs() < 1e-12);
    }

    #[test]
    fn wedge_corner_reports_the_right_included_angle() {
        // Right-angled isoceles triangle: the right angle has included angle
        // π/2, so the tangent turns by π/2 there.
        let tri = Polygon::new(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
        ]);
        let turn_at_origin = turn_angle_at(&tri, 0);
        assert!(
            (turn_at_origin - PI / 2.0).abs() < 1e-9,
            "turn = {}",
            turn_at_origin.to_degrees()
        );
        // The two 45° vertices turn by 135°.
        assert!((turn_angle_at(&tri, 1) - 3.0 * PI / 4.0).abs() < 1e-9);
    }

    #[test]
    fn the_sharpest_of_several_corners_wins() {
        // A diamond with one cusp-like tip and three blunter ones.
        let poly = Polygon::new(vec![
            Vec2::new(2.0, 0.0), // very sharp (thin tip)
            Vec2::new(0.0, 0.6),
            Vec2::new(-0.5, 0.0),
            Vec2::new(0.0, -0.6),
        ]);
        let a = analyse(&poly);
        let te = a.trailing_edge.unwrap();
        assert!(
            (te.position.x - 2.0).abs() < 1e-12,
            "picked {:?}",
            te.position
        );
        assert!(a.runner_up_turn < te.turn_angle);
    }

    #[test]
    fn open_trailing_edge_airfoil_is_still_detected() {
        let poly = shapes::Naca4 {
            trailing_edge: shapes::TrailingEdge::Open,
            ..Default::default()
        }
        .generate(160);
        let a = analyse(&poly);
        assert!(
            a.kutta_applicable,
            "a finite-thickness TE is still a sharp corner"
        );
        let te = a.trailing_edge.unwrap();
        // The blunt TE is bridged by a short closing panel, so the corner there
        // is less extreme than a cusp but still well past the threshold.
        assert!(te.turn_angle > SHARP_CORNER_THRESHOLD);
    }

    #[test]
    fn joukowski_trailing_edge_is_detected_at_the_cusp() {
        let poly = shapes::Joukowski {
            dx: 0.1,
            dy: 0.05,
            c: 1.0,
        }
        .generate(200);
        let a = analyse(&poly);
        assert!(a.kutta_applicable);
        let te = a.trailing_edge.unwrap();
        // The Joukowski map sends ζ = c to z = 2c.
        assert!(
            (te.position.x - 2.0).abs() < 1e-5,
            "TE at {:?}",
            te.position
        );
    }

    #[test]
    fn degenerate_contours_are_handled_without_panicking() {
        assert!(sharpest_corner(&Polygon::new(vec![])).is_none());
        assert!(sharpest_corner(&Polygon::new(vec![Vec2::ZERO, Vec2::X])).is_none());
        let a = analyse(&Polygon::new(vec![Vec2::ZERO]));
        assert!(!a.kutta_applicable);
    }

    #[test]
    fn rotate_to_is_a_no_op_for_index_zero_and_for_multiples_of_n() {
        let poly = shapes::circle(1.0, 16);
        assert_eq!(rotate_to(&poly, 0).points, poly.points);
        assert_eq!(rotate_to(&poly, 16).points, poly.points);
        assert_eq!(rotate_to(&poly, 32).points, poly.points);
    }
}
