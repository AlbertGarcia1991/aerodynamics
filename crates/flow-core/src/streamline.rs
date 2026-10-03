//! Streamline tracing (PRD §18, DEC-002).
//!
//! Streamlines are obtained by integrating the velocity field, **not** by
//! contouring the stream function. Two reasons:
//!
//! 1. `ψ` is multivalued whenever the scene has net source strength, so
//!    contours break across the branch cut.
//! 2. Contouring across body interiors needs special handling, while an
//!    integrator simply stops at the wall.
//!
//! Integration uses classical RK4 on the *normalised* velocity field, so the
//! parameter is arc length and sample points come out evenly spaced — which is
//! what both the renderer and the particle animation want. Reparameterising
//! does not change the geometric streamline.

use crate::field::{FieldEvalOptions, FlowField};
use aeroflow_geometry::{Bounds, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Termination {
    LeftDomain,
    HitBody,
    /// The field speed fell below the cutoff: a stagnation point.
    Stagnation,
    MaxSteps,
    /// The line returned to its own start, as it does around a vortex.
    Closed,
    /// The field produced a non-finite value.
    NonFinite,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Streamline {
    /// Ordered polyline. For a two-sided trace, the backward branch is
    /// reversed and prepended so the result reads upstream → downstream.
    pub points: Vec<Vec2>,
    /// Flow speed at each point, for colour-by-speed rendering.
    pub speeds: Vec<f32>,
    pub arc_length: f64,
    pub termination_forward: Termination,
    pub termination_backward: Option<Termination>,
    /// Index of the seed point within `points`.
    pub seed_index: usize,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct StreamlineConfig {
    pub bounds: Bounds,
    /// Arc-length step. Defaults to 1/400 of the domain diagonal.
    pub step: f64,
    pub max_steps: usize,
    /// Speed below which integration stops, as a fraction of a reference speed.
    pub min_speed: f64,
    pub both_directions: bool,
    /// Stop when the line comes back within this distance of its start.
    pub loop_tolerance: f64,
    /// Field evaluation used for integration. [`FieldEvalOptions::EXACT`] by
    /// default; the renderer opts into the far-field approximation, which makes
    /// tracing cost independent of panel count. Streamlines are a visualisation
    /// — no reported number is derived from them — so this is the same
    /// trade-off the scalar render path makes.
    #[cfg_attr(feature = "serde", serde(default))]
    pub eval: FieldEvalOptions,
}

impl StreamlineConfig {
    pub fn for_bounds(bounds: Bounds) -> Self {
        let diag = bounds.diagonal().max(1e-9);
        Self {
            bounds,
            step: diag / 400.0,
            max_steps: 4000,
            min_speed: 1e-6,
            both_directions: true,
            loop_tolerance: diag / 800.0,
            eval: FieldEvalOptions::EXACT,
        }
    }

    /// Rendering preset: far-field approximation on, no singularity softening
    /// (streamlines should still curl tightly around point vortices).
    pub fn for_rendering(bounds: Bounds) -> Self {
        Self {
            eval: FieldEvalOptions {
                core_radius: 0.0,
                far_field_ratio: 12.0,
                wall_margin: 0.0,
            },
            ..Self::for_bounds(bounds)
        }
    }
}

/// Trace one streamline through `seed`.
pub fn trace(field: &FlowField, seed: Vec2, cfg: &StreamlineConfig) -> Streamline {
    let opts = cfg.eval;
    let (fwd, t_fwd) = integrate(field, seed, cfg, 1.0, &opts);
    // A closed orbit — a streamline encircling a vortex, say — is already the
    // complete curve after the forward pass. Integrating backwards would
    // retrace the same loop and double its reported arc length.
    if !cfg.both_directions || t_fwd == Termination::Closed {
        let speeds = fwd
            .iter()
            .map(|p| field.velocity_with(*p, &cfg.eval).norm() as f32)
            .collect();
        let arc = polyline_length(&fwd);
        return Streamline {
            points: fwd,
            speeds,
            arc_length: arc,
            termination_forward: t_fwd,
            termination_backward: None,
            seed_index: 0,
        };
    }

    let (bwd, t_bwd) = integrate(field, seed, cfg, -1.0, &opts);
    // Reverse the upstream branch and drop its duplicated seed point.
    let mut points: Vec<Vec2> = bwd.into_iter().skip(1).rev().collect();
    let seed_index = points.len();
    points.extend(fwd);
    let speeds = points
        .iter()
        .map(|p| field.velocity_with(*p, &cfg.eval).norm() as f32)
        .collect();
    let arc = polyline_length(&points);
    Streamline {
        points,
        speeds,
        arc_length: arc,
        termination_forward: t_fwd,
        termination_backward: Some(t_bwd),
        seed_index,
    }
}

fn polyline_length(p: &[Vec2]) -> f64 {
    p.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// RK4 on the unit-direction field `V/|V|`, stepped by arc length.
fn integrate(
    field: &FlowField,
    seed: Vec2,
    cfg: &StreamlineConfig,
    sign: f64,
    opts: &FieldEvalOptions,
) -> (Vec<Vec2>, Termination) {
    let mut pts = Vec::with_capacity(256);
    pts.push(seed);

    if field.body_at(seed).is_some() {
        return (pts, Termination::HitBody);
    }

    let h = cfg.step * sign;
    let dir = |p: Vec2| -> Option<Vec2> {
        let v = field.velocity_with(p, opts);
        if !v.is_finite() {
            return None;
        }
        let n = v.norm();
        if n < cfg.min_speed {
            None
        } else {
            Some(v / n)
        }
    };

    let mut p = seed;
    for step in 0..cfg.max_steps {
        let Some(k1) = dir(p) else {
            return (pts, stagnation_or_nonfinite(field, p, opts, cfg));
        };
        let Some(k2) = dir(p + k1 * (h * 0.5)) else {
            return (pts, Termination::Stagnation);
        };
        let Some(k3) = dir(p + k2 * (h * 0.5)) else {
            return (pts, Termination::Stagnation);
        };
        let Some(k4) = dir(p + k3 * h) else {
            return (pts, Termination::Stagnation);
        };
        let next = p + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (h / 6.0);

        if !next.is_finite() {
            return (pts, Termination::NonFinite);
        }
        if !cfg.bounds.contains(next) {
            // Clip to the boundary so the line ends exactly on the frame.
            if let Some(hit) = clip_to_bounds(p, next, &cfg.bounds) {
                pts.push(hit);
            }
            return (pts, Termination::LeftDomain);
        }
        if let Some(bi) = field.body_at(next) {
            // Bisect onto the wall so streamlines terminate flush with the
            // surface rather than a step short of it.
            pts.push(bisect_to_surface(field, p, next, bi));
            return (pts, Termination::HitBody);
        }

        pts.push(next);
        p = next;

        // Closed-orbit detection, skipping the first few steps so we do not
        // immediately match the seed.
        if step > 8 && p.distance(seed) < cfg.loop_tolerance {
            return (pts, Termination::Closed);
        }
    }
    (pts, Termination::MaxSteps)
}

fn stagnation_or_nonfinite(
    field: &FlowField,
    p: Vec2,
    opts: &FieldEvalOptions,
    cfg: &StreamlineConfig,
) -> Termination {
    let v = field.velocity_with(p, opts);
    if !v.is_finite() {
        Termination::NonFinite
    } else if v.norm() < cfg.min_speed {
        Termination::Stagnation
    } else {
        Termination::NonFinite
    }
}

/// Point where segment `a→b` crosses the box boundary.
fn clip_to_bounds(a: Vec2, b: Vec2, bounds: &Bounds) -> Option<Vec2> {
    let d = b - a;
    let mut t_exit = 1.0f64;
    for (p, dd, lo, hi) in [
        (a.x, d.x, bounds.min.x, bounds.max.x),
        (a.y, d.y, bounds.min.y, bounds.max.y),
    ] {
        if dd.abs() < 1e-300 {
            continue;
        }
        let t1 = (lo - p) / dd;
        let t2 = (hi - p) / dd;
        let (near, far) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
        let _ = near;
        if far >= 0.0 {
            t_exit = t_exit.min(far);
        }
    }
    if (0.0..=1.0).contains(&t_exit) {
        Some(a + d * t_exit)
    } else {
        None
    }
}

/// Bisect between an exterior point and an interior point to land on the wall.
fn bisect_to_surface(field: &FlowField, outside: Vec2, inside: Vec2, body: usize) -> Vec2 {
    let mut lo = outside;
    let mut hi = inside;
    for _ in 0..24 {
        let mid = lo.lerp(hi, 0.5);
        if field.bodies[body].contains_point(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// Seed strategy for automatic streamline placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum SeedStrategy {
    /// A rake on the upstream boundary, perpendicular to the freestream.
    /// Cheap and predictable; leaves gaps where the flow diverges.
    Inflow,
    /// Regular grid of seeds.
    Grid,
    /// Jobard–Lefer evenly spaced placement: streamlines are grown until they
    /// approach an existing line, and new seeds are offered perpendicular to
    /// existing lines. Produces uniform coverage with no bunching.
    EvenlySpaced,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SeedingConfig {
    pub strategy: SeedStrategy,
    /// Requested number of streamlines (`Inflow`, `Grid`).
    pub count: usize,
    /// Target separation for `EvenlySpaced`, as a fraction of the domain
    /// diagonal.
    pub separation: f64,
    /// Fraction of `separation` at which a growing line stops because it has
    /// approached a neighbour. Jobard & Lefer recommend ≈ 0.5.
    pub stop_ratio: f64,
    pub max_lines: usize,
}

impl Default for SeedingConfig {
    fn default() -> Self {
        Self {
            strategy: SeedStrategy::EvenlySpaced,
            count: 32,
            separation: 0.035,
            stop_ratio: 0.55,
            max_lines: 400,
        }
    }
}

/// Generate a set of streamlines over the domain.
pub fn trace_set(
    field: &FlowField,
    cfg: &StreamlineConfig,
    seeding: &SeedingConfig,
) -> Vec<Streamline> {
    match seeding.strategy {
        SeedStrategy::Inflow => seed_inflow(field, cfg, seeding)
            .into_iter()
            .map(|s| trace(field, s, cfg))
            .filter(|s| s.points.len() > 2)
            .collect(),
        SeedStrategy::Grid => seed_grid(cfg, seeding)
            .into_iter()
            .filter(|s| field.body_at(*s).is_none())
            .map(|s| trace(field, s, cfg))
            .filter(|s| s.points.len() > 2)
            .collect(),
        SeedStrategy::EvenlySpaced => evenly_spaced(field, cfg, seeding),
    }
}

fn seed_inflow(field: &FlowField, cfg: &StreamlineConfig, seeding: &SeedingConfig) -> Vec<Vec2> {
    let b = cfg.bounds;
    let centre = b.center();
    let fs = field.conditions.freestream();
    let dir = if fs.norm() > 0.0 {
        fs.normalized()
    } else {
        Vec2::X
    };
    let perp = dir.perp_left();

    // Place the rake just inside the *upstream face* of the box, found by
    // projecting the four corners onto the flow direction. Measuring a fixed
    // fraction of the diagonal from the centre would leave the box entirely
    // for most aspect ratios and flow angles.
    let corners = [
        b.min,
        Vec2::new(b.max.x, b.min.y),
        b.max,
        Vec2::new(b.min.x, b.max.y),
    ];
    let proj = |axis: Vec2| {
        corners
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), c| {
                let v = (*c - centre).dot(axis);
                (lo.min(v), hi.max(v))
            })
    };
    let (s_min, s_max) = proj(dir);
    let (t_min, t_max) = proj(perp);
    let inset = 0.01 * (s_max - s_min);
    let origin = centre + dir * (s_min + inset);
    // Inset laterally too, so the outermost seeds are not clipped by rounding.
    let t_lo = t_min + 0.02 * (t_max - t_min);
    let t_hi = t_max - 0.02 * (t_max - t_min);

    (0..seeding.count)
        .map(|i| {
            let f = (i as f64 + 0.5) / seeding.count as f64;
            origin + perp * (t_lo + f * (t_hi - t_lo))
        })
        .filter(|p| b.contains(*p))
        .collect()
}

fn seed_grid(cfg: &StreamlineConfig, seeding: &SeedingConfig) -> Vec<Vec2> {
    let n = (seeding.count as f64).sqrt().ceil().max(2.0) as usize;
    let b = cfg.bounds;
    let mut out = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let fx = (i as f64 + 0.5) / n as f64;
            let fy = (j as f64 + 0.5) / n as f64;
            out.push(Vec2::new(
                b.min.x + fx * b.width(),
                b.min.y + fy * b.height(),
            ));
        }
    }
    out
}

/// Uniform-grid spatial hash over streamline sample points.
struct PointHash {
    cell: f64,
    origin: Vec2,
    nx: usize,
    ny: usize,
    buckets: Vec<Vec<Vec2>>,
}

impl PointHash {
    fn new(bounds: Bounds, cell: f64) -> Self {
        let cell = cell.max(1e-9);
        let nx = (bounds.width() / cell).ceil().max(1.0) as usize + 2;
        let ny = (bounds.height() / cell).ceil().max(1.0) as usize + 2;
        Self {
            cell,
            origin: bounds.min,
            nx,
            ny,
            buckets: vec![Vec::new(); nx * ny],
        }
    }

    #[inline]
    fn index(&self, p: Vec2) -> (isize, isize) {
        (
            ((p.x - self.origin.x) / self.cell).floor() as isize,
            ((p.y - self.origin.y) / self.cell).floor() as isize,
        )
    }

    fn insert(&mut self, p: Vec2) {
        let (i, j) = self.index(p);
        if i < 0 || j < 0 || i as usize >= self.nx || j as usize >= self.ny {
            return;
        }
        self.buckets[j as usize * self.nx + i as usize].push(p);
    }

    /// Is any stored point within `radius` of `p`? Only the 3×3 neighbourhood
    /// needs checking because `cell >= radius`.
    fn has_neighbour_within(&self, p: Vec2, radius: f64) -> bool {
        let (ci, cj) = self.index(p);
        let r2 = radius * radius;
        for dj in -1..=1 {
            for di in -1..=1 {
                let i = ci + di;
                let j = cj + dj;
                if i < 0 || j < 0 || i as usize >= self.nx || j as usize >= self.ny {
                    continue;
                }
                for q in &self.buckets[j as usize * self.nx + i as usize] {
                    if (*q - p).norm_sq() < r2 {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Jobard–Lefer evenly spaced streamline placement.
fn evenly_spaced(
    field: &FlowField,
    cfg: &StreamlineConfig,
    seeding: &SeedingConfig,
) -> Vec<Streamline> {
    let diag = cfg.bounds.diagonal();
    let d_sep = (seeding.separation * diag).max(cfg.step * 2.0);
    let d_test = d_sep * seeding.stop_ratio.clamp(0.1, 0.95);
    let mut hash = PointHash::new(cfg.bounds.expanded(d_sep), d_sep);
    let mut lines: Vec<Streamline> = Vec::new();

    // First seed: slightly off-centre so a symmetric scene does not start on a
    // stagnation streamline, where integration would stall immediately.
    let c = cfg.bounds.center();
    let mut queue: Vec<Vec2> = vec![c + Vec2::new(0.0, 0.137 * d_sep + 0.013 * diag)];
    // Fallback seeds: a rake on the upstream face. Without them a body covering
    // the centre (the cylinder example) rejects the only seed and the whole
    // picture comes out empty. Candidates already near a line are skipped by
    // the separation test, so these cost nothing once coverage exists.
    let rake = SeedingConfig {
        count: ((cfg.bounds.diagonal() / d_sep).ceil() as usize).clamp(4, 200),
        ..*seeding
    };
    queue.extend(seed_inflow(field, cfg, &rake));

    let mut head = 0usize;
    while head < queue.len() && lines.len() < seeding.max_lines {
        let seed = queue[head];
        head += 1;

        if !cfg.bounds.contains(seed)
            || field.body_at(seed).is_some()
            || hash.has_neighbour_within(seed, d_sep)
        {
            continue;
        }

        let line = trace_limited(field, seed, cfg, &hash, d_test);
        if line.points.len() < 3 {
            continue;
        }
        for p in &line.points {
            hash.insert(*p);
        }

        // Offer candidate seeds at ±d_sep perpendicular to the new line,
        // spaced about d_sep apart along it.
        let mut travelled = 0.0;
        for w in line.points.windows(2) {
            travelled += w[0].distance(w[1]);
            if travelled < d_sep {
                continue;
            }
            travelled = 0.0;
            let t = (w[1] - w[0]).normalized();
            if t == Vec2::ZERO {
                continue;
            }
            let n = t.perp_left();
            queue.push(w[0] + n * d_sep);
            queue.push(w[0] - n * d_sep);
        }

        lines.push(line);
    }
    lines
}

/// Like [`trace`] but also stops when the line approaches an existing one.
fn trace_limited(
    field: &FlowField,
    seed: Vec2,
    cfg: &StreamlineConfig,
    hash: &PointHash,
    d_test: f64,
) -> Streamline {
    let opts = cfg.eval;
    let run = |sign: f64| -> (Vec<Vec2>, Termination) {
        let mut pts = vec![seed];
        let mut p = seed;
        let h = cfg.step * sign;
        for step in 0..cfg.max_steps {
            let v = field.velocity_with(p, &opts);
            if !v.is_finite() {
                return (pts, Termination::NonFinite);
            }
            let n = v.norm();
            if n < cfg.min_speed {
                return (pts, Termination::Stagnation);
            }
            // RK4 on the normalised field.
            let d1 = v / n;
            let Some(d2) = unit_dir(field, p + d1 * (h * 0.5), &opts, cfg.min_speed) else {
                return (pts, Termination::Stagnation);
            };
            let Some(d3) = unit_dir(field, p + d2 * (h * 0.5), &opts, cfg.min_speed) else {
                return (pts, Termination::Stagnation);
            };
            let Some(d4) = unit_dir(field, p + d3 * h, &opts, cfg.min_speed) else {
                return (pts, Termination::Stagnation);
            };
            let next = p + (d1 + d2 * 2.0 + d3 * 2.0 + d4) * (h / 6.0);

            if !next.is_finite() {
                return (pts, Termination::NonFinite);
            }
            if !cfg.bounds.contains(next) {
                if let Some(hit) = clip_to_bounds(p, next, &cfg.bounds) {
                    pts.push(hit);
                }
                return (pts, Termination::LeftDomain);
            }
            if let Some(bi) = field.body_at(next) {
                pts.push(bisect_to_surface(field, p, next, bi));
                return (pts, Termination::HitBody);
            }
            if step > 2 && hash.has_neighbour_within(next, d_test) {
                return (pts, Termination::Closed);
            }
            pts.push(next);
            p = next;
            if step > 8 && p.distance(seed) < cfg.loop_tolerance {
                return (pts, Termination::Closed);
            }
        }
        (pts, Termination::MaxSteps)
    };

    let (fwd, t_fwd) = run(1.0);
    if t_fwd == Termination::Closed && fwd.len() > 16 {
        // Closed orbit: the forward pass is the whole curve (see `trace`).
        let speeds = fwd
            .iter()
            .map(|p| field.velocity_with(*p, &cfg.eval).norm() as f32)
            .collect();
        let arc_length = polyline_length(&fwd);
        return Streamline {
            points: fwd,
            speeds,
            arc_length,
            termination_forward: t_fwd,
            termination_backward: None,
            seed_index: 0,
        };
    }
    let (bwd, t_bwd) = run(-1.0);
    let mut points: Vec<Vec2> = bwd.into_iter().skip(1).rev().collect();
    let seed_index = points.len();
    points.extend(fwd);
    let speeds = points
        .iter()
        .map(|p| field.velocity_with(*p, &cfg.eval).norm() as f32)
        .collect();
    let arc_length = polyline_length(&points);
    Streamline {
        points,
        speeds,
        arc_length,
        termination_forward: t_fwd,
        termination_backward: Some(t_bwd),
        seed_index,
    }
}

#[inline]
fn unit_dir(field: &FlowField, p: Vec2, opts: &FieldEvalOptions, min_speed: f64) -> Option<Vec2> {
    let v = field.velocity_with(p, opts);
    if !v.is_finite() {
        return None;
    }
    let n = v.norm();
    if n < min_speed {
        None
    } else {
        Some(v / n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conditions::FlowConditions;
    use crate::elements::Element;
    use crate::field::PanelBody;
    use aeroflow_geometry::{shapes, Panelisation};
    use std::f64::consts::PI;

    fn box_bounds(r: f64) -> Bounds {
        Bounds {
            min: Vec2::new(-r, -r),
            max: Vec2::new(r, r),
        }
    }

    #[test]
    fn uniform_flow_streamlines_are_straight_horizontal_lines() {
        let f = FlowField::new(FlowConditions {
            velocity: 5.0,
            angle: 0.0,
            ..Default::default()
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(5.0));
        let s = trace(&f, Vec2::new(0.0, 1.3), &cfg);
        assert!(s.points.len() > 100);
        for p in &s.points {
            assert!((p.y - 1.3).abs() < 1e-8, "y drifted to {}", p.y);
        }
        assert_eq!(s.termination_forward, Termination::LeftDomain);
        // The line must span the full domain width.
        let x0 = s.points.first().unwrap().x;
        let x1 = s.points.last().unwrap().x;
        assert!((x1 - x0) > 9.9, "span = {}", x1 - x0);
    }

    #[test]
    fn angled_uniform_flow_streamlines_follow_the_angle() {
        let angle = 0.4;
        let f = FlowField::new(FlowConditions {
            velocity: 2.0,
            angle,
            ..Default::default()
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(4.0));
        let s = trace(&f, Vec2::ZERO, &cfg);
        let d = *s.points.last().unwrap() - *s.points.first().unwrap();
        assert!(
            (d.angle() - angle).abs() < 1e-6,
            "traced angle {}",
            d.angle()
        );
    }

    #[test]
    fn vortex_streamlines_close_on_themselves() {
        let mut f = FlowField::new(FlowConditions {
            velocity: 0.0,
            ..Default::default()
        });
        f.elements.push(Element::Vortex {
            position: Vec2::ZERO,
            circulation: 4.0,
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(4.0));
        let s = trace(&f, Vec2::new(1.5, 0.0), &cfg);
        assert_eq!(s.termination_forward, Termination::Closed);
        // Every point must stay on the r = 1.5 circle.
        for p in &s.points {
            assert!(
                (p.norm() - 1.5).abs() < 1e-4,
                "radius drifted to {}",
                p.norm()
            );
        }
        // Closed orbit length ≈ 2πr.
        let expected = 2.0 * PI * 1.5;
        assert!(
            (s.arc_length - expected).abs() / expected < 0.05,
            "arc {} vs {expected}",
            s.arc_length
        );
    }

    #[test]
    fn streamlines_stop_at_a_body_surface() {
        let poly = shapes::circle(1.0, 128);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let mut f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        f.bodies
            .push(PanelBody::new(poly, pz.panels, vec![0.0; k], 0.0));
        // Head straight at the cylinder along the centreline.
        let cfg = StreamlineConfig::for_bounds(box_bounds(4.0));
        let s = trace(&f, Vec2::new(-3.0, 0.0), &cfg);
        assert_eq!(s.termination_forward, Termination::HitBody);
        let last = *s.points.last().unwrap();
        // The terminal point must sit on the wall, not short of it or inside.
        assert!(
            (last.norm() - 1.0).abs() < 1e-3,
            "ended at r = {}",
            last.norm()
        );
        assert!(f.body_at(last).is_none(), "ended inside the body");
    }

    #[test]
    fn seed_inside_a_body_produces_no_line() {
        let poly = shapes::circle(1.0, 64);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let mut f = FlowField::new(FlowConditions::default());
        f.bodies
            .push(PanelBody::new(poly, pz.panels, vec![0.0; k], 0.0));
        let cfg = StreamlineConfig::for_bounds(box_bounds(4.0));
        let s = trace(&f, Vec2::ZERO, &cfg);
        assert_eq!(s.termination_forward, Termination::HitBody);
        assert_eq!(s.points.len(), 1);
    }

    #[test]
    fn zero_velocity_field_terminates_at_once_instead_of_spinning() {
        let f = FlowField::new(FlowConditions {
            velocity: 0.0,
            ..Default::default()
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(2.0));
        let s = trace(&f, Vec2::new(0.5, 0.5), &cfg);
        assert_eq!(s.termination_forward, Termination::Stagnation);
        assert!(s.points.len() < 5);
    }

    #[test]
    fn evenly_spaced_placement_respects_the_separation() {
        let f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        let bounds = box_bounds(2.0);
        let cfg = StreamlineConfig::for_bounds(bounds);
        let seeding = SeedingConfig {
            strategy: SeedStrategy::EvenlySpaced,
            separation: 0.08,
            ..Default::default()
        };
        let lines = trace_set(&f, &cfg, &seeding);
        assert!(lines.len() > 4, "only {} lines", lines.len());

        // No two *distinct* lines may come closer than the stop distance.
        let d_sep = 0.08 * bounds.diagonal();
        let d_min = d_sep * seeding.stop_ratio * 0.9;
        for (a, la) in lines.iter().enumerate() {
            for lb in lines.iter().skip(a + 1) {
                // Subsample for speed; the hash already enforced this during
                // construction, so this is a guard against regressions.
                for p in la.points.iter().step_by(13) {
                    for q in lb.points.iter().step_by(13) {
                        assert!(
                            p.distance(*q) > d_min,
                            "lines too close: {:.4} < {:.4}",
                            p.distance(*q),
                            d_min
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn evenly_spaced_placement_works_when_a_body_covers_the_view_centre() {
        // Regression: the single initial seed sat at the domain centre; with a
        // cylinder there it was rejected and no streamlines were produced.
        let poly = shapes::circle(1.0, 120);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let mut f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        f.bodies
            .push(PanelBody::new(poly, pz.panels, vec![0.0; k], 0.0));
        let cfg = StreamlineConfig::for_bounds(box_bounds(3.0));
        let lines = trace_set(&f, &cfg, &SeedingConfig::default());
        assert!(
            lines.len() >= 8,
            "only {} streamlines around a centred body",
            lines.len()
        );
    }

    #[test]
    fn evenly_spaced_placement_covers_the_domain() {
        let f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        let bounds = box_bounds(2.0);
        let cfg = StreamlineConfig::for_bounds(bounds);
        let lines = trace_set(
            &f,
            &cfg,
            &SeedingConfig {
                separation: 0.06,
                ..Default::default()
            },
        );
        // Coverage check: every cell of a coarse grid should be near some line.
        let mut covered = 0;
        let mut total = 0;
        for j in 0..10 {
            for i in 0..10 {
                let p = Vec2::new(-1.8 + 0.4 * i as f64, -1.8 + 0.4 * j as f64);
                total += 1;
                let near = lines.iter().any(|l| {
                    l.points
                        .iter()
                        .any(|q| q.distance(p) < 0.25 * bounds.diagonal() * 0.06 * 4.0)
                });
                if near {
                    covered += 1;
                }
            }
        }
        assert!(covered * 10 >= total * 8, "covered {covered}/{total}");
    }

    #[test]
    fn inflow_seeding_produces_lines_across_the_domain() {
        let f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(3.0));
        let lines = trace_set(
            &f,
            &cfg,
            &SeedingConfig {
                strategy: SeedStrategy::Inflow,
                count: 12,
                ..Default::default()
            },
        );
        assert!(lines.len() >= 8, "{} lines", lines.len());
        for l in &lines {
            assert!(l.arc_length > 4.0);
        }
    }

    #[test]
    fn grid_seeding_skips_seeds_inside_bodies() {
        let poly = shapes::circle(1.5, 64);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let mut f = FlowField::new(FlowConditions {
            velocity: 1.0,
            ..Default::default()
        });
        f.bodies
            .push(PanelBody::new(poly, pz.panels, vec![0.0; k], 0.0));
        let cfg = StreamlineConfig::for_bounds(box_bounds(3.0));
        let lines = trace_set(
            &f,
            &cfg,
            &SeedingConfig {
                strategy: SeedStrategy::Grid,
                count: 64,
                ..Default::default()
            },
        );
        for l in &lines {
            assert!(f.body_at(l.points[l.seed_index]).is_none());
        }
    }

    #[test]
    fn speeds_are_recorded_alongside_points() {
        let f = FlowField::new(FlowConditions {
            velocity: 7.0,
            angle: 0.0,
            ..Default::default()
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(2.0));
        let s = trace(&f, Vec2::new(0.0, 0.5), &cfg);
        assert_eq!(s.points.len(), s.speeds.len());
        for v in &s.speeds {
            assert!((*v - 7.0).abs() < 1e-4);
        }
    }

    #[test]
    fn far_field_tracing_stays_close_to_the_exact_streamline() {
        // A source-panel ring with non-trivial strengths, a stream at an angle.
        let poly = shapes::circle(1.0, 400);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let sigma: Vec<f64> = pz.panels.iter().map(|p| -2.0 * p.mid.x).collect();
        let mut f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.2,
            ..Default::default()
        });
        f.bodies.push(PanelBody::new(poly, pz.panels, sigma, 0.05));
        let bounds = box_bounds(5.0);
        let exact = trace(
            &f,
            Vec2::new(-4.0, 1.6),
            &StreamlineConfig::for_bounds(bounds),
        );
        let approx = trace(
            &f,
            Vec2::new(-4.0, 1.6),
            &StreamlineConfig::for_rendering(bounds),
        );
        // Hausdorff-style check: every approximate point lies near the exact line.
        let worst = approx
            .points
            .iter()
            .map(|p| {
                exact
                    .points
                    .iter()
                    .map(|q| p.distance(*q))
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(0.0, f64::max);
        assert!(worst < 0.02, "max deviation {worst} over a 10 m domain");
        assert!(exact.points.len() > 100);
    }

    #[test]
    fn two_sided_trace_orders_points_upstream_to_downstream() {
        let f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        let cfg = StreamlineConfig::for_bounds(box_bounds(2.0));
        let s = trace(&f, Vec2::ZERO, &cfg);
        assert!(s.seed_index > 0 && s.seed_index < s.points.len());
        assert!(s.points.first().unwrap().x < s.points.last().unwrap().x);
        assert!(s.points[s.seed_index].distance(Vec2::ZERO) < 1e-12);
    }
}
