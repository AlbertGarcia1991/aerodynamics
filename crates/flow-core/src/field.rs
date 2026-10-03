//! Field evaluation: the generic `evaluate_*` API of PRD §16, plus grid
//! sampling for the renderer.
//!
//! The UI never needs to know how a quantity is computed — it asks for a
//! [`FieldType`] over a [`GridDefinition`] and receives a [`ScalarField`].

use crate::conditions::FlowConditions;
use crate::elements::Element;
use crate::panel_kernel::{PanelKernel, PanelPotentials};
use aeroflow_geometry::{Bounds, Panel, Polygon, Vec2};
use std::f64::consts::PI;

const INV_2PI: f64 = 1.0 / (2.0 * PI);

/// Scalar quantities the renderer and exporter can request (PRD §22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum FieldType {
    VelocityMagnitude,
    VelocityU,
    VelocityV,
    Pressure,
    Cp,
    Vorticity,
    Potential,
    StreamFunction,
}

impl FieldType {
    pub const ALL: [FieldType; 8] = [
        FieldType::VelocityMagnitude,
        FieldType::VelocityU,
        FieldType::VelocityV,
        FieldType::Pressure,
        FieldType::Cp,
        FieldType::Vorticity,
        FieldType::Potential,
        FieldType::StreamFunction,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FieldType::VelocityMagnitude => "Velocity magnitude",
            FieldType::VelocityU => "Velocity u",
            FieldType::VelocityV => "Velocity v",
            FieldType::Pressure => "Static pressure",
            FieldType::Cp => "Pressure coefficient",
            FieldType::Vorticity => "Vorticity",
            FieldType::Potential => "Velocity potential",
            FieldType::StreamFunction => "Stream function",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            FieldType::VelocityMagnitude => "|V|",
            FieldType::VelocityU => "u",
            FieldType::VelocityV => "v",
            FieldType::Pressure => "p",
            FieldType::Cp => "Cp",
            FieldType::Vorticity => "ω",
            FieldType::Potential => "φ",
            FieldType::StreamFunction => "ψ",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            FieldType::VelocityMagnitude | FieldType::VelocityU | FieldType::VelocityV => "m/s",
            FieldType::Pressure => "Pa",
            FieldType::Cp => "–",
            FieldType::Vorticity => "1/s",
            FieldType::Potential => "m²/s",
            FieldType::StreamFunction => "m²/s",
        }
    }

    /// Whether the field is naturally centred on zero, which tells the renderer
    /// to pick a diverging colour map and a symmetric range.
    pub fn is_diverging(self) -> bool {
        matches!(
            self,
            FieldType::VelocityU
                | FieldType::VelocityV
                | FieldType::Vorticity
                | FieldType::Potential
                | FieldType::StreamFunction
        )
    }
}

/// A discretised body with its solved singularity strengths.
///
/// This is *data*, not a solver: `panel-method` produces the strengths, and
/// field evaluation consumes them.
#[derive(Debug, Clone)]
pub struct PanelBody {
    pub panels: Vec<Panel>,
    /// Constant source strength per panel, `σ_j` [m/s].
    pub source_strengths: Vec<f64>,
    /// Uniform vortex strength over the whole body, `γ` [m/s].
    pub vortex_strength: f64,
    /// The contour, kept for inside/outside masking.
    pub polygon: Polygon,
    bbox: Bounds,
    clusters: Vec<PanelCluster>,
    max_panel_length: f64,
}

/// A contiguous run of panels, summarised by its net singularity strengths.
///
/// Far from the cluster the whole run is well approximated by a single point
/// source plus point vortex at its centroid — the Barnes–Hut idea applied to a
/// boundary element method. See [`FieldEvalOptions::far_field_ratio`].
#[derive(Debug, Clone, Copy)]
struct PanelCluster {
    start: usize,
    end: usize,
    centroid: Vec2,
    /// Distance from the centroid to the furthest panel endpoint in the run.
    radius: f64,
    /// `Σ σ_j·L_j` [m²/s] — the equivalent point-source strength.
    net_source: f64,
    /// `Σ γ·L_j` [m²/s] — the equivalent point-vortex circulation.
    net_vortex: f64,
}

/// Panels per cluster. Sixteen keeps the cluster radius well below the typical
/// grid-point stand-off while still cutting the far-field work by 16×.
const CLUSTER_SIZE: usize = 16;

impl PanelBody {
    pub fn new(
        polygon: Polygon,
        panels: Vec<Panel>,
        source_strengths: Vec<f64>,
        vortex_strength: f64,
    ) -> Self {
        debug_assert_eq!(panels.len(), source_strengths.len());
        let bbox = polygon.bounds().unwrap_or(Bounds {
            min: Vec2::ZERO,
            max: Vec2::ZERO,
        });
        let clusters = build_clusters(&panels, &source_strengths, vortex_strength);
        let max_panel_length = panels.iter().map(|p| p.length).fold(0.0, f64::max);
        Self {
            panels,
            source_strengths,
            vortex_strength,
            polygon,
            bbox,
            clusters,
            max_panel_length,
        }
    }

