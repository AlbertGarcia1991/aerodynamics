//! Scene → Solution orchestration (PRD §40 "Solver API", §43, §44, §71).

use crate::prepare::{prepare_body, PreparedBody};
use crate::scene::Scene;
use aeroflow_flow_core::{
    element_force, integrate_forces, surface_points, total_forces, trace_set, BodyForces,
    FieldEvalOptions, FieldType, FlowField, GridDefinition, PanelBody, ScalarField, SeedingConfig,
    Streamline, StreamlineConfig, SurfacePoint, TotalForces, VectorField,
};
use aeroflow_geometry::{CornerInfo, Vec2};
use aeroflow_panel_method::{BodySpec, CirculationMode, PanelDiagnostics, PanelSystem};
use serde::{Deserialize, Serialize};

/// Millisecond clock. Injected because `std::time::Instant` is unavailable on
/// `wasm32-unknown-unknown`; the WASM layer passes `performance.now()`.
pub type NowFn = fn() -> f64;

/// A clock that always reads zero — timings come out as `0.0`.
pub fn no_clock() -> f64 {
    0.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SolveStatus {
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningSeverity {
    Info,
    Warning,
}

/// A user-facing warning (PRD §45: identify the relevant object).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub code: String,
    pub severity: WarningSeverity,
    pub message: String,
    pub object_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyResult {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub forces: BodyForces,
    pub surface: Vec<SurfacePoint>,
    pub panel_count: usize,
    pub circulation_mode: CirculationMode,
    pub kutta_applicable: bool,
    pub trailing_edge: Option<CornerInfo>,
    /// World-space contour for rendering.
    pub polygon: Vec<Vec2>,
    pub vortex_strength: f64,
    pub net_source_outflow: f64,
    pub notes: Vec<String>,
}

/// Force on one elementary singularity (Lagally theorem): the force needed to
/// hold it fixed in the flow. See `aeroflow_flow_core::lagally`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementResult {
    pub id: String,
    pub name: String,
    pub position: Vec2,
    /// Force per unit span [N/m].
    pub force: Vec2,
    /// Wind-axis components of `force` [N/m].
    pub lift: f64,
    pub drag: f64,
    /// Velocity induced at the element by everything else [m/s].
    pub external_velocity: Vec2,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    pub prepare_ms: f64,
    pub assemble_ms: f64,
    pub solve_ms: f64,
    pub forces_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub panel: Option<PanelDiagnostics>,
    pub timings: Timings,
    /// True when the influence matrix from the previous solve was reused
    /// because no body geometry changed.
    pub system_reused: bool,
    pub element_count: usize,
    pub body_count: usize,
    /// Point vortices plus bound circulation [m²/s], counter-clockwise +.
    pub total_circulation: f64,
    /// Net volumetric outflow of the elementary elements [m²/s].
    pub net_outflow: f64,
}

/// The modelling assumptions attached to every result (PRD §15, §86.6).
pub const ASSUMPTIONS: &[&str] = &[
    "Steady flow",
    "Two-dimensional (forces are per unit span)",
    "Incompressible, constant density",
    "Inviscid: no boundary layer, no skin friction, no separation",
    "Irrotational except at explicitly placed vortices and body-bound vortex sheets",
    "Pressure drag of a closed body is identically zero (d'Alembert); reported drag is discretisation error, not a prediction",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub status: SolveStatus,
    pub bodies: Vec<BodyResult>,
    /// Lagally forces on the visible elementary singularities.
    pub elements: Vec<ElementResult>,
    pub total: TotalForces,
    pub diagnostics: Diagnostics,
    pub warnings: Vec<Warning>,
    pub error: Option<String>,
    pub assumptions: Vec<String>,
}

impl Solution {
    fn error(message: String, timings: Timings, element_count: usize, body_count: usize) -> Self {
        Self {
            status: SolveStatus::Error,
            bodies: Vec::new(),
            elements: Vec::new(),
            total: TotalForces::default(),
            diagnostics: Diagnostics {
                panel: None,
                timings,
                system_reused: false,
                element_count,
                body_count,
                total_circulation: 0.0,
                net_outflow: 0.0,
            },
            warnings: Vec::new(),
            error: Some(message),
            assumptions: ASSUMPTIONS.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn body(&self, id: &str) -> Option<&BodyResult> {
        self.bodies.iter().find(|b| b.id == id)
    }
}

/// Point sample for hover read-outs (PRD §36 "accessible numerical
/// alternative" to the canvas).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub position: Vec2,
    pub velocity: Vec2,
    pub speed: f64,
    pub pressure: f64,
    pub cp: Option<f64>,
    pub potential: f64,
    pub stream_function: f64,
    /// Body containing the point, if any.
    pub body_id: Option<String>,
}

/// Cached influence matrix keyed by a hash of the body geometry.
pub(crate) struct SystemCache {
    key: u64,
    system: PanelSystem,
}

/// FNV-1a over the inputs that determine the influence matrix.
fn geometry_key(scene: &Scene) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for b in scene.bodies.iter().filter(|b| b.visible) {
        feed(b.id.as_bytes());
        // Serialising the geometry/transform/circulation/panel settings is
        // the simplest way to hash every field without a hand-written list
        // that could drift from the struct.
        let repr = serde_json::to_vec(&(
            &b.geometry,
            b.position,
            b.rotation,
            b.scale,
            b.circulation,
            b.panels,
        ))
        .unwrap_or_default();
        feed(&repr);
    }
    h
}

/// Run the complete pipeline for `scene`, reusing `cache` when the geometry
/// is unchanged. Returns the solution and, on success, the evaluable field.
pub(crate) fn solve_scene(
    scene: &Scene,
    cache: &mut Option<SystemCache>,
    now: NowFn,
) -> (Solution, Option<FlowField>) {
    let t0 = now();
    let element_count = scene.elements.iter().filter(|e| e.visible).count();
    let body_count = scene.bodies.iter().filter(|b| b.visible).count();
    let mut timings = Timings::default();

    let problems = scene.validate();
    if !problems.is_empty() {
        timings.total_ms = now() - t0;
        return (
            Solution::error(problems.join(" "), timings, element_count, body_count),
            None,
        );
    }

    // 1. Prepare bodies.
    let t_prep = now();
    let mut prepared: Vec<PreparedBody> = Vec::with_capacity(body_count);
    for (i, b) in scene.bodies.iter().enumerate().filter(|(_, b)| b.visible) {
        match prepare_body(b, i) {
            Ok(p) => prepared.push(p),
            Err(e) => {
                timings.total_ms = now() - t0;
                return (
                    Solution::error(e.to_string(), timings, element_count, body_count),
                    None,
                );
            }
        }
    }
    timings.prepare_ms = now() - t_prep;

    // 2. Ambient field: freestream + visible elements.
    let mut field = FlowField::new(scene.conditions);
    field.elements = scene
        .elements
        .iter()
        .filter(|e| e.visible)
        .map(|e| e.element)
        .collect();

    // 3. Influence matrix (cached) and solve.
    let mut system_reused = false;
    let mut panel_diag: Option<PanelDiagnostics> = None;
    let mut solved: Vec<aeroflow_panel_method::SolvedBody> = Vec::new();
    if !prepared.is_empty() {
        let key = geometry_key(scene);
        let t_asm = now();
        let reuse = matches!(cache, Some(c) if c.key == key);
        if !reuse {
            let specs: Vec<BodySpec> = prepared
                .iter()
                .map(|p| BodySpec {
                    polygon: p.polygon.clone(),
                    panels: p.panels.clone(),
                    circulation: p.mode,
                })
                .collect();
            match PanelSystem::assemble(&specs) {
                Ok(system) => *cache = Some(SystemCache { key, system }),
                Err(e) => {
                    timings.total_ms = now() - t0;
                    return (
                        Solution::error(e.to_string(), timings, element_count, body_count),
                        None,
                    );
                }
            }
        } else {
            system_reused = true;
        }
        timings.assemble_ms = now() - t_asm;

        let t_solve = now();
        let system = &cache.as_ref().expect("cache populated above").system;
        match system.solve(&field) {
            Ok(sol) => {
                panel_diag = Some(sol.diagnostics);
                solved = sol.bodies;
            }
            Err(e) => {
                timings.total_ms = now() - t0;
                return (
                    Solution::error(e.to_string(), timings, element_count, body_count),
                    None,
                );
            }
        }
        timings.solve_ms = now() - t_solve;
    }

    // 4. Assemble the evaluable field and per-body results.
    let t_forces = now();
    let mut bodies: Vec<BodyResult> = Vec::with_capacity(prepared.len());
    for (p, s) in prepared.iter().zip(solved.iter()) {
        field.bodies.push(PanelBody::new(
            p.polygon.clone(),
            p.panels.clone(),
            s.source_strengths.clone(),
            s.vortex_strength,
        ));
        let surface = surface_points(
            &p.polygon,
            &p.panels,
            &s.source_strengths,
            &s.tangential_velocity,
            &s.normal_velocity,
            &scene.conditions,
        );
        let forces = integrate_forces(&surface, &scene.conditions, p.reference, s.circulation);
        bodies.push(BodyResult {
            id: p.id.clone(),
            name: p.name.clone(),
            index: p.index,
            forces,
            surface,
            panel_count: p.panels.len(),
            circulation_mode: p.mode,
            kutta_applicable: p.kutta_applicable,
            trailing_edge: p.trailing_edge,
            polygon: p.polygon.points.clone(),
            vortex_strength: s.vortex_strength,
            net_source_outflow: s.net_source_outflow,
            notes: p.notes.clone(),
        });
    }
    let total = total_forces(&bodies.iter().map(|b| b.forces).collect::<Vec<_>>());

    // Lagally forces on elements. `field.elements` holds the visible elements
    // in scene order, so zip them back to their ids.
    let elements: Vec<ElementResult> = scene
        .elements
        .iter()
        .filter(|e| e.visible)
        .enumerate()
        .filter_map(|(i, e)| {
            let f = element_force(&field, i, scene.conditions.density)?;
            let (lift, drag) = scene.conditions.to_wind_axes(f.force);
            Some(ElementResult {
                id: e.id.clone(),
                name: e.name.clone(),
                position: e.element.position()?,
                force: f.force,
                lift,
                drag,
                external_velocity: f.external_velocity,
            })
        })
        .collect();
    timings.forces_ms = now() - t_forces;

    // 5. Warnings.
    let mut warnings = scene_warnings(scene, &field, &bodies);
    if let Some(d) = &panel_diag {
        for w in &d.warnings {
            warnings.push(Warning {
                // Serialise through serde so the code matches the camelCase
                // convention of the scene-level warnings.
                code: serde_json::to_value(w.code)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| format!("{:?}", w.code)),
                severity: WarningSeverity::Warning,
                message: w.message.clone(),
                object_id: w.body.and_then(|i| bodies.get(i).map(|b| b.id.clone())),
            });
        }
    }

    timings.total_ms = now() - t0;
    let status = if warnings
        .iter()
        .any(|w| w.severity == WarningSeverity::Warning)
    {
        SolveStatus::Warning
    } else {
        SolveStatus::Success
    };
    let solution = Solution {
        status,
        elements,
        total,
        diagnostics: Diagnostics {
            panel: panel_diag,
            timings,
            system_reused,
            element_count,
            body_count,
            total_circulation: field.total_circulation(),
            net_outflow: field.total_net_outflow(),
        },
        bodies,
        warnings,
        error: None,
        assumptions: ASSUMPTIONS.iter().map(|s| s.to_string()).collect(),
    };
    (solution, Some(field))
}

