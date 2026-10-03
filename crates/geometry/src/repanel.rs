//! Re-panelisation: redistribute points along an existing contour.
//!
//! Why this exists (a documented addition to the PRD's geometry pipeline):
//! panel-method error is dominated by how well the discretisation resolves
//! high-curvature regions. An airfoil file with 60 evenly spaced points
//! resolves the leading edge poorly, and the resulting `Cp` peak is visibly
//! wrong. Redistributing the *same* contour onto a cosine-clustered
//! arc-length grid cuts that error by roughly an order of magnitude for the
//! same panel count.
//!
//! Sharp corners (a wedge trailing edge, say) are detected and pinned as exact
//! breakpoints so re-panelisation never rounds off a feature the Kutta
//! condition depends on.

use crate::polygon::Polygon;
use crate::vec2::Vec2;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum PanelDistribution {
    /// Keep the imported points exactly as they are.
    AsImported,
    /// Equal arc length between panels. Best for smooth convex bodies.
    Uniform,
    /// Cosine clustering towards both ends of each corner-to-corner arc, and
    /// towards the contour's two extreme points (for an airfoil: the leading
    /// and trailing edge). Best for airfoils.
    Cosine,
    /// Arc length weighted by local curvature.
    Curvature,
    /// Pick [`PanelDistribution::Cosine`] when a sharp corner is present,
    /// otherwise [`PanelDistribution::Uniform`].
    Auto,
}

#[derive(Debug, Clone, Copy)]
pub struct RepanelConfig {
    pub distribution: PanelDistribution,
    /// Target panel count. Ignored by [`PanelDistribution::AsImported`].
    pub count: usize,
    /// Tangent turn above which a vertex is treated as a sharp corner and
    /// pinned exactly. Radians; 25° by default.
    pub corner_threshold: f64,
    /// Curvature weighting exponent for [`PanelDistribution::Curvature`].
    pub curvature_strength: f64,
}

impl Default for RepanelConfig {
    fn default() -> Self {
        Self {
            distribution: PanelDistribution::Auto,
            count: 160,
            corner_threshold: 25.0 * PI / 180.0,
            curvature_strength: 1.0,
        }
    }
}

/// Vertices whose tangent turns by more than `threshold` radians.
pub fn find_corners(poly: &Polygon, threshold: f64) -> Vec<usize> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut corners = Vec::new();
    for i in 0..n {
        let prev = poly.points[(i + n - 1) % n];
        let cur = poly.points[i];
        let next = poly.points[(i + 1) % n];
        let t_in = (cur - prev).normalized();
        let t_out = (next - cur).normalized();
        if t_in == Vec2::ZERO || t_out == Vec2::ZERO {
            continue;
        }
        let turn = t_out.cross(t_in).atan2(t_out.dot(t_in)).abs();
        if turn > threshold {
            corners.push(i);
        }
    }
    corners
}

/// Sample the contour at a given arc length, measured from `points[0]`.
fn sample_at_arc_length(poly: &Polygon, cum: &[f64], s: f64) -> Vec2 {
    let total = *cum.last().unwrap();
    if total <= 0.0 {
        return poly.points[0];
    }
    let s = s.rem_euclid(total);
    // `cum` is sorted; find the segment containing `s`.
    let idx = match cum.binary_search_by(|probe| probe.total_cmp(&s)) {
        Ok(i) => i.min(cum.len() - 2),
        Err(i) => (i.saturating_sub(1)).min(cum.len() - 2),
    };
    let seg_len = cum[idx + 1] - cum[idx];
    let n = poly.len();
    let a = poly.points[idx % n];
    let b = poly.points[(idx + 1) % n];
    if seg_len <= 0.0 {
        return a;
    }
    a.lerp(b, (s - cum[idx]) / seg_len)
}

/// Cosine-distributed parameters on `[0, 1]`, clustered at both ends.
/// `t_i = (1 − cos(π i / k)) / 2` for `i = 0..=k`.
fn cosine_params(k: usize) -> Vec<f64> {
    (0..=k)
        .map(|i| 0.5 * (1.0 - (PI * i as f64 / k as f64).cos()))
        .collect()
}

/// Discrete curvature estimate at each vertex, via the circumradius of the
/// vertex and its two neighbours.
fn vertex_curvature(poly: &Polygon) -> Vec<f64> {
    let n = poly.len();
    let mut k = vec![0.0; n];
    for i in 0..n {
        let a = poly.points[(i + n - 1) % n];
        let b = poly.points[i];
        let c = poly.points[(i + 1) % n];
        let ab = (b - a).norm();
        let bc = (c - b).norm();
        let ca = (a - c).norm();
        let area2 = (b - a).cross(c - a).abs();
        if ab * bc * ca > 0.0 {
            // κ = 4·Area / (|ab|·|bc|·|ca|) = 1 / circumradius
            k[i] = 2.0 * area2 / (ab * bc * ca);
        }
    }
    k
}