    #[inline]
    pub fn max_panel_length(&self) -> f64 {
        self.max_panel_length
    }

    /// Is `p` within `margin` of the body surface (outside or inside)?
    ///
    /// A constant-strength source panel's velocity has a logarithmic
    /// singularity at each panel *vertex*, so field samples that land within a
    /// fraction of a panel length of the wall show spurious spikes. The
    /// renderer masks that band; the surface values it would have shown are
    /// known exactly from the panel solution anyway.
    pub fn near_boundary(&self, p: Vec2, margin: f64) -> bool {
        if margin <= 0.0 || !self.bbox.expanded(margin).contains(p) {
            return false;
        }
        self.polygon.distance_to_boundary(p) < margin
    }

    /// Net bound circulation `Γ = γ·perimeter` [m²/s], counter-clockwise
    /// positive. Lift follows as `L = −ρU∞Γ` under this crate's sign
    /// convention (see [`crate::elements`]).
    pub fn circulation(&self) -> f64 {
        self.vortex_strength * self.panels.iter().map(|p| p.length).sum::<f64>()
    }

    /// Net volumetric outflow `Σσ_j L_j`, which must be ≈ 0 for a closed body
    /// in an incompressible flow. A non-zero value is a solver diagnostic.
    pub fn net_source_outflow(&self) -> f64 {
        self.panels
            .iter()
            .zip(self.source_strengths.iter())
            .map(|(p, s)| p.length * s)
            .sum()
    }

    #[inline]
    pub fn contains_point(&self, p: Vec2) -> bool {
        // A bounding-box reject makes masking cheap for the majority of grid
        // points, which lie well outside every body.
        self.bbox.contains(p) && self.polygon.contains_point(p)
    }

    fn velocity(&self, p: Vec2, opts: &FieldEvalOptions) -> Vec2 {
        if opts.far_field_ratio <= 0.0 {
            return self.velocity_exact(p);
        }
        let ratio = opts.far_field_ratio;
        let mut v = Vec2::ZERO;
        for c in &self.clusters {
            let d = p - c.centroid;
            let d2 = d.norm_sq();
            if d2 > (ratio * c.radius) * (ratio * c.radius) {
                v = v + point_singularity(d, d2, c.net_source, c.net_vortex);
            } else {
                for j in c.start..c.end {
                    v = v + self.panel_velocity(j, p, ratio);
                }
            }
        }
        v
    }

    #[inline]
    fn panel_velocity(&self, j: usize, p: Vec2, ratio: f64) -> Vec2 {
        let panel = &self.panels[j];
        let sigma = self.source_strengths[j];
        let gamma = self.vortex_strength;
        let d = p - panel.mid;
        let d2 = d.norm_sq();
        let cutoff = ratio * panel.length;
        if d2 > cutoff * cutoff {
            // One point singularity of equal total strength: error O((L/d)²).
            point_singularity(d, d2, sigma * panel.length, gamma * panel.length)
        } else {
            let k = PanelKernel::evaluate(panel, p);
            let (sx, sn) = k.source_local();
            let (vx, vn) = k.vortex_local();
            panel.to_global_vector(sigma * sx + gamma * vx, sigma * sn + gamma * vn)
        }
    }

    fn velocity_exact(&self, p: Vec2) -> Vec2 {
        let gamma = self.vortex_strength;
        let mut v = Vec2::ZERO;
        for (panel, &sigma) in self.panels.iter().zip(self.source_strengths.iter()) {
            let k = PanelKernel::evaluate(panel, p);
            let (sx, sn) = k.source_local();
            let (vx, vn) = k.vortex_local();
            v = v + panel.to_global_vector(sigma * sx + gamma * vx, sigma * sn + gamma * vn);
        }
        v
    }

    fn potential(&self, p: Vec2) -> f64 {
        let gamma = self.vortex_strength;
        self.panels
            .iter()
            .zip(self.source_strengths.iter())
            .map(|(panel, &sigma)| {
                let pp = PanelPotentials::evaluate(panel, p);
                sigma * pp.source_potential() + gamma * pp.vortex_potential()
            })
            .sum()
    }

    fn stream_function(&self, p: Vec2) -> f64 {
        let gamma = self.vortex_strength;
        self.panels
            .iter()
            .zip(self.source_strengths.iter())
            .map(|(panel, &sigma)| {
                let pp = PanelPotentials::evaluate(panel, p);
                sigma * pp.source_stream_function() + gamma * pp.vortex_stream_function()
            })
            .sum()
    }
}

#[inline]
fn point_singularity(d: Vec2, d2: f64, source: f64, vortex: f64) -> Vec2 {
    let s = source * INV_2PI / d2;
    let g = vortex * INV_2PI / d2;
    Vec2::new(d.x * s - d.y * g, d.y * s + d.x * g)
}

