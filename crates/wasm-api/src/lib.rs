//! # aeroflow-wasm-api
//!
//! The JavaScript-facing boundary (PRD §42, WASM-002/003). It is deliberately
//! thin: every function parses its inputs, calls `aeroflow-solver`, and
//! serialises the result. No numerical logic lives here, so the core stays
//! testable natively (WASM-007) and the UI never sees solver internals.
//!
//! ## Data transfer
//!
//! * Structured results (solutions, diagnostics, descriptors) cross as plain
//!   JS objects via `serde-wasm-bindgen`.
//! * Bulk render data (fields, streamlines) cross as typed arrays
//!   (`Float32Array`, `Uint8Array`, `Uint32Array`) — PRD §70's "render-oriented
//!   float buffers".
//!
//! ## Errors
//!
//! Every fallible function returns `Result<_, JsValue>`, which surfaces as a
//! thrown `Error` with an actionable message on the JS side.

use aeroflow_flow_core::{element_descriptors, field_note, FieldType};
use aeroflow_geometry::{
    import_geometry as geometry_import, shapes, CleanupOptions, PanelDistribution, RepanelConfig,
    Vec2,
};
use aeroflow_solver::{
    Bounds, FieldEvalOptions, GridDefinition, Scene, SeedingConfig, Simulation, StreamlineConfig,
    SweepConfig, SweepRun,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Sub-millisecond clock: `performance.now()` from the worker's global scope.
/// `Date.now()` only resolves whole milliseconds, which reads as 0 for most
/// solves. Falls back to it if `performance` is unavailable.
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    use wasm_bindgen::JsCast;
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &JsValue::from_str("performance"))
        .ok()
        .and_then(|perf| {
            let now = js_sys::Reflect::get(&perf, &JsValue::from_str("now")).ok()?;
            let f: js_sys::Function = now.dyn_into().ok()?;
            f.call0(&perf).ok()?.as_f64()
        })
        .unwrap_or_else(js_sys::Date::now)
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    aeroflow_solver::no_clock()
}

fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsValue> {
    let ser = serde_wasm_bindgen::Serializer::json_compatible();
    v.serialize(&ser).map_err(js_err)
}

fn set(obj: &js_sys::Object, key: &str, value: impl Into<JsValue>) {
    // Reflect::set only fails on frozen objects, which ours never are.
    let _ = js_sys::Reflect::set(obj, &JsValue::from_str(key), &value.into());
}

#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Crate version, for the About panel and bug reports.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ───────────────────────────── Field requests ─────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FieldRequest {
    field: FieldType,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    nx: usize,
    ny: usize,
    /// When true, use the rendering preset: softened singularities, far-field
    /// approximation, near-wall mask. Numerical exports pass `false`.
    #[serde(default)]
    render: bool,
}

