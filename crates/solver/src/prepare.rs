//! Turn a [`SceneBody`] into solver-ready geometry (PRD §10.2, §11).
//!
//! ```text
//! base contour → counter-clockwise → re-panelise → find trailing edge →
//! rotate to start there → world transform → panels → resolve circulation
//! ```
//!
//! The panel distribution is applied *before* the world transform so that it
//! is a property of the body, not of where the body happens to sit.

use crate::scene::{BodyGeometry, CirculationSetting, SceneBody};
use aeroflow_flow_core::ReferenceValues;
use aeroflow_geometry::{
    analyse_trailing_edge, repanel, shapes, CornerInfo, Panel, PanelDistribution, Panelisation,
    Polygon, RepanelConfig,
};
use aeroflow_panel_method::CirculationMode;

#[derive(Debug, Clone)]
pub struct PreparedBody {
    pub index: usize,
    pub id: String,
    pub name: String,
    /// World-space contour, counter-clockwise, starting at the trailing edge
    /// when one exists.
    pub polygon: Polygon,
    pub panels: Vec<Panel>,
    pub mode: CirculationMode,
    pub kutta_applicable: bool,
    pub trailing_edge: Option<CornerInfo>,
    pub reference: ReferenceValues,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrepareError {
    /// Fewer than three usable points.
    Degenerate {
        body: String,
    },
    InvalidParameter {
        body: String,
        message: String,
    },
}

impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrepareError::Degenerate { body } => {
                write!(
                    f,
                    "Body '{body}' has a degenerate contour (fewer than three distinct points)."
                )
            }
            PrepareError::InvalidParameter { body, message } => {
                write!(f, "Body '{body}': {message}")
            }
        }
    }
}

impl std::error::Error for PrepareError {}

/// Base contour in body-local coordinates.
fn base_polygon(body: &SceneBody) -> Result<(Polygon, bool), PrepareError> {
    let n = body.panels.count.max(MIN_PANELS);
    let invalid = |message: String| PrepareError::InvalidParameter {
        body: body.name.clone(),
        message,
    };
    // The boolean says whether the generator's own spacing is already ideal,
    // in which case `Auto` leaves it alone.
    match &body.geometry {
        BodyGeometry::Points { points } => Ok((Polygon::new(points.clone()), false)),
        BodyGeometry::Naca4 { code, chord } => {
            if !(*chord > 0.0 && chord.is_finite()) {
                return Err(invalid(format!("chord must be positive, got {chord}")));
            }
            let mut n4 = shapes::Naca4::parse(code)
                .ok_or_else(|| invalid(format!("'{code}' is not a valid NACA 4-digit code")))?;
            n4.chord = *chord;
            Ok((n4.generate(n), true))
        }
        BodyGeometry::Circle { radius } => {
            if !(*radius > 0.0 && radius.is_finite()) {
                return Err(invalid(format!("radius must be positive, got {radius}")));
            }
            Ok((shapes::circle(*radius, n), true))
        }
        BodyGeometry::Ellipse {
            semi_axis_x,
            semi_axis_y,
        } => {
            if !(positive_finite(*semi_axis_x) && positive_finite(*semi_axis_y)) {
                return Err(invalid("both semi-axes must be positive".into()));
            }
            Ok((shapes::ellipse(*semi_axis_x, *semi_axis_y, n), true))
        }
        BodyGeometry::Joukowski { thickness, camber } => {
            if !thickness.is_finite() || !camber.is_finite() || *thickness < 0.0 {
                return Err(invalid("thickness must be non-negative and finite".into()));
            }
            let j = shapes::Joukowski {
                dx: *thickness,
                dy: *camber,
                c: 1.0,
            };
            // Normalise to unit chord so the Joukowski body is comparable to a
            // NACA section of the same nominal size.
            let raw = j.generate(n);
            let chord = raw.diameter().map(|d| d.2).unwrap_or(4.0);
            let scaled = Polygon::new(raw.points.iter().map(|p| *p / chord).collect());
            Ok((scaled, true))
        }
    }
}