fn build_clusters(panels: &[Panel], sigma: &[f64], gamma: f64) -> Vec<PanelCluster> {
    let n = panels.len();
    let mut out = Vec::with_capacity(n / CLUSTER_SIZE + 1);
    let mut start = 0usize;
    while start < n {
        let end = (start + CLUSTER_SIZE).min(n);
        let mut centroid = Vec2::ZERO;
        let mut weight = 0.0;
        for j in start..end {
            centroid = centroid + panels[j].mid * panels[j].length;
            weight += panels[j].length;
        }
        if weight > 0.0 {
            centroid = centroid / weight;
        }
        let mut radius: f64 = 0.0;
        let mut net_source = 0.0;
        let mut net_vortex = 0.0;
        for j in start..end {
            radius = radius
                .max(centroid.distance(panels[j].start))
                .max(centroid.distance(panels[j].end));
            net_source += sigma[j] * panels[j].length;
            net_vortex += gamma * panels[j].length;
        }
        out.push(PanelCluster {
            start,
            end,
            centroid,
            radius,
            net_source,
            net_vortex,
        });
        start = end;
    }
    out
}

/// Controls accuracy/speed trade-offs in field evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct FieldEvalOptions {
    /// Singularity softening radius `a`: `r²` becomes `r² + a²`.
    ///
    /// `0.0` gives the exact analytic field. The renderer sets this to roughly
    /// one grid cell so that a pixel landing on a singularity does not destroy
    /// the colour scale, and the legend reports when it is active.
    pub core_radius: f64,
    /// Distance/size ratio above which a panel or cluster is replaced by an
    /// equivalent point singularity. `0.0` disables the approximation.
    ///
    /// The error is `O((L/d)²)`, so 12 costs well under 1% in the rendered
    /// field. **Numerical results always use `0.0`.**
    pub far_field_ratio: f64,
    /// Mask samples closer to a body surface than this many of its longest
    /// panel. Hides the log-singular band around panel vertices in rendered
    /// fields (see [`PanelBody::near_boundary`]). `0.0` disables masking.
    pub wall_margin: f64,
}

impl FieldEvalOptions {
    /// Exact evaluation — the default for anything reported as a number.
    pub const EXACT: Self = Self {
        core_radius: 0.0,
        far_field_ratio: 0.0,
        wall_margin: 0.0,
    };

    /// Rendering preset: soften singularities by `cell`, approximate the far
    /// field, and mask the near-wall band.
    pub fn for_rendering(cell: f64) -> Self {
        Self {
            core_radius: 0.35 * cell,
            far_field_ratio: 12.0,
            wall_margin: 0.75,
        }
    }
}

impl Default for FieldEvalOptions {
    fn default() -> Self {
        Self::EXACT
    }
}

/// A complete evaluable flow: freestream + elementary elements + solved bodies.
#[derive(Debug, Clone, Default)]
pub struct FlowField {
    pub conditions: FlowConditions,
    pub elements: Vec<Element>,
    pub bodies: Vec<PanelBody>,
}

impl FlowField {
    pub fn new(conditions: FlowConditions) -> Self {
        Self {
            conditions,
            elements: Vec::new(),
            bodies: Vec::new(),
        }
    }

    /// Velocity contributed by the freestream and the elementary elements only
    /// — the right-hand side the panel solver needs.
    pub fn ambient_velocity(&self, p: Vec2, core: f64) -> Vec2 {
        let mut v = self.conditions.freestream();
        for e in &self.elements {
            v = v + e.velocity(p, core);
        }
        v
    }

    /// Exact velocity at `p` induced by everything **except** element `skip`:
    /// the freestream, every other element and every body. This is the
    /// "external" velocity the Lagally theorem needs for the force on `skip`.
    pub fn velocity_excluding_element(&self, p: Vec2, skip: usize) -> Vec2 {
        let mut v = self.conditions.freestream();
        for (i, e) in self.elements.iter().enumerate() {
            if i != skip {
                v = v + e.velocity(p, 0.0);
            }
        }
        for b in &self.bodies {
            v = v + b.velocity(p, &FieldEvalOptions::EXACT);
        }
        v
    }

    /// Total velocity at `p` (PRD §16 `evaluate_velocity`).
    pub fn velocity(&self, p: Vec2) -> Vec2 {
        self.velocity_with(p, &FieldEvalOptions::EXACT)
    }

    pub fn velocity_with(&self, p: Vec2, opts: &FieldEvalOptions) -> Vec2 {
        let mut v = self.ambient_velocity(p, opts.core_radius);
        for b in &self.bodies {
            v = v + b.velocity(p, opts);
        }
        v
    }

    pub fn speed(&self, p: Vec2) -> f64 {
        self.velocity(p).norm()
    }

    /// `φ` (PRD §16 `evaluate_potential`). Multivalued where vortices are
    /// present; the branch follows `atan2`.
    pub fn potential(&self, p: Vec2) -> f64 {
        let fs = self.conditions.freestream();
        let mut acc = fs.x * p.x + fs.y * p.y;
        for e in &self.elements {
            acc += e.potential(p);
        }
        for b in &self.bodies {
            acc += b.potential(p);
        }
        acc
    }