/// Redistribute the contour onto `cfg.count` points.
///
/// Returns the original polygon unchanged for [`PanelDistribution::AsImported`]
/// or when `count` is too small to be meaningful.
pub fn repanel(poly: &Polygon, cfg: &RepanelConfig) -> Polygon {
    let n = poly.len();
    if n < 3 || cfg.distribution == PanelDistribution::AsImported || cfg.count < 8 {
        return poly.clone();
    }

    let corners = find_corners(poly, cfg.corner_threshold);
    let distribution = match cfg.distribution {
        PanelDistribution::Auto => {
            if corners.is_empty() {
                PanelDistribution::Uniform
            } else {
                PanelDistribution::Cosine
            }
        }
        other => other,
    };

    let cum = poly.cumulative_arc_length();
    let total = *cum.last().unwrap();
    if total <= 0.0 {
        return poly.clone();
    }

    match distribution {
        PanelDistribution::AsImported | PanelDistribution::Auto => poly.clone(),

        PanelDistribution::Uniform => {
            // Pin corners, then fill each corner-to-corner arc uniformly.
            let breaks = break_arc_lengths(poly, &cum, &corners, total);
            let pts = fill_arcs(poly, &cum, &breaks, total, cfg.count, |k| {
                (0..=k).map(|i| i as f64 / k as f64).collect()
            });
            Polygon::new(pts)
        }

        PanelDistribution::Cosine => {
            let mut breaks = break_arc_lengths(poly, &cum, &corners, total);
            // Also pin the two extreme points (airfoil LE/TE) so the cosine
            // clustering tightens at the leading edge, not just at corners.
            if let Some((i, j, _)) = poly.diameter() {
                for idx in [i, j] {
                    let s = cum[idx];
                    if !breaks.iter().any(|b| (b - s).abs() < 1e-9 * total) {
                        breaks.push(s);
                    }
                }
                breaks.sort_by(f64::total_cmp);
            }
            let pts = fill_arcs(poly, &cum, &breaks, total, cfg.count, cosine_params);
            Polygon::new(pts)
        }

        PanelDistribution::Curvature => {
            let pts = curvature_distribution(poly, &cum, total, cfg);
            Polygon::new(pts)
        }
    }
}

/// Arc lengths of the pinned breakpoints, always including `s = 0`.
fn break_arc_lengths(_poly: &Polygon, cum: &[f64], corners: &[usize], total: f64) -> Vec<f64> {
    let mut breaks: Vec<f64> = corners.iter().map(|&i| cum[i]).collect();
    if breaks.is_empty() {
        breaks.push(0.0);
    }
    breaks.sort_by(f64::total_cmp);
    breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-12 * total.max(1.0));
    breaks
}

/// Distribute `count` points over the arcs between consecutive breakpoints,
/// allocating points to each arc in proportion to its length and placing them
/// according to `params(k)` (a parameterisation of `[0, 1]`).
fn fill_arcs(
    poly: &Polygon,
    cum: &[f64],
    breaks: &[f64],
    total: f64,
    count: usize,
    params: impl Fn(usize) -> Vec<f64>,
) -> Vec<Vec2> {
    let m = breaks.len();
    // Arc i runs from breaks[i] to breaks[i+1] (wrapping, measured forwards).
    let arc_lengths: Vec<f64> = (0..m)
        .map(|i| {
            let a = breaks[i];
            let b = if i + 1 < m {
                breaks[i + 1]
            } else {
                breaks[0] + total
            };
            b - a
        })
        .collect();

    // Proportional allocation with at least one panel per arc.
    let mut alloc: Vec<usize> = arc_lengths
        .iter()
        .map(|l| ((l / total) * count as f64).round().max(1.0) as usize)
        .collect();
    // Reconcile rounding against the requested total.
    let mut sum: usize = alloc.iter().sum();
    while sum > count && alloc.iter().any(|&a| a > 1) {
        let i = alloc
            .iter()
            .enumerate()
            .filter(|(_, &a)| a > 1)
            .max_by(|a, b| {
                (arc_lengths[a.0] / *a.1 as f64).total_cmp(&(arc_lengths[b.0] / *b.1 as f64))
            })
            .map(|(i, _)| i)
            .unwrap();
        alloc[i] -= 1;
        sum -= 1;
    }
    while sum < count {
        let i = (0..m)
            .max_by(|&a, &b| {
                (arc_lengths[a] / alloc[a] as f64).total_cmp(&(arc_lengths[b] / alloc[b] as f64))
            })
            .unwrap();
        alloc[i] += 1;
        sum += 1;
    }

    let mut out: Vec<Vec2> = Vec::with_capacity(count);
    for i in 0..m {
        let s0 = breaks[i];
        let len = arc_lengths[i];
        let k = alloc[i];
        let ts = params(k);
        // Take `0..k`, dropping the arc's final point: it is the next arc's
        // first point, which keeps the closed contour free of duplicates.
        for t in ts.iter().take(k) {
            out.push(sample_at_arc_length(poly, cum, s0 + t * len));
        }
    }
    out
}