fn scene_warnings(scene: &Scene, field: &FlowField, bodies: &[BodyResult]) -> Vec<Warning> {
    let mut out = Vec::new();
    if scene.conditions.velocity == 0.0 {
        out.push(Warning {
            code: "freestreamZero".into(),
            severity: WarningSeverity::Warning,
            message: "Freestream velocity is zero: Cp and the force coefficients cannot be normalised. Dimensional pressure and forces are still reported.".into(),
            object_id: None,
        });
    }
    let net = field.total_net_outflow();
    let scale = scene.conditions.velocity.abs().max(1e-9);
    if net.abs() > 1e-9 * scale {
        out.push(Warning {
            code: "netOutflow".into(),
            severity: WarningSeverity::Info,
            message: format!(
                "The elements have a net outflow of {net:.3e} m²/s, so the flow cannot close at infinity and the stream function is multivalued. Streamlines are integrated directly and remain correct."
            ),
            object_id: None,
        });
    }
    for e in scene.elements.iter().filter(|e| e.visible) {
        if let Some(pos) = e.element.position() {
            if let Some(bi) = field.body_at(pos) {
                let body_name = bodies.get(bi).map(|b| b.name.as_str()).unwrap_or("a body");
                out.push(Warning {
                    code: "elementInsideBody".into(),
                    severity: WarningSeverity::Warning,
                    message: format!(
                        "'{}' lies inside '{}'. A singularity inside a solid body has no effect on the external flow in reality; here it still perturbs the panel solution.",
                        e.name, body_name
                    ),
                    object_id: Some(e.id.clone()),
                });
            }
        }
    }
    for b in bodies {
        if b.forces.reference.chord <= 1e-9 {
            out.push(Warning {
                code: "referenceChordZero".into(),
                severity: WarningSeverity::Warning,
                message: format!(
                    "'{}' has a zero reference chord; coefficients cannot be formed.",
                    b.name
                ),
                object_id: Some(b.id.clone()),
            });
        }
        if b.forces.lift_consistency > 0.01 {
            out.push(Warning {
                code: "liftConsistency".into(),
                severity: WarningSeverity::Warning,
                message: format!(
                    "'{}': pressure-integrated lift and Kutta–Joukowski lift differ by {:.1}%. Increase the panel count.",
                    b.name,
                    100.0 * b.forces.lift_consistency
                ),
                object_id: Some(b.id.clone()),
            });
        }
    }
    out
}

