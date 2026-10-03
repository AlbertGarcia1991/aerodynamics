//! Imported-geometry validation and cleanup (PRD §10.3, §11).
//!
//! Validation is deliberately *diagnostic*: it reports every problem it finds
//! with an actionable message and, where possible, the offending index, instead
//! of failing on the first error. Cleanup is a separate, explicitly requested
//! step so the user always knows what was changed.

use crate::polygon::{segments_properly_intersect, Polygon, Winding};
use crate::vec2::{Bounds, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum IssueKind {
    TooFewPoints,
    NonFiniteCoordinate,
    DuplicatePoint,
    ZeroLengthSegment,
    TinySegment,
    SelfIntersection,
    OpenContour,
    DegenerateArea,
    ExtremeCoordinateRange,
    NonUniformPanelDistribution,
    ClockwiseWinding,
    RepeatedClosingPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Severity {
    /// Informational: something was normalised, nothing is wrong.
    Info,
    /// The geometry is usable but the result may be degraded.
    Warning,
    /// The geometry cannot be solved as given.
    Error,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct GeometryIssue {
    pub kind: IssueKind,
    pub severity: Severity,
    /// Human-readable, actionable text shown directly in the UI.
    pub message: String,
    /// Index of the offending point or segment, when one applies.
    pub index: Option<usize>,
    /// How many further occurrences were collapsed into this issue.
    pub additional_occurrences: usize,
}

impl GeometryIssue {
    fn new(kind: IssueKind, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            kind,
            severity,
            message: message.into(),
            index: None,
            additional_occurrences: 0,
        }
    }

    fn at(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }
}

/// Thresholds for validation, all relative to the contour size so the same
/// settings work for a 1 m chord and a 1000 mm chord.
#[derive(Debug, Clone, Copy)]
pub struct ValidationConfig {
    /// Points closer than `duplicate_tol × diagonal` are duplicates.
    pub duplicate_tol: f64,
    /// Segments shorter than `tiny_segment_tol × diagonal` are flagged.
    pub tiny_segment_tol: f64,
    /// A gap below `closure_tol × diagonal` can be closed automatically.
    pub closure_tol: f64,
    /// Flag the distribution when two *neighbouring* segments differ in length
    /// by more than this factor. A smooth grading is fine; abrupt jumps are not.
    pub max_length_ratio: f64,
    pub min_points: usize,
    /// Absolute magnitude above which coordinates are suspicious.
    pub max_abs_coordinate: f64,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            duplicate_tol: 1e-9,
            tiny_segment_tol: 1e-6,
            closure_tol: 1e-3,
            max_length_ratio: 4.0,
            min_points: 4,
            max_abs_coordinate: 1e7,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub issues: Vec<GeometryIssue>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &GeometryIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    pub fn worst_severity(&self) -> Option<Severity> {
        self.issues.iter().map(|i| i.severity).max()
    }

    fn push(&mut self, issue: GeometryIssue) {
        // Collapse repeats of the same kind so a mangled file produces a short,
        // readable report rather than hundreds of identical lines.
        if let Some(existing) = self.issues.iter_mut().find(|i| i.kind == issue.kind) {
            existing.additional_occurrences += 1;
            return;
        }
        self.issues.push(issue);
    }
}

/// Validate a raw imported point list, before any cleanup.
///
/// `points` is the sequence as parsed. A repeated closing point (first == last)
/// is tolerated and reported as [`IssueKind::RepeatedClosingPoint`].
pub fn validate(points: &[Vec2], cfg: &ValidationConfig) -> ValidationReport {
    let mut report = ValidationReport::default();

    for (i, p) in points.iter().enumerate() {
        if !p.is_finite() {
            report.push(
                GeometryIssue::new(
                    IssueKind::NonFiniteCoordinate,
                    Severity::Error,
                    format!(
                        "Point {} is not a finite number ({}, {}). Remove or correct the row.",
                        i + 1,
                        p.x,
                        p.y
                    ),
                )
                .at(i),
            );
        }
    }
    if report.has_errors() {
        // Everything downstream assumes finite coordinates.
        return report;
    }

    if points.len() < cfg.min_points {
        report.push(GeometryIssue::new(
            IssueKind::TooFewPoints,
            Severity::Error,
            format!(
                "Geometry has {} point(s); at least {} are needed to form a closed body.",
                points.len(),
                cfg.min_points
            ),
        ));
        return report;
    }

    let bounds = Bounds::from_points(points).expect("non-empty");
    let diag = bounds.diagonal();
    if diag <= 0.0 {
        report.push(GeometryIssue::new(
            IssueKind::DegenerateArea,
            Severity::Error,
            "All points are coincident; the geometry has no extent.",
        ));
        return report;
    }

    let max_abs = points
        .iter()
        .flat_map(|p| [p.x.abs(), p.y.abs()])
        .fold(0.0f64, f64::max);
    if max_abs > cfg.max_abs_coordinate {
        report.push(GeometryIssue::new(
            IssueKind::ExtremeCoordinateRange,
            Severity::Warning,
            format!(
                "Coordinates reach {:.3e}. Check the units of the file — the solver works in metres.",
                max_abs
            ),
        ));
    }

    // Does the file repeat the first point at the end?
    let closing_repeated =
        points.first().unwrap().distance(*points.last().unwrap()) < cfg.duplicate_tol * diag;
    if closing_repeated {
        report.push(GeometryIssue::new(
            IssueKind::RepeatedClosingPoint,
            Severity::Info,
            "The first point is repeated at the end of the file; the duplicate will be dropped.",
        ));
    }

    // Work on the de-duplicated closing form for the remaining checks.
    let core: &[Vec2] = if closing_repeated {
        &points[..points.len() - 1]
    } else {
        points
    };

    // Interior duplicates and short segments.
    let n = core.len();
    for i in 0..n {
        let a = core[i];
        let b = core[(i + 1) % n];
        let d = a.distance(b);
        if d <= 0.0 {
            report.push(
                GeometryIssue::new(
                    IssueKind::ZeroLengthSegment,
                    Severity::Error,
                    format!(
                        "Segment {} has zero length (duplicate coordinate). Remove the duplicate point or enable automatic cleanup.",
                        i + 1
                    ),
                )
                .at(i),
            );
        } else if d < cfg.duplicate_tol * diag {
            report.push(
                GeometryIssue::new(
                    IssueKind::DuplicatePoint,
                    Severity::Warning,
                    format!(
                        "Points {} and {} are within {:.1e} of each other. Enable automatic cleanup to merge them.",
                        i + 1, ((i + 1) % n) + 1, cfg.duplicate_tol * diag
                    ),
                )
                .at(i),
            );
        } else if d < cfg.tiny_segment_tol * diag {
            report.push(
                GeometryIssue::new(
                    IssueKind::TinySegment,
                    Severity::Warning,
                    format!(
                        "Segment {} is {:.2e} long — far smaller than the body ({:.4e}). Very short panels degrade conditioning.",
                        i + 1, d, diag
                    ),
                )
                .at(i),
            );
        }
    }

    // Closure: a large gap between first and last point means an open contour.
    if !closing_repeated {
        let gap = core.first().unwrap().distance(*core.last().unwrap());
        // Compare the closing gap against the typical segment length, not the
        // body size: a coarse contour legitimately has long closing segments.
        let mean_seg = Polygon::new(core.to_vec()).perimeter() / n as f64;
        if gap > (cfg.closure_tol * diag).max(3.0 * mean_seg) {
            report.push(GeometryIssue::new(
                IssueKind::OpenContour,
                Severity::Warning,
                format!(
                    "The contour does not close: the gap between the first and last point is {:.4e} ({:.1}× the mean segment length). Close the contour or confirm that the straight closing segment is intended.",
                    gap, gap / mean_seg.max(1e-300)
                ),
            ));
        }
    }

    let poly = Polygon::new(core.to_vec());

    match poly.winding() {
        Winding::Degenerate => report.push(GeometryIssue::new(
            IssueKind::DegenerateArea,
            Severity::Error,
            "The contour encloses no area. Check that the points describe a closed body rather than a line.",
        )),
        Winding::Clockwise => report.push(GeometryIssue::new(
            IssueKind::ClockwiseWinding,
            Severity::Info,
            "Points are ordered clockwise; they will be reversed to the solver's counter-clockwise convention.",
        )),
        Winding::CounterClockwise => {}
    }

    if let Some(idx) = find_self_intersection(core) {
        report.push(
            GeometryIssue::new(
                IssueKind::SelfIntersection,
                Severity::Error,
                format!(
                    "Segment {} crosses another segment. A panel method requires a simple (non-self-intersecting) contour.",
                    idx + 1
                ),
            )
            .at(idx),
        );
    }

    let lengths: Vec<f64> = (0..n)
        .map(|i| core[i].distance(core[(i + 1) % n]))
        .collect();
    let mut worst = (1.0f64, 0usize);
    for i in 0..n {
        let (a, c) = (lengths[i], lengths[(i + 1) % n]);
        if a > 0.0 && c > 0.0 {
            let r = a.max(c) / a.min(c);
            if r > worst.0 {
                worst = (r, i);
            }
        }
    }
    if worst.0 > cfg.max_length_ratio {
        report.push(
            GeometryIssue::new(
                IssueKind::NonUniformPanelDistribution,
                Severity::Warning,
                format!(
                    "Segments {} and {} differ in length by {:.1}×. Abrupt changes between neighbouring panels reduce accuracy — re-panelisation on import fixes this.",
                    worst.1 + 1, (worst.1 + 1) % n + 1, worst.0
                ),
            )
            .at(worst.1),
        );
    }

    report
}

/// First segment index that properly crosses a non-adjacent segment, if any.
///
/// `O(n²)`, run once per import. Adjacent segments share an endpoint and are
/// skipped; see [`segments_properly_intersect`].
pub fn find_self_intersection(points: &[Vec2]) -> Option<usize> {
    let n = points.len();
    if n < 4 {
        return None;
    }
    for i in 0..n {
        let a1 = points[i];
        let a2 = points[(i + 1) % n];
        for j in (i + 1)..n {
            // Skip adjacent segments (including the wrap-around pair).
            if j == i || j == (i + 1) % n || (j + 1) % n == i {
                continue;
            }
            let b1 = points[j];
            let b2 = points[(j + 1) % n];
            if segments_properly_intersect(a1, a2, b1, b2) {
                return Some(i);
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
pub struct CleanupOptions {
    pub merge_duplicates: bool,
    pub drop_repeated_closing_point: bool,
    pub close_contour: bool,
    pub force_counter_clockwise: bool,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            merge_duplicates: true,
            drop_repeated_closing_point: true,
            close_contour: true,
            force_counter_clockwise: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub polygon: Polygon,
    pub removed_points: usize,
    pub reversed: bool,
    pub closed_gap: Option<f64>,
}

/// Normalise a raw point list into a solver-ready counter-clockwise contour.
///
/// Returns `None` only when fewer than three distinct points survive.
pub fn cleanup(
    points: &[Vec2],
    cfg: &ValidationConfig,
    opts: &CleanupOptions,
) -> Option<CleanupResult> {
    let finite: Vec<Vec2> = points.iter().copied().filter(|p| p.is_finite()).collect();
    if finite.len() < 3 {
        return None;
    }
    let diag = Bounds::from_points(&finite)?.diagonal();
    let tol = (cfg.duplicate_tol * diag).max(f64::MIN_POSITIVE);

    let mut out: Vec<Vec2> = Vec::with_capacity(finite.len());
    for p in &finite {
        if opts.merge_duplicates {
            if let Some(last) = out.last() {
                if last.distance(*p) <= tol {
                    continue;
                }
            }
        }
        out.push(*p);
    }

    let mut closed_gap = None;
    if out.len() > 2 {
        let gap = out.first().unwrap().distance(*out.last().unwrap());
        if gap <= tol && opts.drop_repeated_closing_point {
            out.pop();
        } else if opts.close_contour {
            // Nothing to insert: the contour is closed implicitly by the panel
            // that runs from the last point back to the first. Record the gap
            // so the caller can tell the user what was bridged.
            closed_gap = Some(gap);
        }
    }

    if out.len() < 3 {
        return None;
    }

    let removed_points = points.len() - out.len();
    let mut poly = Polygon::new(out);
    let reversed = if opts.force_counter_clockwise {
        poly.make_counter_clockwise()
    } else {
        false
    };

    Some(CleanupResult {
        polygon: poly,
        removed_points,
        reversed,
        closed_gap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(n: usize) -> Vec<Vec2> {
        (0..n)
            .map(|i| {
                let t = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                Vec2::new(t.cos(), t.sin())
            })
            .collect()
    }

    #[test]
    fn clean_circle_has_no_issues() {
        let r = validate(&circle(64), &ValidationConfig::default());
        assert!(r.issues.is_empty(), "unexpected: {:?}", r.issues);
    }

    #[test]
    fn non_finite_is_an_error_and_short_circuits() {
        let mut pts = circle(16);
        pts[3] = Vec2::new(f64::NAN, 0.0);
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r.has_errors());
        assert_eq!(r.issues[0].kind, IssueKind::NonFiniteCoordinate);
    }

    #[test]
    fn infinite_coordinate_is_an_error() {
        let mut pts = circle(16);
        pts[5] = Vec2::new(f64::INFINITY, 1.0);
        assert!(validate(&pts, &ValidationConfig::default()).has_errors());
    }

    #[test]
    fn duplicate_consecutive_point_is_a_zero_length_segment() {
        let mut pts = circle(16);
        pts.insert(4, pts[4]);
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::ZeroLengthSegment));
        assert!(r.has_errors());
    }

    #[test]
    fn repeated_closing_point_is_only_informational() {
        let mut pts = circle(32);
        pts.push(pts[0]);
        let r = validate(&pts, &ValidationConfig::default());
        assert!(!r.has_errors(), "{:?}", r.issues);
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::RepeatedClosingPoint));
    }

    #[test]
    fn self_intersection_is_detected() {
        // A bow-tie.
        let pts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
        ];
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::SelfIntersection));
    }

    #[test]
    fn simple_square_is_not_reported_as_self_intersecting() {
        assert!(find_self_intersection(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ])
        .is_none());
    }

    #[test]
    fn open_contour_is_flagged() {
        // Half a circle: the closing gap is the diameter.
        let pts: Vec<Vec2> = circle(64).into_iter().take(32).collect();
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r.issues.iter().any(|i| i.kind == IssueKind::OpenContour));
    }

    #[test]
    fn clockwise_input_is_flagged_and_then_reversed_by_cleanup() {
        let mut pts = circle(32);
        pts.reverse();
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::ClockwiseWinding));

        let c = cleanup(
            &pts,
            &ValidationConfig::default(),
            &CleanupOptions::default(),
        )
        .unwrap();
        assert!(c.reversed);
        assert_eq!(c.polygon.winding(), Winding::CounterClockwise);
    }

    #[test]
    fn cleanup_merges_duplicates_and_drops_closing_repeat() {
        let mut pts = circle(32);
        pts.insert(10, pts[10]);
        pts.push(pts[0]);
        let c = cleanup(
            &pts,
            &ValidationConfig::default(),
            &CleanupOptions::default(),
        )
        .unwrap();
        assert_eq!(c.polygon.len(), 32);
        assert_eq!(c.removed_points, 2);
    }

    #[test]
    fn degenerate_collinear_contour_is_an_error() {
        let pts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(3.0, 0.0),
        ];
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r.issues.iter().any(|i| i.kind == IssueKind::DegenerateArea));
    }

    #[test]
    fn extreme_coordinates_warn_about_units() {
        let pts: Vec<Vec2> = circle(32).into_iter().map(|p| p * 1e8).collect();
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::ExtremeCoordinateRange));
    }

    #[test]
    fn smooth_grading_passes_but_an_abrupt_jump_is_flagged() {
        // A cosine-spaced NACA has a large global ratio but smooth neighbours.
        let smooth = crate::shapes::Naca4::default().generate(400).points;
        let r = validate(&smooth, &ValidationConfig::default());
        assert!(
            !r.issues
                .iter()
                .any(|i| i.kind == IssueKind::NonUniformPanelDistribution),
            "{:?}",
            r.issues
        );
        // One long edge next to many short ones.
        let mut pts = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)];
        for i in 1..=10 {
            pts.push(Vec2::new(1.0, i as f64 / 10.0));
        }
        pts.push(Vec2::new(0.0, 1.0));
        let r = validate(&pts, &ValidationConfig::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::NonUniformPanelDistribution));
    }

    #[test]
    fn report_collapses_repeated_issues_of_the_same_kind() {
        let mut pts = circle(32);
        for i in (0..30).rev() {
            pts.insert(i, pts[i]);
        }
        let r = validate(&pts, &ValidationConfig::default());
        let zero_len: Vec<_> = r
            .issues
            .iter()
            .filter(|i| i.kind == IssueKind::ZeroLengthSegment)
            .collect();
        assert_eq!(zero_len.len(), 1, "issues should be collapsed");
        assert!(zero_len[0].additional_occurrences > 10);
    }
}
