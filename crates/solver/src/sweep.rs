//! Parameter sweeps (PRD §29, §60).
//!
//! A [`SweepRun`] is stepped one point at a time so the Web Worker can check
//! for cancellation between points instead of blocking until the whole polar
//! is done. Sweeps that leave the geometry untouched (angle of attack, speed,
//! element strength) reuse one influence-matrix factorisation for every point.

use crate::scene::{CirculationSetting, Scene};
use crate::simulation::{solve_scene, NowFn, SolveStatus, SystemCache};
use aeroflow_flow_core::Element;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "parameter",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SweepParameter {
    /// Values in radians.
    AngleOfAttack,
    FreestreamSpeed,
    /// Prescribed circulation of one body [m²/s].
    BodyCirculation {
        body_id: String,
    },
    /// Primary strength of one element: `Λ`, `Γ`, `κ` or `U` by kind.
    ElementStrength {
        element_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepConfig {
    #[serde(flatten)]
    pub parameter: SweepParameter,
    pub start: f64,
    pub end: f64,
    pub steps: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepBodyPoint {
    pub id: String,
    pub name: String,
    pub lift: f64,
    pub drag: f64,
    pub moment: f64,
    pub cl: Option<f64>,
    pub cd: Option<f64>,
    pub cm: Option<f64>,
    pub circulation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepPoint {
    pub index: usize,
    pub value: f64,
    pub status: SolveStatus,
    pub bodies: Vec<SweepBodyPoint>,
    pub total_lift: f64,
    pub total_drag: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepResult {
    pub config: SweepConfig,
    pub points: Vec<SweepPoint>,
    /// False when the run was cancelled before every point was computed.
    pub completed: bool,
}

pub struct SweepRun {
    config: SweepConfig,
    base: Scene,
    cache: Option<SystemCache>,
    index: usize,
    points: Vec<SweepPoint>,
    now: NowFn,
}

impl SweepRun {
    pub fn new(scene: &Scene, config: SweepConfig, now: NowFn) -> Result<Self, String> {
        if config.steps == 0 {
            return Err("A sweep needs at least one step.".into());
        }
        if !(config.start.is_finite() && config.end.is_finite()) {
            return Err("Sweep bounds must be finite.".into());
        }
        match &config.parameter {
            SweepParameter::BodyCirculation { body_id } => {
                if scene.body(body_id).is_none() {
                    return Err(format!("No body with id '{body_id}'."));
                }
            }
            SweepParameter::ElementStrength { element_id } => {
                if scene.element(element_id).is_none() {
                    return Err(format!("No element with id '{element_id}'."));
                }
            }
            _ => {}
        }
        Ok(Self {
            config,
            base: scene.clone(),
            cache: None,
            index: 0,
            points: Vec::new(),
            now,
        })
    }

    pub fn total(&self) -> usize {
        self.config.steps
    }

    pub fn remaining(&self) -> usize {
        self.config.steps - self.index
    }

    pub fn is_done(&self) -> bool {
        self.index >= self.config.steps
    }

    pub fn value_at(&self, i: usize) -> f64 {
        if self.config.steps <= 1 {
            self.config.start
        } else {
            self.config.start
                + (self.config.end - self.config.start) * i as f64 / (self.config.steps - 1) as f64
        }
    }

    fn scene_at(&self, value: f64) -> Scene {
        let mut s = self.base.clone();
        match &self.config.parameter {
            SweepParameter::AngleOfAttack => s.conditions.angle = value,
            SweepParameter::FreestreamSpeed => s.conditions.velocity = value,
            SweepParameter::BodyCirculation { body_id } => {
                if let Some(b) = s.body_mut(body_id) {
                    b.circulation = CirculationSetting::Prescribed { circulation: value };
                }
            }
            SweepParameter::ElementStrength { element_id } => {
                if let Some(e) = s.element_mut(element_id) {
                    e.element = match e.element {
                        Element::UniformFlow { direction, .. } => Element::UniformFlow {
                            velocity: value,
                            direction,
                        },
                        Element::Source { position, .. } => Element::Source {
                            position,
                            strength: value,
                        },
                        Element::Sink { position, .. } => Element::Sink {
                            position,
                            strength: value,
                        },
                        Element::Vortex { position, .. } => Element::Vortex {
                            position,
                            circulation: value,
                        },
                        Element::Doublet {
                            position,
                            orientation,
                            ..
                        } => Element::Doublet {
                            position,
                            strength: value,
                            orientation,
                        },
                    };
                }
            }
        }
        s
    }

    /// Compute the next point. Returns `None` when the sweep is complete.
    pub fn step(&mut self) -> Option<SweepPoint> {
        if self.is_done() {
            return None;
        }
        let i = self.index;
        let value = self.value_at(i);
        let scene = self.scene_at(value);
        let (sol, _) = solve_scene(&scene, &mut self.cache, self.now);
        let point = SweepPoint {
            index: i,
            value,
            status: sol.status,
            bodies: sol
                .bodies
                .iter()
                .map(|b| SweepBodyPoint {
                    id: b.id.clone(),
                    name: b.name.clone(),
                    lift: b.forces.lift,
                    drag: b.forces.drag,
                    moment: b.forces.moment,
                    cl: b.forces.cl,
                    cd: b.forces.cd,
                    cm: b.forces.cm,
                    circulation: b.forces.circulation,
                })
                .collect(),
            total_lift: sol.total.lift,
            total_drag: sol.total.drag,
            error: sol.error,
        };
        self.points.push(point.clone());
        self.index += 1;
        Some(point)
    }

    pub fn run_all(mut self) -> SweepResult {
        while self.step().is_some() {}
        self.result()
    }

    pub fn result(&self) -> SweepResult {
        SweepResult {
            config: self.config.clone(),
            points: self.points.clone(),
            completed: self.is_done(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{BodyGeometry, SceneBody, SceneElement};
    use crate::simulation::no_clock;
    use aeroflow_flow_core::FlowConditions;
    use aeroflow_geometry::Vec2;

    fn airfoil_scene() -> Scene {
        let mut s = Scene::new(
            "sweep",
            FlowConditions {
                velocity: 1.0,
                angle: 0.0,
                density: 1.0,
                pressure: 0.0,
            },
        );
        let mut b = SceneBody::new(
            "a",
            "NACA 0012",
            BodyGeometry::Naca4 {
                code: "0012".into(),
                chord: 1.0,
            },
        );
        b.panels.count = 80;
        s.bodies.push(b);
        s
    }

    #[test]
    fn angle_sweep_is_monotonic_and_antisymmetric_for_a_symmetric_section() {
        let cfg = SweepConfig {
            parameter: SweepParameter::AngleOfAttack,
            start: (-6.0f64).to_radians(),
            end: 6.0f64.to_radians(),
            steps: 7,
        };
        let run = SweepRun::new(&airfoil_scene(), cfg, no_clock).unwrap();
        let r = run.run_all();
        assert!(r.completed);
        assert_eq!(r.points.len(), 7);
        let cls: Vec<f64> = r.points.iter().map(|p| p.bodies[0].cl.unwrap()).collect();
        for w in cls.windows(2) {
            assert!(w[1] > w[0], "CL must increase with α: {cls:?}");
        }
        // Antisymmetry about α = 0.
        for i in 0..3 {
            assert!((cls[i] + cls[6 - i]).abs() < 1e-9, "{cls:?}");
        }
        assert!(cls[3].abs() < 1e-9);
    }

    #[test]
    fn stepping_allows_cancellation_midway() {
        let cfg = SweepConfig {
            parameter: SweepParameter::AngleOfAttack,
            start: 0.0,
            end: 0.2,
            steps: 5,
        };
        let mut run = SweepRun::new(&airfoil_scene(), cfg, no_clock).unwrap();
        assert_eq!(run.remaining(), 5);
        run.step();
        run.step();
        assert_eq!(run.remaining(), 3);
        let partial = run.result();
        assert!(!partial.completed);
        assert_eq!(partial.points.len(), 2);
    }

    #[test]
    fn single_step_sweep_evaluates_the_start_value() {
        let cfg = SweepConfig {
            parameter: SweepParameter::FreestreamSpeed,
            start: 7.0,
            end: 99.0,
            steps: 1,
        };
        let r = SweepRun::new(&airfoil_scene(), cfg, no_clock)
            .unwrap()
            .run_all();
        assert_eq!(r.points.len(), 1);
        assert_eq!(r.points[0].value, 7.0);
    }

    #[test]
    fn circulation_sweep_follows_kutta_joukowski() {
        let mut s = Scene::new(
            "kj",
            FlowConditions {
                velocity: 2.0,
                angle: 0.0,
                density: 1.5,
                pressure: 0.0,
            },
        );
        s.bodies.push(SceneBody::new(
            "c",
            "Cyl",
            BodyGeometry::Circle { radius: 1.0 },
        ));
        let cfg = SweepConfig {
            parameter: SweepParameter::BodyCirculation {
                body_id: "c".into(),
            },
            start: -4.0,
            end: 4.0,
            steps: 5,
        };
        let r = SweepRun::new(&s, cfg, no_clock).unwrap().run_all();
        for p in &r.points {
            let expected = -1.5 * 2.0 * p.value; // L = −ρUΓ
            let got = p.bodies[0].lift;
            assert!(
                (got - expected).abs() <= 0.01 * expected.abs().max(1e-9) + 1e-9,
                "Γ = {}: lift {got} vs KJ {expected}",
                p.value
            );
        }
    }

    #[test]
    fn element_strength_sweep_changes_the_right_element() {
        let mut s = airfoil_scene();
        s.elements.push(SceneElement {
            id: "v".into(),
            name: "V".into(),
            visible: true,
            locked: false,
            element: Element::Vortex {
                position: Vec2::new(-1.0, 0.5),
                circulation: 0.0,
            },
        });
        let cfg = SweepConfig {
            parameter: SweepParameter::ElementStrength {
                element_id: "v".into(),
            },
            start: -1.0,
            end: 1.0,
            steps: 3,
        };
        let r = SweepRun::new(&s, cfg, no_clock).unwrap().run_all();
        let lifts: Vec<f64> = r.points.iter().map(|p| p.bodies[0].lift).collect();
        assert!(lifts[0] != lifts[1] && lifts[1] != lifts[2], "{lifts:?}");
    }

    #[test]
    fn unknown_ids_and_bad_configs_are_rejected_up_front() {
        let s = airfoil_scene();
        assert!(SweepRun::new(
            &s,
            SweepConfig {
                parameter: SweepParameter::BodyCirculation {
                    body_id: "nope".into()
                },
                start: 0.0,
                end: 1.0,
                steps: 3
            },
            no_clock
        )
        .is_err());
        assert!(SweepRun::new(
            &s,
            SweepConfig {
                parameter: SweepParameter::AngleOfAttack,
                start: 0.0,
                end: 1.0,
                steps: 0
            },
            no_clock
        )
        .is_err());
        assert!(SweepRun::new(
            &s,
            SweepConfig {
                parameter: SweepParameter::AngleOfAttack,
                start: f64::NAN,
                end: 1.0,
                steps: 2
            },
            no_clock
        )
        .is_err());
    }

    #[test]
    fn sweep_config_serialises_with_a_flat_parameter_tag() {
        let cfg = SweepConfig {
            parameter: SweepParameter::BodyCirculation {
                body_id: "c".into(),
            },
            start: 0.0,
            end: 1.0,
            steps: 2,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"parameter\":\"bodyCirculation\""), "{json}");
        assert!(json.contains("\"bodyId\":\"c\""));
        let back: SweepConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }
}