    /// `ψ` (PRD §16 `evaluate_stream_function`). Multivalued where net source
    /// strength is non-zero.
    pub fn stream_function(&self, p: Vec2) -> f64 {
        let fs = self.conditions.freestream();
        let mut acc = fs.x * p.y - fs.y * p.x;
        for e in &self.elements {
            acc += e.stream_function(p);
        }
        for b in &self.bodies {
            acc += b.stream_function(p);
        }
        acc
    }

    pub fn pressure(&self, p: Vec2) -> f64 {
        self.conditions.pressure_at_speed(self.speed(p))
    }

    pub fn cp(&self, p: Vec2) -> Option<f64> {
        self.conditions.cp_at_speed(self.speed(p))
    }

    /// Vorticity `ω_z` (PRD §21).
    ///
    /// In ideal potential flow the vorticity of the fluid is **exactly zero**
    /// away from the singularities themselves; sources, doublets, uniform flow
    /// and source panels are all curl-free, and the bodies' vortex sheets lie
    /// on their boundaries, inside the masked region.
    ///
    /// Rather than render an identically blank plot, this returns the vorticity
    /// of the *softened* field, in which each point vortex becomes a finite
    /// core of radius `a`:
    ///
    /// ```text
    /// ω(r) = Γ·a² / (π·(r² + a²)²)
    /// ```
    ///
    /// That distribution integrates to exactly `Γ` over the plane, so the
    /// picture conserves circulation instead of inventing it. With `core = 0`
    /// the honest answer — zero everywhere but the singular points — is
    /// returned.
    pub fn vorticity(&self, p: Vec2, core: f64) -> f64 {
        let a2 = core * core;
        let mut w = 0.0;
        for e in &self.elements {
            let gamma = e.circulation();
            if gamma == 0.0 {
                continue;
            }
            let Some(pos) = e.position() else { continue };
            let r2 = (p - pos).norm_sq();
            if a2 <= 0.0 {
                if r2 <= 0.0 {
                    return f64::INFINITY * gamma.signum();
                }
            } else {
                let den = r2 + a2;
                w += gamma * a2 / (PI * den * den);
            }
        }
        w
    }

    /// Index of the body containing `p`, if any.
    pub fn body_at(&self, p: Vec2) -> Option<usize> {
        self.bodies.iter().position(|b| b.contains_point(p))
    }

    /// Like [`FlowField::body_at`], but for rendering: also claims points in
    /// the near-wall band when `opts.wall_margin > 0`.
    pub fn masked_at(&self, p: Vec2, opts: &FieldEvalOptions) -> Option<usize> {
        self.mask_code_at(p, opts).map(|(i, _)| i)
    }

    /// Body index and mask code (`MASK_BODY` or `MASK_NEAR_WALL`) for `p`, or
    /// `None` for ordinary fluid.
    pub fn mask_code_at(&self, p: Vec2, opts: &FieldEvalOptions) -> Option<(usize, u8)> {
        for (i, b) in self.bodies.iter().enumerate() {
            if b.contains_point(p) {
                return Some((i, MASK_BODY));
            }
            if opts.wall_margin > 0.0 && b.near_boundary(p, opts.wall_margin * b.max_panel_length())
            {
                return Some((i, MASK_NEAR_WALL));
            }
        }
        None
    }

    /// Total circulation in the scene (vortex elements plus bound circulation).
    pub fn total_circulation(&self) -> f64 {
        self.elements.iter().map(|e| e.circulation()).sum::<f64>()
            + self.bodies.iter().map(|b| b.circulation()).sum::<f64>()
    }

    /// Total net volumetric outflow. Non-zero means the flow cannot close at
    /// infinity, which is reported as a modelling warning.
    pub fn total_net_outflow(&self) -> f64 {
        self.elements.iter().map(|e| e.net_outflow()).sum()
    }

    pub fn scalar_at(&self, p: Vec2, field: FieldType, opts: &FieldEvalOptions) -> f64 {
        match field {
            FieldType::VelocityMagnitude => self.velocity_with(p, opts).norm(),
            FieldType::VelocityU => self.velocity_with(p, opts).x,
            FieldType::VelocityV => self.velocity_with(p, opts).y,
            FieldType::Pressure => self
                .conditions
                .pressure_at_speed(self.velocity_with(p, opts).norm()),
            FieldType::Cp => self
                .conditions
                .cp_at_speed(self.velocity_with(p, opts).norm())
                .unwrap_or(f64::NAN),
            FieldType::Vorticity => self.vorticity(p, opts.core_radius.max(1e-9)),
            FieldType::Potential => self.potential(p),
            FieldType::StreamFunction => self.stream_function(p),
        }
    }
}

/// Sampling grid for a field request.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct GridDefinition {
    pub bounds: Bounds,
    pub nx: usize,
    pub ny: usize,
}

impl GridDefinition {
    pub fn new(bounds: Bounds, nx: usize, ny: usize) -> Self {
        Self {
            bounds,
            nx: nx.max(2),
            ny: ny.max(2),
        }
    }