impl FieldRequest {
    fn grid(&self) -> GridDefinition {
        GridDefinition::new(
            Bounds {
                min: Vec2::new(self.min_x, self.min_y),
                max: Vec2::new(self.max_x, self.max_y),
            },
            self.nx,
            self.ny,
        )
    }
    fn options(&self, grid: &GridDefinition) -> FieldEvalOptions {
        if self.render {
            FieldEvalOptions::for_rendering(grid.cell())
        } else {
            FieldEvalOptions::EXACT
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamlineRequest {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    #[serde(default)]
    seeding: Option<SeedingConfig>,
    /// Optional manual seeds, `[x0, y0, x1, y1, …]`.
    #[serde(default)]
    seeds: Vec<f64>,
    #[serde(default)]
    max_steps: Option<usize>,
}

/// Flatten streamlines into typed-array-friendly buffers.
///
/// `data` holds `x, y, speed` triples; `offsets[i]..offsets[i+1]` is the
/// point range of line `i` (so `offsets.len() == lines + 1`).
#[derive(Debug, Default, PartialEq)]
pub struct FlatStreamlines {
    pub data: Vec<f32>,
    pub offsets: Vec<u32>,
    pub seed_index: Vec<u32>,
}

pub fn flatten_streamlines(lines: &[aeroflow_solver::Streamline]) -> FlatStreamlines {
    let total: usize = lines.iter().map(|l| l.points.len()).sum();
    let mut out = FlatStreamlines {
        data: Vec::with_capacity(total * 3),
        offsets: Vec::with_capacity(lines.len() + 1),
        seed_index: Vec::with_capacity(lines.len()),
    };
    let mut cursor = 0u32;
    for l in lines {
        out.offsets.push(cursor);
        out.seed_index.push(l.seed_index as u32);
        for (p, s) in l.points.iter().zip(l.speeds.iter()) {
            out.data.push(p.x as f32);
            out.data.push(p.y as f32);
            out.data.push(*s);
        }
        cursor += l.points.len() as u32;
    }
    out.offsets.push(cursor);
    out
}

// ───────────────────────────── The solver handle ─────────────────────────────

/// One simulation session: owns the scene, the cached influence matrix, the
/// latest solution and any sweep in progress.
#[wasm_bindgen]
pub struct AeroflowSolver {
    sim: Simulation,
    sweep: Option<SweepRun>,
}

impl Default for AeroflowSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl AeroflowSolver {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            sim: Simulation::new(now_ms),
            sweep: None,
        }
    }

    /// Replace the scene from its JSON form. Validation problems are returned
    /// as an error; the previous scene is kept in that case.
    #[wasm_bindgen(js_name = setScene)]
    pub fn set_scene(&mut self, scene_json: &str) -> Result<(), JsValue> {
        let scene: Scene = serde_json::from_str(scene_json)
            .map_err(|e| js_err(format!("The scene could not be read: {e}")))?;
        let problems = scene.validate();
        if !problems.is_empty() {
            return Err(js_err(problems.join(" ")));
        }
        self.sim.set_scene(scene);
        Ok(())
    }

    /// The current scene as JSON (the `.aeroflow.json` format).
    #[wasm_bindgen(js_name = sceneJson)]
    pub fn scene_json(&self) -> Result<String, JsValue> {
        serde_json::to_string_pretty(self.sim.scene()).map_err(js_err)
    }

    /// Solve the current scene and return the structured `Solution`.
    pub fn solve(&mut self) -> Result<JsValue, JsValue> {
        let solution = self.sim.solve();
        to_js(solution)
    }

    /// Sample a scalar field. Returns
    /// `{ values: Float32Array, mask: Uint8Array, nx, ny, min, max, robustMin,
    ///    robustMax, softened, field }`.
    #[wasm_bindgen(js_name = sampleScalar)]
    pub fn sample_scalar(&self, request_json: &str) -> Result<JsValue, JsValue> {
        let req: FieldRequest = serde_json::from_str(request_json).map_err(js_err)?;
        let grid = req.grid();
        let opts = req.options(&grid);
        let f = self
            .sim
            .sample_scalar(req.field, &grid, &opts)
            .ok_or_else(|| js_err("No solution yet: call solve() first."))?;
        let obj = js_sys::Object::new();
        set(&obj, "values", js_sys::Float32Array::from(&f.values[..]));
        set(&obj, "mask", js_sys::Uint8Array::from(&f.mask[..]));
        set(&obj, "nx", f.grid.nx as u32);
        set(&obj, "ny", f.grid.ny as u32);
        set(&obj, "min", f.min);
        set(&obj, "max", f.max);
        set(&obj, "robustMin", f.robust_min);
        set(&obj, "robustMax", f.robust_max);
        set(&obj, "softened", f.softened);
        set(
            &obj,
            "field",
            serde_json::to_value(f.field)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default(),
        );
        Ok(obj.into())
    }

    /// Sample the velocity vector field. Returns
    /// `{ u: Float32Array, v: Float32Array, mask: Uint8Array, nx, ny, maxMagnitude }`.
    #[wasm_bindgen(js_name = sampleVectors)]
    pub fn sample_vectors(&self, request_json: &str) -> Result<JsValue, JsValue> {
        let req: FieldRequest = serde_json::from_str(request_json).map_err(js_err)?;
        let grid = req.grid();
        let opts = req.options(&grid);
        let f = self
            .sim
            .sample_vectors(&grid, &opts)
            .ok_or_else(|| js_err("No solution yet: call solve() first."))?;
        let obj = js_sys::Object::new();
        set(&obj, "u", js_sys::Float32Array::from(&f.u[..]));
        set(&obj, "v", js_sys::Float32Array::from(&f.v[..]));
        set(&obj, "mask", js_sys::Uint8Array::from(&f.mask[..]));
        set(&obj, "nx", f.grid.nx as u32);
        set(&obj, "ny", f.grid.ny as u32);
        set(&obj, "maxMagnitude", f.max_magnitude);
        Ok(obj.into())
    }

    /// Trace streamlines. Returns
    /// `{ data: Float32Array (x,y,speed triples), offsets: Uint32Array,
    ///    seedIndex: Uint32Array, count }`.
    pub fn streamlines(&self, request_json: &str) -> Result<JsValue, JsValue> {
        let req: StreamlineRequest = serde_json::from_str(request_json).map_err(js_err)?;
        let bounds = Bounds {
            min: Vec2::new(req.min_x, req.min_y),
            max: Vec2::new(req.max_x, req.max_y),
        };
        // Render path: far-field approximation keeps tracing cost independent of panel count.
        let mut cfg = StreamlineConfig::for_rendering(bounds);
        if let Some(m) = req.max_steps {
            cfg.max_steps = m.clamp(16, 20_000);
        }
        let field = self
            .sim
            .field()
            .ok_or_else(|| js_err("No solution yet: call solve() first."))?;

        let mut lines = Vec::new();
        if !req.seeds.is_empty() {
            for pair in req.seeds.chunks_exact(2) {
                let line = aeroflow_flow_core::trace(field, Vec2::new(pair[0], pair[1]), &cfg);
                if line.points.len() > 1 {
                    lines.push(line);
                }
            }
        }
        if req.seeds.is_empty() || req.seeding.is_some() {
            let seeding = req.seeding.unwrap_or_default();
            lines.extend(self.sim.streamlines(&cfg, &seeding));
        }

        let flat = flatten_streamlines(&lines);
        let obj = js_sys::Object::new();
        set(&obj, "data", js_sys::Float32Array::from(&flat.data[..]));
        set(
            &obj,
            "offsets",
            js_sys::Uint32Array::from(&flat.offsets[..]),
        );
        set(
            &obj,
            "seedIndex",
            js_sys::Uint32Array::from(&flat.seed_index[..]),
        );
        set(&obj, "count", lines.len() as u32);
        Ok(obj.into())
    }

    /// Point read-out for hover tooltips.
    pub fn probe(&self, x: f64, y: f64) -> Result<JsValue, JsValue> {
        match self.sim.probe(Vec2::new(x, y)) {
            Some(p) => to_js(&p),
            None => Ok(JsValue::NULL),
        }
    }

    /// Start a parameter sweep. Returns the number of steps. Call
    /// `sweepStep()` repeatedly; the worker checks for cancellation between
    /// calls (PRD §60).
    #[wasm_bindgen(js_name = beginSweep)]
    pub fn begin_sweep(&mut self, config_json: &str) -> Result<u32, JsValue> {
        let cfg: SweepConfig = serde_json::from_str(config_json).map_err(js_err)?;
        let run = SweepRun::new(self.sim.scene(), cfg, now_ms).map_err(js_err)?;
        let total = run.total() as u32;
        self.sweep = Some(run);
        Ok(total)
    }

    /// Compute the next sweep point, or `null` when the sweep is finished.
    #[wasm_bindgen(js_name = sweepStep)]
    pub fn sweep_step(&mut self) -> Result<JsValue, JsValue> {
        let run = self
            .sweep
            .as_mut()
            .ok_or_else(|| js_err("No sweep in progress."))?;
        match run.step() {
            Some(p) => to_js(&p),
            None => Ok(JsValue::NULL),
        }
    }

    /// Points computed so far, and whether the sweep completed.
    #[wasm_bindgen(js_name = sweepResult)]
    pub fn sweep_result(&self) -> Result<JsValue, JsValue> {
        match &self.sweep {
            Some(run) => to_js(&run.result()),
            None => Ok(JsValue::NULL),
        }
    }

    #[wasm_bindgen(js_name = cancelSweep)]
    pub fn cancel_sweep(&mut self) {
        self.sweep = None;
    }
}

// ───────────────────────────── Geometry ─────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    points: Vec<Vec2>,
    name: Option<String>,
    notes: Vec<String>,
    issues: Vec<aeroflow_geometry::GeometryIssue>,
    kutta_applicable: bool,
    trailing_edge: Option<aeroflow_geometry::CornerInfo>,
    original_point_count: usize,
    panel_count: usize,
    chord: f64,
}

