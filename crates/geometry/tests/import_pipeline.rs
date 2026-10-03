//! End-to-end import pipeline tests (PRD §10.2).

use aeroflow_geometry::*;

fn csv_from(points: &[(f64, f64)]) -> String {
    let mut s = String::from("x,y\n");
    for (x, y) in points {
        s.push_str(&format!("{x},{y}\n"));
    }
    s
}

fn naca_csv(n: usize) -> String {
    let poly = Naca4::default().generate(n);
    parse::to_csv(&poly.points)
}

#[test]
fn naca_round_trip_through_the_import_pipeline() {
    let text = naca_csv(160);
    let r = import_geometry(&text, &RepanelConfig::default(), &CleanupOptions::default()).unwrap();
    assert!(!r.report.has_errors(), "{:?}", r.report.issues);
    assert!(
        r.kutta_applicable,
        "a NACA section must support a Kutta condition"
    );
    assert_eq!(r.polygon.winding(), Winding::CounterClockwise);
    assert!(
        (r.polygon.points[0].x - 1.0).abs() < 1e-6,
        "must start at the TE"
    );
    assert!(r.panelisation.len() > 100);
    assert!(r.panelisation.min_length > 0.0);
}

#[test]
fn cylinder_import_reports_no_trailing_edge() {
    let text = parse::to_csv(&circle(1.0, 120).points);
    let r = import_geometry(&text, &RepanelConfig::default(), &CleanupOptions::default()).unwrap();
    assert!(!r.kutta_applicable);
    assert!(r.trailing_edge.is_none());
    assert!(
        r.notes.iter().any(|n| n.contains("non-lifting")),
        "{:?}",
        r.notes
    );
}

#[test]
fn clockwise_file_is_reversed_and_noted() {
    let mut poly = Naca4::default().generate(120);
    poly.reverse();
    let r = import_geometry(
        &parse::to_csv(&poly.points),
        &RepanelConfig::default(),
        &CleanupOptions::default(),
    )
    .unwrap();
    assert_eq!(r.polygon.winding(), Winding::CounterClockwise);
    assert!(
        r.notes.iter().any(|n| n.contains("counter-clockwise")),
        "{:?}",
        r.notes
    );
}

#[test]
fn file_starting_mid_surface_is_rotated_to_the_trailing_edge() {
    let poly = Naca4::default().generate(160);
    let te = poly.points[0];
    let shifted = rotate_to(&poly, 61);
    let r = import_geometry(
        &parse::to_csv(&shifted.points),
        &RepanelConfig {
            distribution: PanelDistribution::AsImported,
            ..Default::default()
        },
        &CleanupOptions::default(),
    )
    .unwrap();
    assert!(r.polygon.points[0].distance(te) < 1e-6, "TE not restored");
    assert!(
        r.notes.iter().any(|n| n.contains("rotated")),
        "{:?}",
        r.notes
    );
}

#[test]
fn self_intersecting_geometry_is_rejected_with_an_actionable_message() {
    let text = csv_from(&[(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)]);
    let err =
        import_geometry(&text, &RepanelConfig::default(), &CleanupOptions::default()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("crosses"), "{msg}");
    assert!(msg.contains("simple") || msg.contains("self"), "{msg}");
}

#[test]
fn duplicate_points_are_an_error_until_cleanup_is_requested() {
    let mut pts: Vec<(f64, f64)> = circle(1.0, 40).points.iter().map(|p| (p.x, p.y)).collect();
    pts.insert(7, pts[7]);
    let text = csv_from(&pts);
    // Validation runs before cleanup, so a true zero-length segment is fatal
    // and the user is told how to fix it.
    let err =
        import_geometry(&text, &RepanelConfig::default(), &CleanupOptions::default()).unwrap_err();
    assert!(err.to_string().contains("zero length"), "{err}");
}

#[test]
fn empty_and_garbage_files_produce_readable_errors() {
    for bad in ["", "   \n\n", "this is not data\njust prose\n"] {
        let err = import_geometry(bad, &RepanelConfig::default(), &CleanupOptions::default())
            .unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn repanelisation_count_is_honoured_and_noted() {
    // `Naca4::generate` shares the leading and trailing edge between the two
    // surfaces, so it emits slightly fewer than the requested point count.
    let generated = Naca4::default().generate(240);
    let text = parse::to_csv(&generated.points);
    let r = import_geometry(
        &text,
        &RepanelConfig {
            count: 80,
            ..Default::default()
        },
        &CleanupOptions::default(),
    )
    .unwrap();
    assert_eq!(r.panelisation.len(), 80);
    assert!(
        r.notes.iter().any(|n| n.contains("Re-panelised")),
        "{:?}",
        r.notes
    );
    assert_eq!(r.original_point_count, generated.len());
}

#[test]
fn whitespace_dat_file_with_a_name_line_imports() {
    let poly = Naca4::parse("2412").unwrap().generate(140);
    let mut text = String::from("NACA 2412\n");
    for p in &poly.points {
        text.push_str(&format!("  {:.6}  {:.6}\n", p.x, p.y));
    }
    let r = import_geometry(&text, &RepanelConfig::default(), &CleanupOptions::default()).unwrap();
    assert_eq!(r.name.as_deref(), Some("NACA 2412"));
    assert!(r.kutta_applicable);
}