/// `x > 0` and finite — NaN-safe, unlike `!(x <= 0.0)`-style rewrites.
fn positive_finite(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Smallest panel count the generators produce; requests below it are raised.
pub const MIN_PANELS: usize = 16;

pub fn prepare_body(body: &SceneBody, index: usize) -> Result<PreparedBody, PrepareError> {
    let (mut poly, native_spacing) = base_polygon(body)?;
    if poly.len() < 3 {
        return Err(PrepareError::Degenerate {
            body: body.name.clone(),
        });
    }
    let mut notes = Vec::new();
    if body.panels.count < MIN_PANELS && !matches!(body.geometry, BodyGeometry::Points { .. }) {
        notes.push(format!(
            "Panel count raised from {} to the minimum of {MIN_PANELS}.",
            body.panels.count
        ));
    }
    if poly.make_counter_clockwise() {
        notes.push("Reversed to counter-clockwise order.".to_string());
    }

    // Re-panelise, unless the generator's spacing is already purpose-built
    // and the user left the distribution on Auto.
    let distribution = body.panels.distribution;
    let skip = native_spacing && distribution == PanelDistribution::Auto;
    if !skip && distribution != PanelDistribution::AsImported {
        let before = poly.len();
        poly = repanel(
            &poly,
            &RepanelConfig {
                distribution,
                count: body.panels.count,
                ..RepanelConfig::default()
            },
        );
        if poly.len() != before {
            notes.push(format!(
                "Re-panelised from {before} to {} panels.",
                poly.len()
            ));
        }
    }

    let te = analyse_trailing_edge(&poly);
    if te.rotated_by > 0 {
        notes.push(format!(
            "Contour rotated by {} points to start at the trailing edge.",
            te.rotated_by
        ));
    }
    let local = te.polygon;

    // World transform.
    let world = local.transformed(body.position, body.rotation, body.scale);
    let pz = Panelisation::from_polygon(&world).ok_or_else(|| PrepareError::Degenerate {
        body: body.name.clone(),
    })?;

    let trailing_edge = te.trailing_edge.map(|c| CornerInfo {
        position: world.points[0],
        ..c
    });

    let mode = match body.circulation {
        CirculationSetting::Auto => {
            if te.kutta_applicable {
                CirculationMode::Kutta
            } else {
                CirculationMode::None
            }
        }
        CirculationSetting::Kutta => CirculationMode::Kutta,
        CirculationSetting::None => CirculationMode::None,
        CirculationSetting::Prescribed { circulation } => {
            CirculationMode::Prescribed { circulation }
        }
    };

    let reference = reference_values(&world, trailing_edge.as_ref(), body);

    Ok(PreparedBody {
        index,
        id: body.id.clone(),
        name: body.name.clone(),
        polygon: world,
        panels: pz.panels,
        mode,
        kutta_applicable: te.kutta_applicable,
        trailing_edge,
        reference,
        notes,
    })
}

/// Chord and moment reference in world space.
///
/// With a detected trailing edge the leading edge is the vertex furthest from
/// it — robust to any rotation of the body. Without one, fall back to the
/// contour's diameter.
fn reference_values(world: &Polygon, te: Option<&CornerInfo>, body: &SceneBody) -> ReferenceValues {
    let (le, te_pt, chord) = match te {
        Some(c) => {
            let te_pt = c.position;
            let (le, d) = world.points.iter().map(|p| (*p, p.distance(te_pt))).fold(
                (te_pt, 0.0),
                |acc, cur| if cur.1 > acc.1 { cur } else { acc },
            );
            (le, te_pt, d)
        }
        None => {
            let (i, j, d) = world.diameter().unwrap_or((0, 0, 1.0));
            let (a, b) = (world.points[i], world.points[j]);
            // Upstream-most (smaller x) end as the leading edge.
            if a.x <= b.x {
                (a, b, d)
            } else {
                (b, a, d)
            }
        }
    };
    let chord = body
        .reference
        .chord
        .filter(|c| *c > 0.0)
        .unwrap_or(chord.max(1e-12));
    let point = match body.reference.point {
        // Override is given in body-local coordinates; map it to the world.
        Some(local) => local.rotate(body.rotation) * body.scale + body.position,
        None => le.lerp(te_pt, 0.25),
    };
    ReferenceValues {
        chord,
        area: chord,
        point,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeroflow_geometry::{Vec2, Winding};
    use std::f64::consts::PI;

    fn naca(code: &str) -> SceneBody {
        SceneBody::new(
            "b",
            "Airfoil",
            BodyGeometry::Naca4 {
                code: code.into(),
                chord: 1.0,
            },
        )
    }

    #[test]
    fn naca_prepares_with_kutta_and_quarter_chord_reference() {
        let p = prepare_body(&naca("0012"), 0).unwrap();
        assert_eq!(p.mode, CirculationMode::Kutta);
        assert!(p.kutta_applicable);
        assert_eq!(p.polygon.winding(), Winding::CounterClockwise);
        assert!((p.reference.chord - 1.0).abs() < 1e-6);
        assert!((p.reference.point.x - 0.25).abs() < 1e-3);
        assert!((p.polygon.points[0].x - 1.0).abs() < 1e-9, "starts at TE");
    }

    #[test]
    fn circle_prepares_without_kutta() {
        let b = SceneBody::new("c", "Cylinder", BodyGeometry::Circle { radius: 2.0 });
        let p = prepare_body(&b, 0).unwrap();
        assert_eq!(p.mode, CirculationMode::None);
        assert!(!p.kutta_applicable);
        assert!((p.reference.chord - 4.0).abs() < 1e-3, "chord = diameter");
        assert_eq!(p.panels.len(), 120);
    }

    #[test]
    fn explicit_circulation_settings_are_honoured() {
        let mut b = SceneBody::new("c", "Cylinder", BodyGeometry::Circle { radius: 1.0 });
        b.circulation = CirculationSetting::Prescribed { circulation: -3.0 };
        assert_eq!(
            prepare_body(&b, 0).unwrap().mode,
            CirculationMode::Prescribed { circulation: -3.0 }
        );
        b.circulation = CirculationSetting::Kutta;
        assert_eq!(prepare_body(&b, 0).unwrap().mode, CirculationMode::Kutta);
        let mut a = naca("0012");
        a.circulation = CirculationSetting::None;
        assert_eq!(prepare_body(&a, 0).unwrap().mode, CirculationMode::None);
    }

    #[test]
    fn world_transform_is_applied_after_panelisation() {
        let mut b = naca("0012");
        b.position = Vec2::new(3.0, -1.0);
        b.rotation = PI / 2.0;
        b.scale = 2.0;
        let p = prepare_body(&b, 0).unwrap();
        // TE at local (1,0) → scaled (2,0) → rotated (0,2) → translated (3,1).
        let te = p.polygon.points[0];
        assert!(
            (te.x - 3.0).abs() < 1e-9 && (te.y - 1.0).abs() < 1e-9,
            "TE at {te:?}"
        );
        assert!((p.reference.chord - 2.0).abs() < 1e-6);
        // Quarter chord sits a quarter of the way from LE (3,-1) to TE (3,1).
        assert!(
            (p.reference.point.y - (-1.0 + 0.5)).abs() < 1e-3,
            "{:?}",
            p.reference.point
        );
        assert_eq!(p.polygon.winding(), Winding::CounterClockwise);
    }

    #[test]
    fn reference_overrides_are_applied_in_body_local_coordinates() {
        let mut b = naca("0012");
        b.position = Vec2::new(10.0, 0.0);
        b.reference = crate::scene::ReferenceOverride {
            chord: Some(0.5),
            point: Some(Vec2::new(0.5, 0.0)),
        };
        let p = prepare_body(&b, 0).unwrap();
        assert_eq!(p.reference.chord, 0.5);
        assert!((p.reference.point.x - 10.5).abs() < 1e-12);
    }

    #[test]
    fn imported_points_are_repanelised_to_the_requested_count() {
        let pts = shapes::Naca4::default().generate(300).points;
        let mut b = SceneBody::new("i", "Imported", BodyGeometry::Points { points: pts });
        b.panels.count = 90;
        let p = prepare_body(&b, 0).unwrap();
        assert_eq!(p.panels.len(), 90);
        assert!(p.notes.iter().any(|n| n.contains("Re-panelised")));
        assert!(p.kutta_applicable);
    }

    #[test]
    fn as_imported_keeps_the_points_untouched() {
        let pts = shapes::Naca4::default().generate(64).points;
        let n = pts.len();
        let mut b = SceneBody::new("i", "Imported", BodyGeometry::Points { points: pts });
        b.panels.distribution = PanelDistribution::AsImported;
        b.panels.count = 20;
        assert_eq!(prepare_body(&b, 0).unwrap().panels.len(), n);
    }

    #[test]
    fn clockwise_imported_points_are_reversed() {
        let mut pts = shapes::Naca4::default().generate(64).points;
        pts.reverse();
        let b = SceneBody::new("i", "Imported", BodyGeometry::Points { points: pts });
        let p = prepare_body(&b, 0).unwrap();
        assert_eq!(p.polygon.winding(), Winding::CounterClockwise);
        assert!(p.notes.iter().any(|n| n.contains("Reversed")));
    }

    #[test]
    fn a_panel_count_below_the_minimum_is_raised_and_noted() {
        let mut b = SceneBody::new("c", "Cyl", BodyGeometry::Circle { radius: 1.0 });
        b.panels.count = 10;
        let p = prepare_body(&b, 0).unwrap();
        assert_eq!(p.panels.len(), MIN_PANELS);
        assert!(
            p.notes.iter().any(|n| n.contains("raised")),
            "{:?}",
            p.notes
        );
    }

    #[test]
    fn nan_dimensions_are_rejected() {
        let b = SceneBody::new(
            "e",
            "E",
            BodyGeometry::Ellipse {
                semi_axis_x: f64::NAN,
                semi_axis_y: 1.0,
            },
        );
        assert!(prepare_body(&b, 0).is_err());
        let b = SceneBody::new("c", "C", BodyGeometry::Circle { radius: f64::NAN });
        assert!(prepare_body(&b, 0).is_err());
    }

    #[test]
    fn invalid_parameters_name_the_body() {
        let b = SceneBody::new("c", "My Cylinder", BodyGeometry::Circle { radius: -1.0 });
        let e = prepare_body(&b, 0).unwrap_err();
        assert!(e.to_string().contains("My Cylinder"));
        let b = naca("99");
        assert!(prepare_body(&b, 0).is_err());
        let b = SceneBody::new(
            "d",
            "Dot",
            BodyGeometry::Points {
                points: vec![Vec2::ZERO; 2],
            },
        );
        assert!(matches!(
            prepare_body(&b, 0),
            Err(PrepareError::Degenerate { .. })
        ));
    }

    #[test]
    fn joukowski_is_normalised_to_unit_chord() {
        let b = SceneBody::new(
            "j",
            "Jouk",
            BodyGeometry::Joukowski {
                thickness: 0.1,
                camber: 0.05,
            },
        );
        let p = prepare_body(&b, 0).unwrap();
        assert!(
            (p.reference.chord - 1.0).abs() < 1e-6,
            "chord {}",
            p.reference.chord
        );
        assert!(p.kutta_applicable);
    }
}
