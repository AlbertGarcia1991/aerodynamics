//! # aeroflow-geometry
//!
//! Geometry primitives, coordinate-file parsing, validation and panelisation
//! for the AeroFlow 2D potential-flow solver.
//!
//! ## Conventions (PRD §14.1)
//!
//! * `+x` right, `+y` up, positive angles counter-clockwise.
//! * Body contours are stored **counter-clockwise** with no repeated closing
//!   point. Under that winding, each edge's **outward** normal is its tangent
//!   rotated −90° ([`Vec2::perp_right`]), which is what the panel solver uses.
//! * Lengths are metres.
//!
//! This crate has no dependency on the solver and is fully testable natively,
//! which satisfies the PRD's requirement that geometry preprocessing be
//! independently verifiable (PRD §86.3).

// Dense numerical kernels read most clearly as explicit index loops over
// matrices and panel arrays; the iterator rewrites clippy suggests obscure them.
#![allow(clippy::needless_range_loop)]

pub mod panel;
pub mod parse;
pub mod polygon;
pub mod repanel;
pub mod shapes;
pub mod trailing_edge;
pub mod validate;
pub mod vec2;

pub use panel::{Panel, Panelisation};
pub use parse::{parse_coordinates, CoordinateFormat, ParseError, ParsedGeometry};
pub use polygon::{distance_point_segment, Polygon, Winding};
pub use repanel::{find_corners, repanel, PanelDistribution, RepanelConfig};
pub use shapes::{circle, ellipse, Joukowski, Naca4, TrailingEdge};
pub use trailing_edge::{
    analyse as analyse_trailing_edge, rotate_to, sharpest_corner, CornerInfo, TrailingEdgeAnalysis,
    SHARP_CORNER_THRESHOLD,
};
pub use validate::{
    cleanup, validate, CleanupOptions, CleanupResult, GeometryIssue, IssueKind, Severity,
    ValidationConfig, ValidationReport,
};
pub use vec2::{Bounds, Vec2};

/// Full import pipeline: parse → validate → clean → re-panelise → panels.
///
/// This is the single entry point the WASM layer calls, so the sequence of
/// operations is identical in tests and in the browser.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub polygon: Polygon,
    pub panelisation: Panelisation,
    pub report: ValidationReport,
    pub name: Option<String>,
    pub notes: Vec<String>,
    pub original_point_count: usize,
    /// Detected trailing edge, rotated to index 0 of `polygon`.
    pub trailing_edge: Option<CornerInfo>,
    /// Whether the geometry has a corner sharp enough to carry a Kutta
    /// condition. The UI uses this to pre-select the circulation mode.
    pub kutta_applicable: bool,
}

#[derive(Debug, Clone)]
pub enum ImportError {
    Parse(ParseError),
    Invalid(ValidationReport),
    Degenerate,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Parse(e) => write!(f, "{e}"),
            ImportError::Invalid(r) => {
                let msgs: Vec<&str> = r.errors().map(|i| i.message.as_str()).collect();
                write!(f, "{}", msgs.join(" "))
            }
            ImportError::Degenerate => write!(
                f,
                "After cleanup fewer than three distinct points remained; the contour is degenerate."
            ),
        }
    }
}

/// Parse and prepare a coordinate file for the solver.
pub fn import_geometry(
    text: &str,
    repanel_cfg: &RepanelConfig,
    cleanup_opts: &CleanupOptions,
) -> Result<ImportResult, ImportError> {
    let parsed = parse_coordinates(text).map_err(ImportError::Parse)?;
    let cfg = ValidationConfig::default();
    let report = validate(&parsed.points, &cfg);
    if report.has_errors() {
        return Err(ImportError::Invalid(report));
    }
    let cleaned = cleanup(&parsed.points, &cfg, cleanup_opts).ok_or(ImportError::Degenerate)?;
    let repaneled = repanel(&cleaned.polygon, repanel_cfg);
    // Rotate the contour so a sharp trailing edge lands on index 0, which is
    // where the Kutta condition expects it.
    let te = analyse_trailing_edge(&repaneled);
    let poly = te.polygon.clone();
    let panelisation = Panelisation::from_polygon(&poly).ok_or(ImportError::Degenerate)?;

    let mut notes = parsed.notes;
    if cleaned.reversed {
        notes.push("Point order was reversed to the counter-clockwise convention.".into());
    }
    if cleaned.removed_points > 0 {
        notes.push(format!(
            "{} duplicate point(s) were merged.",
            cleaned.removed_points
        ));
    }
    if let Some(gap) = cleaned.closed_gap {
        if gap > 0.0 {
            notes.push(format!(
                "The contour was closed with a straight panel spanning {gap:.4e}."
            ));
        }
    }
    if poly.len() != cleaned.polygon.len() {
        notes.push(format!(
            "Re-panelised from {} to {} panels.",
            cleaned.polygon.len(),
            poly.len()
        ));
    }
    if te.rotated_by > 0 {
        notes.push(format!(
            "Point order was rotated by {} so the contour starts at the detected trailing edge.",
            te.rotated_by
        ));
    }
    match &te.trailing_edge {
        Some(c) => notes.push(format!(
            "Sharp trailing edge detected at ({:.4}, {:.4}) with an included angle of {:.1}°; a Kutta condition can be applied.",
            c.position.x, c.position.y, c.included_angle.to_degrees()
        )),
        None => notes.push(
            "No sharp trailing edge was found, so this body is treated as non-lifting unless you prescribe a circulation."
                .to_string(),
        ),
    }

    Ok(ImportResult {
        polygon: poly,
        panelisation,
        report,
        name: parsed.name,
        notes,
        original_point_count: parsed.points.len(),
        trailing_edge: te.trailing_edge,
        kutta_applicable: te.kutta_applicable,
    })
}