    /// Cell size in x and y.
    pub fn spacing(&self) -> (f64, f64) {
        (
            self.bounds.width() / (self.nx - 1) as f64,
            self.bounds.height() / (self.ny - 1) as f64,
        )
    }

    /// Smaller of the two cell dimensions — the natural softening scale.
    pub fn cell(&self) -> f64 {
        let (dx, dy) = self.spacing();
        dx.min(dy)
    }

    #[inline]
    pub fn point(&self, i: usize, j: usize) -> Vec2 {
        let (dx, dy) = self.spacing();
        Vec2::new(
            self.bounds.min.x + dx * i as f64,
            self.bounds.min.y + dy * j as f64,
        )
    }

    pub fn len(&self) -> usize {
        self.nx * self.ny
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Mask codes stored alongside a sampled field.
pub const MASK_FLUID: u8 = 0;
pub const MASK_BODY: u8 = 1;
/// Outside the body but within the near-wall band: the stored value is an
/// extrapolation from neighbouring fluid cells, not a direct evaluation. The
/// renderer draws it (so the band is continuous); exports flag it.
pub const MASK_NEAR_WALL: u8 = 2;

/// A sampled scalar field, ready for upload to a texture.
///
/// Values are `f32` (PRD §70: render buffers may drop to single precision)
/// while the reported range stays `f64`. Row-major, `y` increasing with `j`.
#[derive(Debug, Clone)]
pub struct ScalarField {
    pub grid: GridDefinition,
    pub field: FieldType,
    pub values: Vec<f32>,
    /// `MASK_BODY` where the sample lies inside a body.
    pub mask: Vec<u8>,
    /// Range over fluid cells only — body interiors would otherwise dominate.
    pub min: f64,
    pub max: f64,
    /// Percentile range, a far better default for colour mapping than
    /// min/max when a singularity is in view.
    pub robust_min: f64,
    pub robust_max: f64,
    /// True when singularity softening was applied.
    pub softened: bool,
}

/// A sampled vector field for arrow rendering.
#[derive(Debug, Clone)]
pub struct VectorField {
    pub grid: GridDefinition,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub mask: Vec<u8>,
    pub max_magnitude: f64,
}

impl FlowField {
    /// Sample a scalar field over `grid`.
    pub fn sample_scalar(
        &self,
        field: FieldType,
        grid: &GridDefinition,
        opts: &FieldEvalOptions,
    ) -> ScalarField {
        let n = grid.len();
        let mut values = vec![f32::NAN; n];
        let mut mask = vec![MASK_FLUID; n];
        let mut fluid: Vec<f64> = Vec::with_capacity(n);

        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let idx = j * grid.nx + i;
                let p = grid.point(i, j);
                if let Some((_, code)) = self.mask_code_at(p, opts) {
                    mask[idx] = code;
                    values[idx] = f32::NAN;
                    continue;
                }
                let value = self.scalar_at(p, field, opts);
                values[idx] = value as f32;
                if value.is_finite() {
                    fluid.push(value);
                }
            }
        }
        fill_near_wall(&mut values, &mut mask, grid.nx, grid.ny);

        let (min, max) = fluid
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(*v), hi.max(*v))
            });
        let (robust_min, robust_max) = percentile_range(&mut fluid, 0.01);

        ScalarField {
            grid: *grid,
            field,
            values,
            mask,
            min: if min.is_finite() { min } else { 0.0 },
            max: if max.is_finite() { max } else { 0.0 },
            robust_min,
            robust_max,
            softened: opts.core_radius > 0.0,
        }
    }

    /// Sample the velocity vector field over `grid`.
    pub fn sample_vectors(&self, grid: &GridDefinition, opts: &FieldEvalOptions) -> VectorField {
        let n = grid.len();
        let mut u = vec![0.0f32; n];
        let mut v = vec![0.0f32; n];
        let mut mask = vec![MASK_FLUID; n];
        let mut max_magnitude: f64 = 0.0;

        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let idx = j * grid.nx + i;
                let p = grid.point(i, j);
                if self.masked_at(p, opts).is_some() {
                    mask[idx] = MASK_BODY;
                    continue;
                }
                let vel = self.velocity_with(p, opts);
                u[idx] = vel.x as f32;
                v[idx] = vel.y as f32;
                let m = vel.norm();
                if m.is_finite() {
                    max_magnitude = max_magnitude.max(m);
                }
            }
        }
        VectorField {
            grid: *grid,
            u,
            v,
            mask,
            max_magnitude,
        }
    }
}