/// A live simulation: scene, cached influence matrix, and the latest result.
pub struct Simulation {
    scene: Scene,
    cache: Option<SystemCache>,
    field: Option<FlowField>,
    solution: Option<Solution>,
    now: NowFn,
}

impl Simulation {
    pub fn new(now: NowFn) -> Self {
        Self {
            scene: Scene::default(),
            cache: None,
            field: None,
            solution: None,
            now,
        }
    }

    pub fn with_scene(scene: Scene, now: NowFn) -> Self {
        let mut s = Self::new(now);
        s.set_scene(scene);
        s
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Replace the scene. The influence-matrix cache is kept; it is validated
    /// by geometry hash at the next solve.
    pub fn set_scene(&mut self, scene: Scene) {
        self.scene = scene;
    }

    pub fn solve(&mut self) -> &Solution {
        let (solution, field) = solve_scene(&self.scene, &mut self.cache, self.now);
        self.field = field;
        self.solution = Some(solution);
        self.solution.as_ref().expect("just set")
    }

    pub fn solution(&self) -> Option<&Solution> {
        self.solution.as_ref()
    }

    pub fn field(&self) -> Option<&FlowField> {
        self.field.as_ref()
    }

    pub fn sample_scalar(
        &self,
        field: FieldType,
        grid: &GridDefinition,
        opts: &FieldEvalOptions,
    ) -> Option<ScalarField> {
        self.field
            .as_ref()
            .map(|f| f.sample_scalar(field, grid, opts))
    }

    pub fn sample_vectors(
        &self,
        grid: &GridDefinition,
        opts: &FieldEvalOptions,
    ) -> Option<VectorField> {
        self.field.as_ref().map(|f| f.sample_vectors(grid, opts))
    }

    pub fn streamlines(&self, cfg: &StreamlineConfig, seeding: &SeedingConfig) -> Vec<Streamline> {
        self.field
            .as_ref()
            .map(|f| trace_set(f, cfg, seeding))
            .unwrap_or_default()
    }

    pub fn probe(&self, p: Vec2) -> Option<Probe> {
        let f = self.field.as_ref()?;
        let body_id = f.body_at(p).and_then(|i| {
            self.solution
                .as_ref()
                .and_then(|s| s.bodies.get(i))
                .map(|b| b.id.clone())
        });
        let velocity = f.velocity(p);
        let speed = velocity.norm();
        Some(Probe {
            position: p,
            velocity,
            speed,
            pressure: f.conditions.pressure_at_speed(speed),
            cp: f.conditions.cp_at_speed(speed),
            potential: f.potential(p),
            stream_function: f.stream_function(p),
            body_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{BodyGeometry, SceneBody, SceneElement};
    use aeroflow_flow_core::{Element, FlowConditions};

    fn conditions(u: f64, alpha_deg: f64) -> FlowConditions {
        FlowConditions {
            velocity: u,
            angle: alpha_deg.to_radians(),
            density: 1.0,
            pressure: 0.0,
        }
    }

    #[test]
    fn empty_scene_solves_to_the_freestream() {
        let mut sim = Simulation::with_scene(Scene::new("e", conditions(3.0, 0.0)), no_clock);
        let s = sim.solve().clone();
        assert_eq!(s.status, SolveStatus::Success);
        assert!(s.bodies.is_empty());
        let p = sim.probe(Vec2::new(1.0, 1.0)).unwrap();
        assert!((p.velocity.x - 3.0).abs() < 1e-12 && p.velocity.y.abs() < 1e-12);
        assert!(p.cp.unwrap().abs() < 1e-12);
    }

    #[test]
    fn invalid_conditions_produce_an_error_solution() {
        let mut scene = Scene::new("bad", conditions(1.0, 0.0));
        scene.conditions.density = 0.0;
        let mut sim = Simulation::with_scene(scene, no_clock);
        let s = sim.solve();
        assert_eq!(s.status, SolveStatus::Error);
        assert!(s.error.as_ref().unwrap().contains("density"));
        assert!(sim.field().is_none());
    }

    #[test]
    fn zero_freestream_is_a_warning_not_an_error() {
        let mut sim = Simulation::with_scene(Scene::new("z", conditions(0.0, 0.0)), no_clock);
        let s = sim.solve();
        assert_eq!(s.status, SolveStatus::Warning);
        assert!(s.warnings.iter().any(|w| w.code == "freestreamZero"));
    }

    #[test]
    fn invisible_bodies_and_elements_are_excluded() {
        let mut scene = Scene::new("v", conditions(1.0, 0.0));
        let mut b = SceneBody::new("b", "Cyl", BodyGeometry::Circle { radius: 1.0 });
        b.visible = false;
        scene.bodies.push(b);
        scene.elements.push(SceneElement {
            id: "s".into(),
            name: "S".into(),
            visible: false,
            locked: false,
            element: Element::Source {
                position: Vec2::ZERO,
                strength: 100.0,
            },
        });
        let mut sim = Simulation::with_scene(scene, no_clock);
        let s = sim.solve().clone();
        assert!(s.bodies.is_empty());
        assert_eq!(s.diagnostics.body_count, 0);
        assert_eq!(s.diagnostics.element_count, 0);
        let p = sim.probe(Vec2::new(0.1, 0.0)).unwrap();
        assert!(
            (p.velocity.x - 1.0).abs() < 1e-12,
            "hidden source leaked: {:?}",
            p.velocity
        );
    }

    #[test]
    fn influence_matrix_is_reused_when_only_elements_change() {
        let mut scene = Scene::new("c", conditions(1.0, 0.0));
        scene.bodies.push(SceneBody::new(
            "a",
            "Airfoil",
            BodyGeometry::Naca4 {
                code: "0012".into(),
                chord: 1.0,
            },
        ));
        scene.elements.push(SceneElement {
            id: "v".into(),
            name: "Vortex".into(),
            visible: true,
            locked: false,
            element: Element::Vortex {
                position: Vec2::new(-2.0, 0.5),
                circulation: 1.0,
            },
        });
        let mut sim = Simulation::with_scene(scene, no_clock);
        let first = sim.solve().clone();
        assert!(!first.diagnostics.system_reused);

        // Move the vortex: geometry unchanged → reuse.
        sim.scene_mut().element_mut("v").unwrap().element = Element::Vortex {
            position: Vec2::new(-2.0, -0.5),
            circulation: 1.0,
        };
        let second = sim.solve().clone();
        assert!(second.diagnostics.system_reused);
        assert!(
            (first.bodies[0].forces.lift - second.bodies[0].forces.lift).abs() > 1e-6,
            "moving the vortex must change the lift"
        );

        // Move the body: geometry changed → re-assemble.
        sim.scene_mut().body_mut("a").unwrap().position = Vec2::new(0.0, 0.1);
        let third = sim.solve();
        assert!(!third.diagnostics.system_reused);
    }

    #[test]
    fn changing_circulation_setting_invalidates_the_cache() {
        let mut scene = Scene::new("c", conditions(1.0, 0.0));
        scene.bodies.push(SceneBody::new(
            "c",
            "Cyl",
            BodyGeometry::Circle { radius: 1.0 },
        ));
        let mut sim = Simulation::with_scene(scene, no_clock);
        sim.solve();
        sim.scene_mut().body_mut("c").unwrap().circulation =
            crate::scene::CirculationSetting::Prescribed { circulation: -2.0 };
        let s = sim.solve();
        assert!(!s.diagnostics.system_reused);
        assert!((s.bodies[0].forces.circulation + 2.0).abs() < 1e-12);
    }

    #[test]
    fn element_inside_a_body_is_flagged_with_its_id() {
        let mut scene = Scene::new("i", conditions(1.0, 0.0));
        scene.bodies.push(SceneBody::new(
            "c",
            "Cylinder",
            BodyGeometry::Circle { radius: 1.0 },
        ));
        scene.elements.push(SceneElement {
            id: "src".into(),
            name: "Source 01".into(),
            visible: true,
            locked: false,
            element: Element::Source {
                position: Vec2::new(0.2, 0.1),
                strength: 1.0,
            },
        });
        let mut sim = Simulation::with_scene(scene, no_clock);
        let s = sim.solve();
        let w = s
            .warnings
            .iter()
            .find(|w| w.code == "elementInsideBody")
            .unwrap();
        assert_eq!(w.object_id.as_deref(), Some("src"));
        assert!(w.message.contains("Cylinder"));
    }

    #[test]
    fn bad_body_parameters_fail_the_solve_naming_the_body() {
        let mut scene = Scene::new("b", conditions(1.0, 0.0));
        scene.bodies.push(SceneBody::new(
            "x",
            "Broken",
            BodyGeometry::Circle { radius: 0.0 },
        ));
        let mut sim = Simulation::with_scene(scene, no_clock);
        let s = sim.solve();
        assert_eq!(s.status, SolveStatus::Error);
        assert!(s.error.as_ref().unwrap().contains("Broken"));
    }

    #[test]
    fn probe_identifies_the_containing_body() {
        let mut scene = Scene::new("p", conditions(1.0, 0.0));
        scene.bodies.push(SceneBody::new(
            "cyl",
            "Cyl",
            BodyGeometry::Circle { radius: 1.0 },
        ));
        let mut sim = Simulation::with_scene(scene, no_clock);
        sim.solve();
        assert_eq!(
            sim.probe(Vec2::ZERO).unwrap().body_id.as_deref(),
            Some("cyl")
        );
        assert!(sim.probe(Vec2::new(3.0, 0.0)).unwrap().body_id.is_none());
    }

    #[test]
    fn assumptions_travel_with_every_solution() {
        let mut sim = Simulation::with_scene(Scene::new("a", conditions(1.0, 0.0)), no_clock);
        let s = sim.solve();
        assert!(s.assumptions.iter().any(|a| a.contains("Inviscid")));
        assert!(s.assumptions.iter().any(|a| a.contains("d'Alembert")));
    }

    #[test]
    fn solution_serialises_to_json() {
        let mut scene = Scene::new("j", conditions(1.0, 5.0));
        scene.bodies.push(SceneBody::new(
            "a",
            "Airfoil",
            BodyGeometry::Naca4 {
                code: "0012".into(),
                chord: 1.0,
            },
        ));
        let mut sim = Simulation::with_scene(scene, no_clock);
        let json = serde_json::to_string(sim.solve()).unwrap();
        assert!(json.contains("\"status\":\"success\"") || json.contains("\"status\":\"warning\""));
        assert!(json.contains("\"cl\":"));
        assert!(json.contains("\"circulationMode\":{\"mode\":\"kutta\"}"));
    }
}