/// Place points so that arc length is consumed faster in flat regions and more
/// slowly where the contour curves.
fn curvature_distribution(
    poly: &Polygon,
    cum: &[f64],
    total: f64,
    cfg: &RepanelConfig,
) -> Vec<Vec2> {
    let n = poly.len();
    let kappa = vertex_curvature(poly);
    let scale = poly.bounds().map(|b| b.diagonal()).unwrap_or(1.0);
    // Non-dimensional, smoothed density weight per original segment.
    let mut w = Vec::with_capacity(n);
    for i in 0..n {
        let k_avg = 0.5 * (kappa[i] + kappa[(i + 1) % n]) * scale;
        w.push((1.0 + cfg.curvature_strength * k_avg).powf(0.5));
    }
    // Cumulative "weighted arc length".
    let mut wcum = vec![0.0f64; n + 1];
    for i in 0..n {
        let seg = cum[i + 1] - cum[i];
        wcum[i + 1] = wcum[i] + seg * w[i];
    }
    let wtotal = wcum[n];
    if wtotal <= 0.0 {
        return poly.points.clone();
    }

    let count = cfg.count;
    let mut out = Vec::with_capacity(count);
    for j in 0..count {
        let target = wtotal * j as f64 / count as f64;
        // Invert the weighted-arc-length map.
        let idx = match wcum.binary_search_by(|p| p.total_cmp(&target)) {
            Ok(i) => i.min(n - 1),
            Err(i) => i.saturating_sub(1).min(n - 1),
        };
        let dw = wcum[idx + 1] - wcum[idx];
        let frac = if dw > 0.0 {
            (target - wcum[idx]) / dw
        } else {
            0.0
        };
        let s = cum[idx] + frac * (cum[idx + 1] - cum[idx]);
        out.push(sample_at_arc_length(poly, cum, s.min(total)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(n: usize) -> Polygon {
        Polygon::new(
            (0..n)
                .map(|i| {
                    let t = 2.0 * PI * i as f64 / n as f64;
                    Vec2::new(t.cos(), t.sin())
                })
                .collect(),
        )
    }

    /// A thin diamond: sharp corners at the left and right tips.
    fn diamond() -> Polygon {
        Polygon::new(vec![
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.1),
            Vec2::new(-1.0, 0.0),
            Vec2::new(0.0, -0.1),
        ])
    }

    #[test]
    fn as_imported_is_a_no_op() {
        let c = circle(20);
        let r = repanel(
            &c,
            &RepanelConfig {
                distribution: PanelDistribution::AsImported,
                ..Default::default()
            },
        );
        assert_eq!(r.points, c.points);
    }

    #[test]
    fn uniform_hits_the_requested_count_and_stays_on_the_circle() {
        let out = repanel(
            &circle(200),
            &RepanelConfig {
                distribution: PanelDistribution::Uniform,
                count: 64,
                ..Default::default()
            },
        );
        assert_eq!(out.len(), 64);
        for p in &out.points {
            // Chords of the sampled 200-gon sit just inside the unit circle.
            assert!((p.norm() - 1.0).abs() < 2e-4, "r = {}", p.norm());
        }
    }

    #[test]
    fn uniform_segments_are_nearly_equal_in_length() {
        let out = repanel(
            &circle(400),
            &RepanelConfig {
                distribution: PanelDistribution::Uniform,
                count: 50,
                ..Default::default()
            },
        );
        let cum = out.cumulative_arc_length();
        let lens: Vec<f64> = (0..out.len()).map(|i| cum[i + 1] - cum[i]).collect();
        let mn = lens.iter().copied().fold(f64::INFINITY, f64::min);
        let mx = lens.iter().copied().fold(0.0, f64::max);
        assert!(mx / mn < 1.02, "ratio = {}", mx / mn);
    }

    #[test]
    fn corners_are_detected_on_a_diamond_and_not_on_a_circle() {
        // The diamond's two tips turn by 168.6°; its mid vertices turn by only
        // 11.4°, so the default 25° threshold must find exactly the two tips.
        let tips = find_corners(&diamond(), 25.0 * PI / 180.0);
        assert_eq!(tips.len(), 2, "{tips:?}");
        assert!(tips.contains(&0) && tips.contains(&2));
        // Lowering the threshold below 11.4° picks up all four vertices.
        assert_eq!(find_corners(&diamond(), 10.0 * PI / 180.0).len(), 4);
        assert!(find_corners(&circle(64), 25.0 * PI / 180.0).is_empty());
    }

    #[test]
    fn auto_picks_uniform_for_smooth_bodies_and_cosine_for_cornered_ones() {
        // Cosine clustering produces unequal panels; uniform does not.
        let smooth = repanel(
            &circle(400),
            &RepanelConfig {
                count: 60,
                ..Default::default()
            },
        );
        let cum = smooth.cumulative_arc_length();
        let lens: Vec<f64> = (0..smooth.len()).map(|i| cum[i + 1] - cum[i]).collect();
        let ratio = lens.iter().copied().fold(0.0, f64::max)
            / lens.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(ratio < 1.05, "smooth body should get uniform panels");

        let cornered = repanel(
            &diamond(),
            &RepanelConfig {
                count: 60,
                ..Default::default()
            },
        );
        let cum = cornered.cumulative_arc_length();
        let lens: Vec<f64> = (0..cornered.len()).map(|i| cum[i + 1] - cum[i]).collect();
        let ratio = lens.iter().copied().fold(0.0, f64::max)
            / lens.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(ratio > 3.0, "cornered body should cluster, ratio = {ratio}");
    }

    #[test]
    fn sharp_corners_survive_repanelisation_exactly() {
        let d = diamond();
        let out = repanel(
            &d,
            &RepanelConfig {
                count: 80,
                ..Default::default()
            },
        );
        for corner in &d.points {
            let hit = out.points.iter().any(|p| p.distance(*corner) < 1e-9);
            assert!(hit, "corner {corner:?} was rounded off");
        }
    }

    #[test]
    fn cosine_clusters_towards_pinned_breakpoints() {
        let out = repanel(
            &circle(400),
            &RepanelConfig {
                distribution: PanelDistribution::Cosine,
                count: 80,
                ..Default::default()
            },
        );
        let cum = out.cumulative_arc_length();
        let lens: Vec<f64> = (0..out.len()).map(|i| cum[i + 1] - cum[i]).collect();
        // The panel adjacent to a pinned breakpoint must be shorter than one
        // in the middle of an arc.
        assert!(lens[0] < lens[out.len() / 4]);
    }

    #[test]
    fn curvature_mode_puts_more_points_where_the_contour_bends() {
        // Ellipse with 8:1 aspect ratio — curvature peaks at the two ends.
        let ell = Polygon::new(
            (0..400)
                .map(|i| {
                    let t = 2.0 * PI * i as f64 / 400.0;
                    Vec2::new(4.0 * t.cos(), 0.5 * t.sin())
                })
                .collect(),
        );
        let by_curvature = repanel(
            &ell,
            &RepanelConfig {
                distribution: PanelDistribution::Curvature,
                count: 100,
                ..Default::default()
            },
        );
        // Compare against *uniform arc length* on the same contour. Comparing
        // against the input's own spacing would be meaningless here: the input
        // is spaced by eccentric anomaly, which already clusters at the caps.
        let by_arc_length = repanel(
            &ell,
            &RepanelConfig {
                distribution: PanelDistribution::Uniform,
                count: 100,
                ..Default::default()
            },
        );
        let caps = |p: &Polygon| p.points.iter().filter(|q| q.x.abs() > 3.5).count();
        assert!(
            caps(&by_curvature) > caps(&by_arc_length) + 5,
            "curvature caps = {}, uniform caps = {}",
            caps(&by_curvature),
            caps(&by_arc_length)
        );
    }

    #[test]
    fn repanelisation_preserves_area_closely() {
        let c = circle(500);
        let out = repanel(
            &c,
            &RepanelConfig {
                distribution: PanelDistribution::Uniform,
                count: 120,
                ..Default::default()
            },
        );
        assert!((out.area() - PI).abs() / PI < 1e-3);
    }

    #[test]
    fn count_below_the_minimum_returns_the_input_untouched() {
        let c = circle(20);
        let out = repanel(
            &c,
            &RepanelConfig {
                count: 4,
                ..Default::default()
            },
        );
        assert_eq!(out.points, c.points);
    }
}
