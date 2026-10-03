//! The user-facing simulation definition — the `.aeroflow.json` format
//! (PRD §6, §51).
//!
//! A [`Scene`] is pure configuration: it holds *what* the user built, never a
//! numerical result (PRD §86.4, scene state ≠ solution state). It is the unit
//! of persistence and of undo/redo.

use aeroflow_flow_core::{Element, FlowConditions};
use aeroflow_geometry::{repanel::PanelDistribution, Vec2};
use serde::{Deserialize, Serialize};

/// Current on-disk format version.
pub const SCENE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub version: u32,
    #[serde(default)]
    pub name: String,
    pub conditions: FlowConditions,
    #[serde(default)]
    pub elements: Vec<SceneElement>,
    #[serde(default)]
    pub bodies: Vec<SceneBody>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            version: SCENE_FORMAT_VERSION,
            name: "Untitled simulation".to_string(),
            conditions: FlowConditions::default(),
            elements: Vec::new(),
            bodies: Vec::new(),
        }
    }
}

impl Scene {
    pub fn new(name: &str, conditions: FlowConditions) -> Self {
        Self {
            name: name.to_string(),
            conditions,
            ..Default::default()
        }
    }

    pub fn element(&self, id: &str) -> Option<&SceneElement> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn element_mut(&mut self, id: &str) -> Option<&mut SceneElement> {
        self.elements.iter_mut().find(|e| e.id == id)
    }

    pub fn body(&self, id: &str) -> Option<&SceneBody> {
        self.bodies.iter().find(|b| b.id == id)
    }

    pub fn body_mut(&mut self, id: &str) -> Option<&mut SceneBody> {
        self.bodies.iter_mut().find(|b| b.id == id)
    }

    /// Structural validation of the configuration itself — not of the
    /// geometry, which happens during preparation.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.version > SCENE_FORMAT_VERSION {
            problems.push(format!(
                "This file uses format version {} but this build understands up to {}.",
                self.version, SCENE_FORMAT_VERSION
            ));
        }
        if !self.conditions.is_valid() {
            problems.push(
                "Flow conditions are invalid: density must be positive and all values finite."
                    .into(),
            );
        }
        let mut ids: Vec<&str> = self
            .elements
            .iter()
            .map(|e| e.id.as_str())
            .chain(self.bodies.iter().map(|b| b.id.as_str()))
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != total {
            problems.push("Object ids are not unique.".into());
        }
        for e in &self.elements {
            if !e.element.is_finite() {
                problems.push(format!("Element '{}' has a non-finite parameter.", e.name));
            }
        }
        for b in &self.bodies {
            if !(b.scale.is_finite() && b.scale > 0.0) {
                problems.push(format!(
                    "Body '{}' has an invalid scale {}.",
                    b.name, b.scale
                ));
            }
            if !b.position.is_finite() || !b.rotation.is_finite() {
                problems.push(format!("Body '{}' has a non-finite transform.", b.name));
            }
        }
        problems
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneElement {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    pub element: Element,
}

fn default_true() -> bool {
    true
}

/// Where a body's base contour comes from. Parametric sources let the bundled
/// examples open without any file (PRD §57) and keep saved scenes small.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BodyGeometry {
    /// Imported points in body-local coordinates, already cleaned and
    /// counter-clockwise.
    Points {
        points: Vec<Vec2>,
    },
    Naca4 {
        code: String,
        #[serde(default = "default_chord")]
        chord: f64,
    },
    Circle {
        radius: f64,
    },
    Ellipse {
        semi_axis_x: f64,
        semi_axis_y: f64,
    },
    Joukowski {
        thickness: f64,
        camber: f64,
    },
}

fn default_chord() -> f64 {
    1.0
}

/// How the body's circulation is decided (PRD §12.1, DEC-001).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
#[derive(Default)]
pub enum CirculationSetting {
    /// Kutta if a sharp trailing edge is detected, otherwise zero.
    #[default]
    Auto,
    Kutta,
    None,
    /// Fixed `Γ` [m²/s], counter-clockwise positive.
    Prescribed {
        circulation: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSettings {
    pub count: usize,
    pub distribution: PanelDistribution,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            count: 120,
            distribution: PanelDistribution::Auto,
        }
    }
}

/// Optional overrides for the non-dimensionalising reference values (PRD §26,
/// §27: "must be configurable").
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceOverride {
    pub chord: Option<f64>,
    /// Moment reference point in **body-local** coordinates.
    pub point: Option<Vec2>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneBody {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    pub geometry: BodyGeometry,
    #[serde(default)]
    pub position: Vec2,
    /// Rotation about the body-local origin [rad], counter-clockwise.
    #[serde(default)]
    pub rotation: f64,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub circulation: CirculationSetting,
    #[serde(default)]
    pub panels: PanelSettings,
    #[serde(default)]
    pub reference: ReferenceOverride,
    /// Name recorded in the imported file, if any.
    #[serde(default)]
    pub source_name: Option<String>,
}

fn default_scale() -> f64 {
    1.0
}

impl SceneBody {
    pub fn new(id: &str, name: &str, geometry: BodyGeometry) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            visible: true,
            locked: false,
            geometry,
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: 1.0,
            circulation: CirculationSetting::Auto,
            panels: PanelSettings::default(),
            reference: ReferenceOverride::default(),
            source_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_round_trips_through_json() {
        let mut s = Scene::new("test", FlowConditions::default());
        s.elements.push(SceneElement {
            id: "e1".into(),
            name: "Source 01".into(),
            visible: true,
            locked: false,
            element: Element::Source {
                position: Vec2::new(1.0, 2.0),
                strength: 3.0,
            },
        });
        s.bodies.push(SceneBody::new(
            "b1",
            "Airfoil 01",
            BodyGeometry::Naca4 {
                code: "2412".into(),
                chord: 1.0,
            },
        ));
        let json = serde_json::to_string_pretty(&s).unwrap();
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"type\": \"source\""));
        let back: Scene = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn missing_optional_fields_take_defaults() {
        let json = r#"{
            "version": 1,
            "conditions": {"velocity": 10, "angle": 0, "density": 1.225, "pressure": 101325},
            "bodies": [{"id":"b","name":"B","geometry":{"kind":"circle","radius":1}}]
        }"#;
        let s: Scene = serde_json::from_str(json).unwrap();
        assert!(s.bodies[0].visible);
        assert_eq!(s.bodies[0].scale, 1.0);
        assert_eq!(s.bodies[0].circulation, CirculationSetting::Auto);
        assert_eq!(s.bodies[0].panels.count, 120);
        assert!(s.elements.is_empty());
    }

    #[test]
    fn validation_catches_duplicate_ids_and_bad_conditions() {
        let mut s = Scene::default();
        s.bodies.push(SceneBody::new(
            "x",
            "A",
            BodyGeometry::Circle { radius: 1.0 },
        ));
        s.bodies.push(SceneBody::new(
            "x",
            "B",
            BodyGeometry::Circle { radius: 1.0 },
        ));
        s.conditions.density = -1.0;
        let p = s.validate();
        assert!(p.iter().any(|m| m.contains("unique")));
        assert!(p.iter().any(|m| m.contains("density")));
    }

    #[test]
    fn newer_format_version_is_flagged() {
        let s = Scene {
            version: SCENE_FORMAT_VERSION + 1,
            ..Default::default()
        };
        assert!(s.validate().iter().any(|m| m.contains("format version")));
    }
}
