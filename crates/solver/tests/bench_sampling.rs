//! Render-path sampling benchmark (PRD §65): scalar field vs. streamline tracing,
//! exact vs. far-field. Run with
//! `cargo test -p aeroflow-solver --release --test bench_sampling -- --ignored --nocapture`.

use aeroflow_solver::*;
use std::time::Instant;

#[test]
#[ignore]
fn where_does_sampling_time_go() {
    for n in [160usize, 500, 1000] {
        let mut s = Scene::new(
            "p",
            FlowConditions {
                velocity: 10.0,
                angle: 0.087,
                density: 1.225,
                pressure: 0.0,
            },
        );
        let mut b = SceneBody::new(
            "a",
            "A",
            BodyGeometry::Naca4 {
                code: "0012".into(),
                chord: 1.0,
            },
        );
        b.panels.count = n;
        s.bodies.push(b);
        let mut sim = Simulation::with_scene(s, no_clock);
        sim.solve();
        let bounds = Bounds {
            min: Vec2::new(-1.0, -0.8),
            max: Vec2::new(2.0, 0.8),
        };
        let grid = GridDefinition::new(bounds, 280, 150);
        let t = Instant::now();
        let _ = sim.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::for_rendering(grid.cell()),
        );
        let scalar = t.elapsed().as_secs_f64() * 1e3;
        for (label, cfg) in [
            ("exact", StreamlineConfig::for_bounds(bounds)),
            ("render", StreamlineConfig::for_rendering(bounds)),
        ] {
            let t = Instant::now();
            let lines = sim.streamlines(&cfg, &SeedingConfig::default());
            let sl = t.elapsed().as_secs_f64() * 1e3;
            let pts: usize = lines.iter().map(|l| l.points.len()).sum();
            println!("N={n}: scalar {scalar:.1} ms | streamlines[{label}] {sl:.1} ms ({} lines, {pts} pts)", lines.len());
        }
    }
}

#[test]
#[ignore]
fn which_warnings_fire_on_generated_airfoils() {
    for n in [120usize, 160, 300, 1000] {
        let mut s = Scene::new(
            "w",
            FlowConditions {
                velocity: 10.0,
                angle: 0.087,
                density: 1.225,
                pressure: 0.0,
            },
        );
        let mut b = SceneBody::new(
            "a",
            "A",
            BodyGeometry::Naca4 {
                code: "0012".into(),
                chord: 1.0,
            },
        );
        b.panels.count = n;
        s.bodies.push(b);
        let mut sim = Simulation::with_scene(s, no_clock);
        let sol = sim.solve();
        let d = sol.diagnostics.panel.as_ref().unwrap();
        println!(
            "N={n}: ratio {:.0} cond {:.1e} warnings {:?}",
            d.max_panel_length / d.min_panel_length,
            d.condition_number,
            sol.warnings
                .iter()
                .map(|w| (&w.code, w.message.chars().take(70).collect::<String>()))
                .collect::<Vec<_>>()
        );
    }
}