/// Pure helper (no `JsValue`), so it stays testable on native targets.
fn parse_distribution(s: &str) -> Result<PanelDistribution, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("Unknown panel distribution '{s}'."))
}

/// Parse, validate, clean and re-panelise a coordinate file (PRD §10).
///
/// The returned `points` are body-local, counter-clockwise, and start at the
/// detected trailing edge; store them in a `points` body geometry.
#[wasm_bindgen(js_name = importGeometry)]
pub fn import_geometry(
    text: &str,
    panel_count: u32,
    distribution: &str,
) -> Result<JsValue, JsValue> {
    let dist = parse_distribution(distribution).map_err(js_err)?;
    let cfg = RepanelConfig {
        distribution: dist,
        count: panel_count as usize,
        ..Default::default()
    };
    let r = geometry_import(text, &cfg, &CleanupOptions::default()).map_err(js_err)?;
    let chord = r.polygon.diameter().map(|d| d.2).unwrap_or(0.0);
    to_js(&ImportResponse {
        panel_count: r.panelisation.len(),
        points: r.polygon.points,
        name: r.name,
        notes: r.notes,
        issues: r.report.issues,
        kutta_applicable: r.kutta_applicable,
        trailing_edge: r.trailing_edge,
        original_point_count: r.original_point_count,
        chord,
    })
}

