//! Cross-language contract test (PRD §68): every scene the TypeScript UI writes
//! to `examples/` must deserialise into the Rust `Scene` and solve cleanly.
//! The fixtures are generated from the UI's own example definitions
//! (`apps/web/scripts/export-examples.ts`), so a schema drift on either side
//! fails here.

use aeroflow_solver::*;
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn load(name: &str) -> Scene {
    let path = examples_dir().join(format!("{name}.aeroflow.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} does not match the Rust schema: {e}", path.display()))
}

fn solve(name: &str) -> Solution {
    let mut sim = Simulation::with_scene(load(name), no_clock);
    sim.solve().clone()
}

#[test]
fn every_fixture_deserialises_and_solves_without_error_or_warning() {
    let mut count = 0;
    for entry in std::fs::read_dir(examples_dir()).unwrap() {
        let path = entry.unwrap().path();
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".aeroflow.json"))
        else {
            continue;
        };
        let sol = solve(name);
        assert_eq!(sol.error, None, "{name}");
        let warnings: Vec<_> = sol
            .warnings
            .iter()
            .filter(|w| w.severity == WarningSeverity::Warning)
            .collect();
        assert!(
            warnings.is_empty(),
            "{name} shows warnings to a first-time user: {warnings:?}"
        );
        count += 1;
    }
    assert!(count >= 10, "only {count} fixtures found");
}

#[test]
fn cylinder_example_is_symmetric_with_zero_lift() {
    let s = solve("cylinder");
    assert!(s.bodies[0].forces.lift.abs() < 1e-9);
}

#[test]
fn naca0012_example_matches_the_showcase_claim() {
    // The example's description promises CL ≈ 0.60 (PRD §82).
    let cl = solve("naca0012").bodies[0].forces.cl.unwrap();
    assert!((0.57..0.63).contains(&cl), "CL = {cl}");
}

#[test]
fn magnus_example_lift_equals_kutta_joukowski() {
    let s = solve("magnus");
    let f = &s.bodies[0].forces;
    // ρ = 1.225, U = 1, Γ = −4  →  L = 4.9 N/m
    assert!((f.lift - 4.9).abs() / 4.9 < 5e-3, "lift {}", f.lift);
}

#[test]
fn naca2412_example_lifts_at_zero_alpha_with_nose_down_moment() {
    let f = solve("naca2412").bodies[0].forces;
    assert!(f.cl.unwrap() > 0.2 && f.cm.unwrap() < 0.0, "{f:?}");
}

#[test]
fn biplane_example_wings_each_lose_lift_to_interference() {
    let s = solve("biplane");
    assert_eq!(s.bodies.len(), 2);
    let single = {
        let mut sc = load("biplane");
        let first = sc.bodies[0].clone();
        sc.bodies = vec![SceneBody {
            position: Vec2::ZERO,
            ..first
        }];
        Simulation::with_scene(sc, no_clock).solve().bodies[0]
            .forces
            .cl
            .unwrap()
    };
    for b in &s.bodies {
        assert!(
            b.forces.cl.unwrap() < single,
            "{} {:?} vs isolated {single}",
            b.name,
            b.forces.cl
        );
    }
}

#[test]
fn source_sink_example_closes_into_an_oval() {
    let s = solve("source-sink");
    assert!(s.diagnostics.net_outflow.abs() < 1e-12);
}

#[test]
fn doublet_example_reproduces_the_panel_cylinder_off_body() {
    let mut d = Simulation::with_scene(load("doublet"), no_clock);
    d.solve();
    let mut c = Simulation::with_scene(load("cylinder"), no_clock);
    c.solve();
    for p in [
        Vec2::new(1.5, 0.3),
        Vec2::new(-0.4, 1.8),
        Vec2::new(2.5, -2.0),
    ] {
        let vd = d.probe(p).unwrap().velocity;
        let vc = c.probe(p).unwrap().velocity;
        assert!(
            (vd - vc).norm() < 5e-3,
            "at {p:?}: doublet {vd:?} vs panels {vc:?}"
        );
    }
}
