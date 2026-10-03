//! The shipped `examples/naca2412-selig.dat` must import cleanly — it is the
//! file the README tells users to try first.
use aeroflow_geometry::*;

#[test]
fn shipped_selig_file_imports_with_a_trailing_edge() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/naca2412-selig.dat"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let r = import_geometry(&text, &RepanelConfig::default(), &CleanupOptions::default()).unwrap();
    assert_eq!(r.name.as_deref(), Some("NACA 2412"));
    assert!(r.kutta_applicable);
    assert!(!r.report.has_errors(), "{:?}", r.report.issues);
    assert_eq!(r.panelisation.len(), RepanelConfig::default().count);
}