/// Validate a coordinate file without importing it — for the import preview.
#[wasm_bindgen(js_name = validateGeometry)]
pub fn validate_geometry(text: &str) -> Result<JsValue, JsValue> {
    let parsed = aeroflow_geometry::parse_coordinates(text).map_err(js_err)?;
    let report = aeroflow_geometry::validate(
        &parsed.points,
        &aeroflow_geometry::ValidationConfig::default(),
    );
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct V {
        points: Vec<Vec2>,
        name: Option<String>,
        notes: Vec<String>,
        issues: Vec<aeroflow_geometry::GeometryIssue>,
        has_errors: bool,
    }
    to_js(&V {
        has_errors: report.has_errors(),
        points: parsed.points,
        name: parsed.name,
        notes: parsed.notes,
        issues: report.issues,
    })
}

/// Points of a parametric shape, for previews. `spec_json` is a
/// `BodyGeometry` value.
#[wasm_bindgen(js_name = generateGeometry)]
pub fn generate_geometry(spec_json: &str, count: u32) -> Result<JsValue, JsValue> {
    use aeroflow_solver::BodyGeometry;
    let spec: BodyGeometry = serde_json::from_str(spec_json).map_err(js_err)?;
    let n = (count as usize).max(16);
    let poly = match spec {
        BodyGeometry::Points { points } => aeroflow_geometry::Polygon::new(points),
        BodyGeometry::Naca4 { code, chord } => {
            let mut n4 = shapes::Naca4::parse(&code)
                .ok_or_else(|| js_err(format!("'{code}' is not a valid NACA 4-digit code.")))?;
            n4.chord = chord;
            n4.generate(n)
        }
        BodyGeometry::Circle { radius } => shapes::circle(radius, n),
        BodyGeometry::Ellipse {
            semi_axis_x,
            semi_axis_y,
        } => shapes::ellipse(semi_axis_x, semi_axis_y, n),
        BodyGeometry::Joukowski { thickness, camber } => shapes::Joukowski {
            dx: thickness,
            dy: camber,
            c: 1.0,
        }
        .generate(n),
    };
    to_js(&poly.points)
}

/// Points as CSV with an `x,y` header (geometry export, PRD §52).
#[wasm_bindgen(js_name = pointsToCsv)]
pub fn points_to_csv(points_json: &str) -> Result<String, JsValue> {
    let pts: Vec<Vec2> = serde_json::from_str(points_json).map_err(js_err)?;
    Ok(aeroflow_geometry::parse::to_csv(&pts))
}