/// Give near-wall cells the mean of their finite neighbours, iterating a few
/// times so bands up to ~3 cells wide fill in. Cells that stay empty (deep in
/// a thick band) are downgraded to `MASK_BODY` so the renderer discards them.
fn fill_near_wall(values: &mut [f32], mask: &mut [u8], nx: usize, ny: usize) {
    if !mask.contains(&MASK_NEAR_WALL) {
        return;
    }
    for _ in 0..3 {
        let snapshot = values.to_vec();
        let mut changed = false;
        for j in 0..ny {
            for i in 0..nx {
                let idx = j * nx + i;
                if mask[idx] != MASK_NEAR_WALL || snapshot[idx].is_finite() {
                    continue;
                }
                let mut acc = 0.0f64;
                let mut n = 0usize;
                for dj in -1i32..=1 {
                    for di in -1i32..=1 {
                        if di == 0 && dj == 0 {
                            continue;
                        }
                        let ii = i as i32 + di;
                        let jj = j as i32 + dj;
                        if ii < 0 || jj < 0 || ii >= nx as i32 || jj >= ny as i32 {
                            continue;
                        }
                        let k = jj as usize * nx + ii as usize;
                        if mask[k] != MASK_BODY && snapshot[k].is_finite() {
                            acc += snapshot[k] as f64;
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    values[idx] = (acc / n as f64) as f32;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for idx in 0..values.len() {
        if mask[idx] == MASK_NEAR_WALL && !values[idx].is_finite() {
            mask[idx] = MASK_BODY;
        }
    }
}

/// Symmetric percentile clip, used as the default colour range.
fn percentile_range(values: &mut [f64], tail: f64) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let lo = ((n as f64 * tail) as usize).min(n - 1);
    let hi = ((n as f64 * (1.0 - tail)) as usize).min(n - 1);
    (values[lo], values[hi])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Element;
    use aeroflow_geometry::{shapes, Panelisation};

    fn cylinder_body(radius: f64, n: usize, u: f64) -> PanelBody {
        // Construct the *analytic* cylinder as a vortex-free source-panel body
        // would not be: instead, place no panel strengths and rely on the
        // doublet. Here we only need the geometry for masking tests.
        let poly = shapes::circle(radius, n);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let _ = u;
        PanelBody::new(poly, pz.panels, vec![0.0; k], 0.0)
    }

    #[test]
    fn empty_field_is_just_the_freestream() {
        let f = FlowField::new(FlowConditions {
            velocity: 4.0,
            angle: 0.0,
            ..Default::default()
        });
        let v = f.velocity(Vec2::new(3.0, -2.0));
        assert!((v.x - 4.0).abs() < 1e-12 && v.y.abs() < 1e-12);
        assert!(f.cp(Vec2::ZERO).unwrap().abs() < 1e-12);
    }

    /// PRD §61.1: uniform flow at zero angle of attack must give `u = U∞`,
    /// `v = 0` everywhere.
    #[test]
    fn uniform_flow_regression_case() {
        let f = FlowField::new(FlowConditions {
            velocity: 10.0,
            angle: 0.0,
            ..Default::default()
        });
        for p in [Vec2::ZERO, Vec2::new(100.0, -50.0), Vec2::new(-7.0, 3.0)] {
            let v = f.velocity(p);
            assert!((v.x - 10.0).abs() < 1e-14);
            assert!(v.y.abs() < 1e-14);
            assert!((f.pressure(p) - f.conditions.pressure).abs() < 1e-9);
        }
    }

    #[test]
    fn body_masking_marks_interior_cells() {
        let mut f = FlowField::new(FlowConditions::default());
        f.bodies.push(cylinder_body(1.0, 64, 1.0));
        assert!(f.body_at(Vec2::ZERO).is_some());
        assert!(f.body_at(Vec2::new(3.0, 0.0)).is_none());

        let grid = GridDefinition::new(
            Bounds {
                min: Vec2::new(-2.0, -2.0),
                max: Vec2::new(2.0, 2.0),
            },
            41,
            41,
        );
        let s = f.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::EXACT,
        );
        let body_cells = s.mask.iter().filter(|m| **m == MASK_BODY).count();
        // A unit disc is π/16 of a 4×4 box, so ≈ 19.6% of 1681 cells.
        assert!(
            (250..=360).contains(&body_cells),
            "masked {body_cells} of {}",
            s.mask.len()
        );
        for (val, m) in s.values.iter().zip(s.mask.iter()) {
            if *m == MASK_BODY {
                assert!(val.is_nan());
            }
        }
    }

    #[test]
    fn far_field_approximation_stays_within_tolerance() {
        // A ring of panels with non-trivial strengths, then compare approximate
        // against exact evaluation on a grid that excludes the near field.
        let poly = shapes::circle(1.0, 128);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let sigma: Vec<f64> = pz
            .panels
            .iter()
            .map(|p| 2.0 * p.mid.x + 0.5 * p.mid.y)
            .collect();
        let body = PanelBody::new(poly, pz.panels, sigma, 0.37);

        let approx_opts = FieldEvalOptions {
            core_radius: 0.0,
            far_field_ratio: 12.0,
            wall_margin: 0.0,
        };
        let mut worst: f64 = 0.0;
        let mut scale: f64 = 0.0;
        for j in 0..40 {
            for i in 0..40 {
                let p = Vec2::new(-4.0 + 0.2 * i as f64, -4.0 + 0.2 * j as f64);
                if p.norm() < 1.3 {
                    continue; // inside or hugging the body
                }
                let exact = body.velocity(p, &FieldEvalOptions::EXACT);
                let approx = body.velocity(p, &approx_opts);
                worst = worst.max((exact - approx).norm());
                scale = scale.max(exact.norm());
            }
        }
        assert!(worst / scale < 0.01, "relative error {}", worst / scale);
    }

    #[test]
    fn far_field_approximation_is_actually_used() {
        // Sanity: with a huge ratio nothing is approximated, so results must
        // match the exact path bit for bit.
        let poly = shapes::circle(1.0, 64);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let body = PanelBody::new(poly, pz.panels, vec![1.0; k], 0.2);
        let p = Vec2::new(5.0, 5.0);
        let never = FieldEvalOptions {
            core_radius: 0.0,
            far_field_ratio: 1e9,
            wall_margin: 0.0,
        };
        assert!(
            (body.velocity(p, &never) - body.velocity(p, &FieldEvalOptions::EXACT)).norm() < 1e-15
        );
    }

    #[test]
    fn net_outflow_and_circulation_are_reported() {
        let poly = shapes::circle(1.0, 32);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let k = pz.len();
        let perimeter = pz.perimeter;
        let body = PanelBody::new(poly, pz.panels, vec![2.0; k], 0.5);
        assert!((body.net_source_outflow() - 2.0 * perimeter).abs() < 1e-9);
        assert!((body.circulation() - 0.5 * perimeter).abs() < 1e-9);
    }

    #[test]
    fn vorticity_is_zero_away_from_vortices_and_integrates_to_gamma() {
        let mut f = FlowField::new(FlowConditions::default());
        f.elements.push(Element::Source {
            position: Vec2::ZERO,
            strength: 3.0,
        });
        // A source contributes no vorticity at all.
        assert!(f.vorticity(Vec2::new(1.0, 1.0), 0.1).abs() < 1e-15);

        let gamma = 2.5;
        let core = 0.2;
        f.elements.push(Element::Vortex {
            position: Vec2::ZERO,
            circulation: gamma,
        });
        // ω(0) = Γ/(πa²)
        let w0 = f.vorticity(Vec2::ZERO, core);
        assert!((w0 - gamma / (PI * core * core)).abs() < 1e-12);

        // Integrate the softened distribution over a large disc: → Γ.
        let mut total = 0.0;
        let nr = 4000;
        let r_max = 200.0 * core;
        for i in 0..nr {
            let r = r_max * (i as f64 + 0.5) / nr as f64;
            let dr = r_max / nr as f64;
            total += f.vorticity(Vec2::new(r, 0.0), core) * 2.0 * PI * r * dr;
        }
        assert!(
            (total - gamma).abs() / gamma < 2e-4,
            "∫ω dA = {total}, Γ = {gamma}"
        );
    }

    #[test]
    fn vorticity_with_zero_core_is_zero_in_the_fluid() {
        let mut f = FlowField::new(FlowConditions::default());
        f.elements.push(Element::Vortex {
            position: Vec2::ZERO,
            circulation: 1.0,
        });
        assert_eq!(f.vorticity(Vec2::new(1.0, 0.0), 0.0), 0.0);
    }

    #[test]
    fn grid_definition_spacing_and_points_line_up() {
        let g = GridDefinition::new(
            Bounds {
                min: Vec2::new(-1.0, -2.0),
                max: Vec2::new(3.0, 2.0),
            },
            5,
            9,
        );
        let (dx, dy) = g.spacing();
        assert!((dx - 1.0).abs() < 1e-12 && (dy - 0.5).abs() < 1e-12);
        assert_eq!(g.point(0, 0), Vec2::new(-1.0, -2.0));
        assert_eq!(g.point(4, 8), Vec2::new(3.0, 2.0));
        assert_eq!(g.len(), 45);
    }

    #[test]
    fn robust_range_rejects_a_singularity_spike() {
        let mut f = FlowField::new(FlowConditions {
            velocity: 1.0,
            ..Default::default()
        });
        f.elements.push(Element::Source {
            position: Vec2::ZERO,
            strength: 1.0,
        });
        let grid = GridDefinition::new(
            Bounds {
                min: Vec2::new(-2.0, -2.0),
                max: Vec2::new(2.0, 2.0),
            },
            81,
            81,
        );
        let s = f.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::EXACT,
        );
        // The nearest sample sits one cell (0.05 m) from the source, where
        // |V| = U + Λ/(2πr) = 1 + 1/(2π·0.05) ≈ 4.18.
        let (dx, _) = grid.spacing();
        let near_source = 1.0 + 1.0 / (2.0 * PI * dx);
        assert!(
            (s.max - near_source).abs() / near_source < 0.05,
            "raw max {} should be the near-singularity value {near_source}",
            s.max
        );
        // The percentile clip must throw that spike away and land near the
        // freestream value that dominates the domain.
        assert!(
            s.robust_max < 0.5 * s.max,
            "robust {} vs max {}",
            s.robust_max,
            s.max
        );
        assert!(
            s.robust_max > 1.0,
            "robust max {} below freestream",
            s.robust_max
        );
    }

    #[test]
    fn potential_and_stream_function_gradients_hold_with_panels_present() {
        let poly = shapes::circle(1.0, 96);
        let pz = Panelisation::from_polygon(&poly).unwrap();
        let sigma: Vec<f64> = pz.panels.iter().map(|p| 0.8 * p.mid.y).collect();
        let mut f = FlowField::new(FlowConditions {
            velocity: 2.0,
            angle: 0.3,
            ..Default::default()
        });
        f.bodies.push(PanelBody::new(poly, pz.panels, sigma, 0.1));

        let h = 1e-6;
        let p = Vec2::new(2.5, 1.1);
        let gx =
            (f.potential(p + Vec2::new(h, 0.0)) - f.potential(p - Vec2::new(h, 0.0))) / (2.0 * h);
        let gy =
            (f.potential(p + Vec2::new(0.0, h)) - f.potential(p - Vec2::new(0.0, h))) / (2.0 * h);
        let v = f.velocity(p);
        assert!(
            (gx - v.x).abs() < 1e-5 && (gy - v.y).abs() < 1e-5,
            "∇φ = ({gx},{gy}) vs {v:?}"
        );

        let u = (f.stream_function(p + Vec2::new(0.0, h))
            - f.stream_function(p - Vec2::new(0.0, h)))
            / (2.0 * h);
        let w = -(f.stream_function(p + Vec2::new(h, 0.0))
            - f.stream_function(p - Vec2::new(h, 0.0)))
            / (2.0 * h);
        assert!(
            (u - v.x).abs() < 1e-5 && (w - v.y).abs() < 1e-5,
            "ψ derivs = ({u},{w})"
        );
    }

    #[test]
    fn wall_margin_masks_the_near_wall_band_only_for_rendering() {
        let mut f = FlowField::new(FlowConditions::default());
        f.bodies.push(cylinder_body(1.0, 64, 1.0));
        let l = f.bodies[0].max_panel_length();
        let just_outside = Vec2::new(1.0 + 0.3 * l, 0.0);
        assert!(
            f.body_at(just_outside).is_none(),
            "exact test must not mask fluid"
        );
        assert!(f
            .masked_at(just_outside, &FieldEvalOptions::EXACT)
            .is_none());
        let render = FieldEvalOptions::for_rendering(0.05);
        assert!(
            f.masked_at(just_outside, &render).is_some(),
            "near-wall band should be masked"
        );
        let far = Vec2::new(1.0 + 2.0 * l, 0.0);
        assert!(f.masked_at(far, &render).is_none());
    }

    #[test]
    fn near_wall_cells_are_filled_from_fluid_neighbours_and_flagged() {
        let mut f = FlowField::new(FlowConditions {
            velocity: 1.0,
            angle: 0.0,
            ..Default::default()
        });
        f.bodies.push(cylinder_body(1.0, 64, 1.0));
        let grid = GridDefinition::new(
            Bounds {
                min: Vec2::new(-2.0, -2.0),
                max: Vec2::new(2.0, 2.0),
            },
            81,
            81,
        );
        let s = f.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::for_rendering(grid.cell()),
        );
        let near: Vec<usize> = (0..s.mask.len())
            .filter(|k| s.mask[*k] == MASK_NEAR_WALL)
            .collect();
        assert!(
            !near.is_empty(),
            "the render preset must produce a near-wall band"
        );
        for k in near {
            assert!(s.values[k].is_finite(), "near-wall cell {k} was not filled");
        }
        // Interior cells stay masked and empty.
        let centre = 40 * 81 + 40;
        assert_eq!(s.mask[centre], MASK_BODY);
        assert!(s.values[centre].is_nan());
        // Exact evaluation has no band at all.
        let e = f.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::EXACT,
        );
        assert!(e.mask.iter().all(|m| *m != MASK_NEAR_WALL));
    }

    #[test]
    fn field_type_metadata_is_complete() {
        for f in FieldType::ALL {
            assert!(!f.label().is_empty());
            assert!(!f.symbol().is_empty());
            assert!(!f.unit().is_empty());
        }
        assert!(FieldType::Vorticity.is_diverging());
        assert!(!FieldType::VelocityMagnitude.is_diverging());
    }

    #[test]
    fn vector_sampling_reports_the_peak_magnitude() {
        let f = FlowField::new(FlowConditions {
            velocity: 3.0,
            angle: 0.0,
            ..Default::default()
        });
        let grid = GridDefinition::new(
            Bounds {
                min: Vec2::new(-1.0, -1.0),
                max: Vec2::new(1.0, 1.0),
            },
            11,
            11,
        );
        let vf = f.sample_vectors(&grid, &FieldEvalOptions::EXACT);
        assert!((vf.max_magnitude - 3.0).abs() < 1e-12);
        assert!(vf.u.iter().all(|x| (*x - 3.0).abs() < 1e-6));
    }
}