// ───────────────────────────── Metadata ─────────────────────────────

/// Element metadata used to build the properties panel (PRD §7, §33).
#[wasm_bindgen(js_name = elementDescriptors)]
pub fn element_descriptors_js() -> Result<JsValue, JsValue> {
    to_js(&element_descriptors())
}

/// All contextual help topics (PRD §56).
#[wasm_bindgen(js_name = helpTopics)]
pub fn help_topics() -> Result<JsValue, JsValue> {
    let topics: Vec<&aeroflow_flow_core::HelpTopic> = aeroflow_flow_core::concept_help()
        .iter()
        .chain(element_descriptors().iter().map(|d| &d.help))
        .collect();
    to_js(&topics)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldInfo {
    id: FieldType,
    label: &'static str,
    symbol: &'static str,
    unit: &'static str,
    diverging: bool,
    note: &'static str,
}

/// Field types with labels, units and notes for the visualisation toolbar.
#[wasm_bindgen(js_name = fieldTypes)]
pub fn field_types() -> Result<JsValue, JsValue> {
    let infos: Vec<FieldInfo> = FieldType::ALL
        .iter()
        .map(|f| FieldInfo {
            id: *f,
            label: f.label(),
            symbol: f.symbol(),
            unit: f.unit(),
            diverging: f.is_diverging(),
            note: field_note(*f),
        })
        .collect();
    to_js(&infos)
}

/// The modelling assumptions attached to every result (PRD §15).
#[wasm_bindgen]
pub fn assumptions() -> Result<JsValue, JsValue> {
    to_js(&aeroflow_solver::ASSUMPTIONS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeroflow_flow_core::Termination;
    use aeroflow_solver::Streamline;

    #[test]
    fn flatten_streamlines_produces_consistent_offsets() {
        let l1 = Streamline {
            points: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(2.0, 0.0),
            ],
            speeds: vec![1.0, 2.0, 3.0],
            arc_length: 2.0,
            termination_forward: Termination::LeftDomain,
            termination_backward: None,
            seed_index: 0,
        };
        let l2 = Streamline {
            points: vec![Vec2::new(5.0, 5.0), Vec2::new(6.0, 5.0)],
            speeds: vec![4.0, 5.0],
            arc_length: 1.0,
            termination_forward: Termination::HitBody,
            termination_backward: Some(Termination::LeftDomain),
            seed_index: 1,
        };
        let flat = flatten_streamlines(&[l1, l2]);
        assert_eq!(flat.offsets, vec![0, 3, 5]);
        assert_eq!(flat.seed_index, vec![0, 1]);
        assert_eq!(flat.data.len(), 15);
        assert_eq!(&flat.data[0..3], &[0.0, 0.0, 1.0]);
        assert_eq!(&flat.data[9..12], &[5.0, 5.0, 4.0]);
    }

    #[test]
    fn empty_streamline_set_flattens_to_a_single_offset() {
        let flat = flatten_streamlines(&[]);
        assert_eq!(flat.offsets, vec![0]);
        assert!(flat.data.is_empty());
    }

    #[test]
    fn field_request_parses_camel_case_and_defaults_render_to_false() {
        let r: FieldRequest = serde_json::from_str(
            r#"{"field":"velocityMagnitude","minX":-1,"minY":-1,"maxX":1,"maxY":1,"nx":10,"ny":5}"#,
        )
        .unwrap();
        assert_eq!(r.field, FieldType::VelocityMagnitude);
        assert!(!r.render);
        assert_eq!(r.grid().len(), 50);
        assert_eq!(r.options(&r.grid()), FieldEvalOptions::EXACT);
    }

    #[test]
    fn distribution_strings_match_the_serde_names() {
        assert_eq!(
            parse_distribution("cosine").unwrap(),
            PanelDistribution::Cosine
        );
        assert_eq!(
            parse_distribution("asImported").unwrap(),
            PanelDistribution::AsImported
        );
        assert!(parse_distribution("bogus").is_err());
    }
}
