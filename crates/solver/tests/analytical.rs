//! Analytical acceptance suite (PRD §61, §62, §81–§83).
//!
//! Every tolerance here was established by running the solver and recording
//! the observed error, then setting the bound with modest headroom. The
//! observed values are documented in `tests/numerical/TOLERANCES.md` so that a
//! future regression is visible as a number, not merely as a failed assert.

use aeroflow_solver::*;
use std::f64::consts::PI;

fn cond(u: f64, alpha_deg: f64) -> FlowConditions {
    FlowConditions {
        velocity: u,
        angle: alpha_deg.to_radians(),
        density: 1.0,
        pressure: 0.0,
    }
}

fn cylinder_scene(n: usize, circulation: CirculationSetting) -> Scene {
    let mut s = Scene::new("cylinder", cond(1.0, 0.0));
    let mut b = SceneBody::new("cyl", "Cylinder", BodyGeometry::Circle { radius: 1.0 });
    b.panels.count = n;
    b.circulation = circulation;
    s.bodies.push(b);
    s
}

fn naca_scene(code: &str, n: usize, alpha_deg: f64) -> Scene {
    let mut s = Scene::new("naca", cond(1.0, alpha_deg));
    let mut b = SceneBody::new(
        "af",
        code,
        BodyGeometry::Naca4 {
            code: code.into(),
            chord: 1.0,
        },
    );
    b.panels.count = n;
    s.bodies.push(b);
    s
}

/// RMS of `Cp_panel − Cp_analytic` over the cylinder surface.
fn cylinder_cp_rms(n: usize) -> f64 {
    let mut sim = Simulation::with_scene(cylinder_scene(n, CirculationSetting::None), no_clock);
    let sol = sim.solve();
    let b = &sol.bodies[0];
    let mut acc = 0.0;
    for sp in &b.surface {
        let theta = sp.position.angle();
        let exact = 1.0 - 4.0 * theta.sin().powi(2);
        let d = sp.cp.unwrap() - exact;
        acc += d * d;
    }
    (acc / b.surface.len() as f64).sqrt()
}

// ───────────────────────── §61.6 Cylinder ─────────────────────────

#[test]
fn cylinder_cp_matches_the_analytic_distribution() {
    // Observed: 120 panels → RMS ≈ 1.1e-3.
    let rms = cylinder_cp_rms(120);
    println!("cylinder Cp RMS (120 panels) = {rms:.3e}");
    assert!(rms < 5e-3, "RMS Cp error {rms}");
}

#[test]
fn cylinder_cp_is_exact_on_a_regular_polygon_at_every_panel_count() {
    // A remarkable property, not a coincidence: on a regular N-gon the
    // Hess–Smith system is circulant and the solution reproduces the analytic
    // cylinder surface speed at the panel midpoints to round-off for *every*
    // N. Observed RMS: 1.3e-15 (N=40), 2.5e-15 (N=80), 4.0e-15 (N=160).
    for n in [40usize, 80, 160] {
        let rms = cylinder_cp_rms(n);
        println!("cylinder Cp RMS N={n}: {rms:.3e}");
        assert!(
            rms < 1e-12,
            "N={n}: RMS {rms} — the circulant identity broke"
        );
    }
}

/// RMS of `|V_t| − V_exact` on an ellipse `x = a cos t, y = b sin t` in a stream
/// along `x`, where the exact surface speed is
/// `U (a + b) |sin t| / sqrt(a² sin² t + b² cos² t)`.
fn ellipse_speed_rms(n: usize) -> f64 {
    let (a, b) = (1.0f64, 0.5f64);
    let mut s = Scene::new("ellipse", cond(1.0, 0.0));
    let mut body = SceneBody::new(
        "e",
        "Ellipse",
        BodyGeometry::Ellipse {
            semi_axis_x: a,
            semi_axis_y: b,
        },
    );
    body.panels.count = n;
    s.bodies.push(body);
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve();
    let mut acc = 0.0;
    for sp in &sol.bodies[0].surface {
        let t = (sp.position.y / b).atan2(sp.position.x / a);
        let exact =
            (a + b) * t.sin().abs() / (a * a * t.sin().powi(2) + b * b * t.cos().powi(2)).sqrt();
        let d = sp.tangential_velocity.abs() - exact;
        acc += d * d;
    }
    (acc / sol.bodies[0].surface.len() as f64).sqrt()
}

#[test]
fn ellipse_surface_speed_converges_with_panel_count() {
    // Unlike the circle, the ellipse has genuine discretisation error, so it is
    // the right body for a convergence test.
    let e40 = ellipse_speed_rms(40);
    let e80 = ellipse_speed_rms(80);
    let e160 = ellipse_speed_rms(160);
    println!("ellipse |V| RMS: N=40 {e40:.3e}, N=80 {e80:.3e}, N=160 {e160:.3e}");
    assert!(e40 > 1e-6, "ellipse should not be exact: {e40}");
    assert!(e80 < 0.7 * e40, "no convergence 40→80: {e40} → {e80}");
    assert!(e160 < 0.7 * e80, "no convergence 80→160: {e80} → {e160}");
    assert!(e160 < 5e-3, "N=160 error {e160}");
}

#[test]
fn cylinder_at_rest_has_zero_lift_and_dalembert_drag() {
    let mut sim = Simulation::with_scene(cylinder_scene(120, CirculationSetting::None), no_clock);
    let sol = sim.solve();
    assert_ne!(sol.status, SolveStatus::Error);
    let f = &sol.bodies[0].forces;
    assert!(f.cl.unwrap().abs() < 1e-9, "CL = {:?}", f.cl);
    assert!(f.cd.unwrap().abs() < 1e-9, "CD = {:?}", f.cd);
    assert!(f.circulation.abs() < 1e-12);
    // The midpoint nearest the front stagnation point sits at θ = π ± π/N, so
    // its exact Cp is 1 − 4·sin²(π/N) = 0.99726 for N = 120 — and the panel
    // solution reproduces that to round-off (see the circulant-exactness test).
    let front = sol.bodies[0]
        .surface
        .iter()
        .min_by(|a, b| a.position.x.total_cmp(&b.position.x))
        .unwrap();
    let expected = 1.0 - 4.0 * (PI / 120.0).sin().powi(2);
    assert!(
        (front.cp.unwrap() - expected).abs() < 1e-9,
        "front Cp = {:?} vs {expected}",
        front.cp
    );
}

#[test]
fn cylinder_crown_speed_approaches_twice_the_freestream() {
    let mut sim = Simulation::with_scene(cylinder_scene(160, CirculationSetting::None), no_clock);
    let sol = sim.solve();
    let crown = sol.bodies[0]
        .surface
        .iter()
        .max_by(|a, b| a.position.y.total_cmp(&b.position.y))
        .unwrap();
    assert!(
        (crown.tangential_velocity.abs() - 2.0).abs() < 5e-3,
        "|V_t| at the crown = {}",
        crown.tangential_velocity.abs()
    );
    // Cp = 1 − 4 = −3 there.
    assert!((crown.cp.unwrap() + 3.0).abs() < 2e-2);
}

#[test]
fn cylinder_field_evaluation_matches_the_doublet_solution() {
    // The panel solution's *off-body* field must agree with the exact
    // doublet + uniform stream everywhere outside the cylinder.
    let mut sim = Simulation::with_scene(cylinder_scene(160, CirculationSetting::None), no_clock);
    sim.solve();
    let field = sim.field().unwrap();
    let exact = |p: Vec2| {
        let r2 = p.norm_sq();
        // u = U(1 + (y²−x²)/r⁴), v = −2Uxy/r⁴ for a = 1, U = 1
        Vec2::new(
            1.0 + (p.y * p.y - p.x * p.x) / (r2 * r2),
            -2.0 * p.x * p.y / (r2 * r2),
        )
    };
    let mut worst = 0.0f64;
    for (x, y) in [
        (1.5, 0.0),
        (0.0, 1.5),
        (-2.0, 1.0),
        (1.2, -1.2),
        (3.0, 3.0),
        (-0.9, 0.9),
    ] {
        let p = Vec2::new(x, y);
        let d = (field.velocity(p) - exact(p)).norm();
        worst = worst.max(d);
    }
    // Observed 5.2e-3 at 160 panels; the worst point is 0.27 radii off the
    // wall, where constant-strength panels are least accurate.
    println!("cylinder off-body velocity error (160 panels) = {worst:.3e}");
    assert!(worst < 1e-2, "worst field error {worst}");
}

// ───────────────────────── §61.7 Cylinder with circulation ─────────────────────────

#[test]
fn rotating_cylinder_obeys_kutta_joukowski() {
    let gamma = -3.0; // clockwise → positive lift
    let mut sim = Simulation::with_scene(
        cylinder_scene(160, CirculationSetting::Prescribed { circulation: gamma }),
        no_clock,
    );
    let sol = sim.solve();
    let f = &sol.bodies[0].forces;
    let expected = -gamma; // L = −ρU∞Γ with ρ = U∞ = 1
    println!("rotating cylinder: lift {} vs KJ {expected}", f.lift);
    assert!(
        (f.lift - expected).abs() / expected < 5e-3,
        "lift {} vs {expected}",
        f.lift
    );
    // |L_pressure − L_KJ| / (q∞·c) with c = 2a: observed 1.28e-2 at 160 panels.
    assert!(
        f.lift_consistency < 2e-2,
        "consistency {}",
        f.lift_consistency
    );
    assert!(f.cd.unwrap().abs() < 1e-6, "CD = {:?}", f.cd);
    assert!((f.circulation - gamma).abs() < 1e-12);
}

#[test]
fn rotating_cylinder_cp_matches_the_analytic_distribution() {
    let gamma = -2.0;
    let mut sim = Simulation::with_scene(
        cylinder_scene(160, CirculationSetting::Prescribed { circulation: gamma }),
        no_clock,
    );
    let sol = sim.solve();
    let mut acc = 0.0;
    for sp in &sol.bodies[0].surface {
        let theta = sp.position.angle();
        // V_θ = −2U sinθ + Γ/(2πa)
        let vt = -2.0 * theta.sin() + gamma / (2.0 * PI);
        let exact = 1.0 - vt * vt;
        let d = sp.cp.unwrap() - exact;
        acc += d * d;
    }
    let rms = (acc / sol.bodies[0].surface.len() as f64).sqrt();
    println!("rotating cylinder Cp RMS = {rms:.3e}");
    assert!(rms < 5e-3, "RMS {rms}");
}

#[test]
fn rotating_cylinder_stagnation_points_move_as_predicted() {
    // With Γ/(4πUa) = sin θ_s the stagnation points sit at θ_s and π − θ_s
    // (measured from the +x axis, in the lower half for clockwise Γ).
    let gamma = -2.0;
    let mut sim = Simulation::with_scene(
        cylinder_scene(200, CirculationSetting::Prescribed { circulation: gamma }),
        no_clock,
    );
    let sol = sim.solve();
    let theta_s = (gamma / (4.0 * PI)).asin(); // negative: lower surface
                                               // Find the two panels with the smallest |V_t| and check their angles.
    let mut by_speed: Vec<_> = sol.bodies[0].surface.iter().collect();
    by_speed.sort_by(|a, b| {
        a.tangential_velocity
            .abs()
            .total_cmp(&b.tangential_velocity.abs())
    });
    let found: Vec<f64> = by_speed[..2].iter().map(|s| s.position.angle()).collect();
    for f in &found {
        let d1 = (f - theta_s).abs();
        let d2 = (f - (PI - theta_s)).abs().min((f - (-PI - theta_s)).abs());
        assert!(
            d1.min(d2) < 0.05,
            "stagnation at {:.3}, expected {:.3} or {:.3}",
            f,
            theta_s,
            PI - theta_s
        );
    }
}

// ───────────────────────── §61.8 Airfoils ─────────────────────────

#[test]
fn naca0012_lift_at_five_degrees_matches_the_reference_range() {
    // XFOIL inviscid, 160 panels: CL ≈ 0.600 at α = 5°. Thin-airfoil theory
    // gives 2πα = 0.548; the 12% thickness raises the slope by ~9%.
    let mut sim = Simulation::with_scene(naca_scene("0012", 160, 5.0), no_clock);
    let sol = sim.solve();
    assert_ne!(sol.status, SolveStatus::Error, "{:?}", sol.error);
    let f = &sol.bodies[0].forces;
    let cl = f.cl.unwrap();
    println!(
        "NACA 0012 α=5°: CL = {cl:.4}, CD = {:.2e}, Cm = {:.4}, Γ = {:.4}",
        f.cd.unwrap(),
        f.cm.unwrap(),
        f.circulation
    );
    assert!((0.57..0.63).contains(&cl), "CL = {cl}");
    assert!(
        f.cd.unwrap().abs() < 5e-3,
        "CD = {:?} should be ~0 (inviscid)",
        f.cd
    );
    // Symmetric section: moment about the quarter chord is ~0.
    assert!(f.cm.unwrap().abs() < 0.01, "Cm = {:?}", f.cm);
    assert!(
        f.lift_consistency < 1e-2,
        "KJ vs pressure lift mismatch {}",
        f.lift_consistency
    );
}

#[test]
fn naca0012_lift_slope_matches_theory() {
    let cl_at = |deg: f64| {
        let mut sim = Simulation::with_scene(naca_scene("0012", 160, deg), no_clock);
        sim.solve().bodies[0].forces.cl.unwrap()
    };
    let slope = (cl_at(4.0) - cl_at(-4.0)) / (8.0f64).to_radians();
    println!(
        "NACA 0012 dCL/dα = {slope:.3} per rad (2π = {:.3})",
        2.0 * PI
    );
    // Thickness raises the inviscid slope above 2π; XFOIL gives ≈ 6.9/rad.
    assert!((6.5..7.3).contains(&slope), "slope {slope}");
}

#[test]
fn naca2412_has_positive_lift_and_nose_down_moment_at_zero_alpha() {
    // XFOIL inviscid at α = 0: CL ≈ 0.25, Cm_c/4 ≈ −0.053.
    let mut sim = Simulation::with_scene(naca_scene("2412", 160, 0.0), no_clock);
    let sol = sim.solve();
    let f = &sol.bodies[0].forces;
    println!(
        "NACA 2412 α=0°: CL = {:.4}, Cm = {:.4}",
        f.cl.unwrap(),
        f.cm.unwrap()
    );
    assert!((0.22..0.29).contains(&f.cl.unwrap()), "CL = {:?}", f.cl);
    assert!((-0.08..-0.03).contains(&f.cm.unwrap()), "Cm = {:?}", f.cm);
    assert!(f.circulation < 0.0, "lift requires clockwise circulation");
}

#[test]
fn cambered_airfoil_zero_lift_angle_is_negative() {
    // NACA 2412 thin-airfoil zero-lift angle ≈ −2.1°; XFOIL ≈ −2.2°.
    let cl_at = |deg: f64| {
        let mut sim = Simulation::with_scene(naca_scene("2412", 160, deg), no_clock);
        sim.solve().bodies[0].forces.cl.unwrap()
    };
    let (a, b) = (-3.0, -1.0);
    let (ca, cb) = (cl_at(a), cl_at(b));
    let alpha0 = a - ca * (b - a) / (cb - ca);
    println!("NACA 2412 zero-lift angle ≈ {alpha0:.2}°");
    assert!((-2.6..-1.7).contains(&alpha0), "α₀ = {alpha0}");
}

#[test]
fn joukowski_lift_converges_towards_the_conformal_mapping_solution() {
    // Exact: L = 4πρU²R·sin(α + β), R the pre-image circle radius,
    // β = atan2(dy, c + dx); hence CL = 8πR·sin(α+β)/chord.
    //
    // A Joukowski section has a *true cusp* (thickness ∝ s^{3/2}): near the
    // trailing edge the two surfaces are closer than their own panel length,
    // which a source-based formulation represents poorly. The result is
    // first-order convergence of the pressure-integrated lift:
    //   N = 100: −11.3 %,  200: −7.1 %,  400: −4.2 %,  800: −2.4 %
    // while the circulation from the Kutta condition converges faster:
    //   N = 200: −2.4 %,  400: −1.5 %,  800: −0.8 %.
    // Both routes to lift are asserted so a regression in either is caught.
    let (dx, dy, c) = (0.08, 0.05, 1.0);
    let raw = aeroflow_geometry::Joukowski { dx, dy, c }.generate(2000);
    let chord = raw.diameter().unwrap().2;
    let r = ((c + dx).powi(2) + dy * dy).sqrt();
    let beta = dy.atan2(c + dx);

    let run = |n: usize, alpha_deg: f64| -> (f64, f64, f64) {
        let alpha = alpha_deg.to_radians();
        let mut s = Scene::new("jouk", cond(1.0, alpha_deg));
        let mut b = SceneBody::new(
            "j",
            "Joukowski",
            BodyGeometry::Joukowski {
                thickness: dx,
                camber: dy,
            },
        );
        b.panels.count = n;
        s.bodies.push(b);
        let mut sim = Simulation::with_scene(s, no_clock);
        let f = sim.solve().bodies[0].forces;
        let exact = 8.0 * PI * r * (alpha + beta).sin() / chord;
        let cl_kj = -f.circulation / (0.5 * f.reference.chord);
        (f.cl.unwrap(), cl_kj, exact)
    };

    let mut prev_err = f64::INFINITY;
    for n in [100usize, 200, 400] {
        let (cl_p, cl_kj, exact) = run(n, 0.0);
        let err_p = (cl_p - exact).abs() / exact;
        println!("Joukowski N={n}: CL_pressure {cl_p:.4} ({:+.2}%), CL_KJ {cl_kj:.4} ({:+.2}%), exact {exact:.4}",
            100.0 * (cl_p - exact) / exact, 100.0 * (cl_kj - exact) / exact);
        assert!(
            err_p < prev_err,
            "pressure lift stopped converging at N={n}"
        );
        prev_err = err_p;
    }
    let (cl_p, cl_kj, exact) = run(400, 0.0);
    assert!(
        (cl_p - exact).abs() / exact < 0.05,
        "pressure CL {cl_p} vs {exact}"
    );
    assert!(
        (cl_kj - exact).abs() / exact < 0.02,
        "circulation CL {cl_kj} vs {exact}"
    );

    // Asymmetric case, so a sign error in α or β cannot hide.
    let (_, cl_kj, exact) = run(400, 4.0);
    assert!(
        (cl_kj - exact).abs() / exact < 0.025,
        "α=4°: circulation CL {cl_kj} vs {exact}"
    );
}

#[test]
fn airfoil_lift_scales_correctly_with_speed_density_and_chord() {
    // L ∝ ρU²c while CL is invariant.
    let base = {
        let mut sim = Simulation::with_scene(naca_scene("0012", 100, 4.0), no_clock);
        sim.solve().bodies[0].forces
    };
    let mut s = naca_scene("0012", 100, 4.0);
    s.conditions.velocity = 3.0;
    s.conditions.density = 2.0;
    s.bodies[0].scale = 2.0;
    let mut sim = Simulation::with_scene(s, no_clock);
    let scaled = sim.solve().bodies[0].forces;
    assert!((scaled.cl.unwrap() - base.cl.unwrap()).abs() < 1e-6);
    // ρ×2, U²×9, c×2 → ×36
    assert!(
        (scaled.lift / base.lift - 36.0).abs() < 1e-3,
        "ratio {}",
        scaled.lift / base.lift
    );
}

// ───────────────────────── Multiple bodies (§13) ─────────────────────────

#[test]
fn two_cylinders_in_tandem_repel_each_other_with_equal_and_opposite_drag() {
    let mut s = Scene::new("tandem", cond(1.0, 0.0));
    for (id, x) in [("front", -1.5), ("rear", 1.5)] {
        let mut b = SceneBody::new(id, id, BodyGeometry::Circle { radius: 1.0 });
        b.position = Vec2::new(x, 0.0);
        b.panels.count = 100;
        s.bodies.push(b);
    }
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve();
    assert_eq!(sol.bodies.len(), 2);
    let front = sol.body("front").unwrap().forces;
    let rear = sol.body("rear").unwrap().forces;
    println!(
        "tandem cylinders: drag front {:.4}, rear {:.4}",
        front.drag, rear.drag
    );
    // Symmetric about y = 0: no lift on either.
    assert!(front.lift.abs() < 1e-9 && rear.lift.abs() < 1e-9);
    // The gap between them is a slow, high-pressure region, so the pair is
    // pushed apart: the front body is pushed upstream (negative drag), the
    // rear downstream, and by x-reflection symmetry the two cancel exactly —
    // d'Alembert for the pair.
    assert!(
        front.drag < -1e-3,
        "front should be pushed upstream: {}",
        front.drag
    );
    assert!(
        rear.drag > 1e-3,
        "rear should be pushed downstream: {}",
        rear.drag
    );
    assert!((front.drag + rear.drag).abs() < 1e-9, "x-symmetry broken");
    assert!(sol.total.drag.abs() < 1e-9);
    // And the interaction is visible on the surface: the crown speed is no
    // longer the isolated 2U.
    let crown = sol.bodies[0]
        .surface
        .iter()
        .max_by(|a, b| a.position.y.total_cmp(&b.position.y))
        .unwrap();
    assert!(
        (crown.tangential_velocity.abs() - 2.0).abs() > 5e-3,
        "no interaction at the crown"
    );
}

#[test]
fn two_cylinders_side_by_side_attract_each_other() {
    // Venturi effect: faster flow in the gap lowers the pressure there, so
    // the upper body is pushed down and the lower body up, antisymmetrically.
    let mut s = Scene::new("side", cond(1.0, 0.0));
    for (id, y) in [("up", 1.5), ("down", -1.5)] {
        let mut b = SceneBody::new(id, id, BodyGeometry::Circle { radius: 1.0 });
        b.position = Vec2::new(0.0, y);
        b.panels.count = 100;
        s.bodies.push(b);
    }
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve();
    let up = sol.body("up").unwrap().forces.lift;
    let down = sol.body("down").unwrap().forces.lift;
    println!("side-by-side cylinders: lift up {up:.4}, down {down:.4}");
    assert!(up < -1e-3, "upper body should be pulled down: {up}");
    assert!(down > 1e-3, "lower body should be pulled up: {down}");
    assert!((up + down).abs() < 1e-9, "antisymmetry broken");
    assert!(sol.total.lift.abs() < 1e-9);
}

#[test]
fn biplane_interference_reduces_the_lift_of_each_wing() {
    // Two identical lifting airfoils one chord apart: each sits in the other's
    // downwash, so each produces less lift than it would alone and the pair
    // produces less than twice the isolated lift (Munk's biplane result).
    let isolated = {
        let mut sim = Simulation::with_scene(naca_scene("0012", 120, 4.0), no_clock);
        sim.solve().bodies[0].forces.cl.unwrap()
    };
    let mut s = naca_scene("0012", 120, 4.0);
    let mut lower = SceneBody::new(
        "lower",
        "Lower wing",
        BodyGeometry::Naca4 {
            code: "0012".into(),
            chord: 1.0,
        },
    );
    lower.position = Vec2::new(0.0, -1.0);
    lower.panels.count = 120;
    s.bodies.push(lower);
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve();
    let upper_cl = sol.bodies[0].forces.cl.unwrap();
    let lower_cl = sol.bodies[1].forces.cl.unwrap();
    println!("biplane: isolated CL {isolated:.4}, upper {upper_cl:.4}, lower {lower_cl:.4}");
    assert!(upper_cl > 0.0 && lower_cl > 0.0);
    assert!(
        upper_cl < isolated,
        "upper wing should lose lift: {upper_cl} vs {isolated}"
    );
    assert!(
        lower_cl < isolated,
        "lower wing should lose lift: {lower_cl} vs {isolated}"
    );
    assert!(upper_cl + lower_cl < 2.0 * isolated);
}

// ───────────────────────── Elementary superposition (§61.5, §83) ─────────────────────────

#[test]
fn source_sink_pair_in_a_stream_forms_a_rankine_oval() {
    let (u, lambda, b) = (1.0, 2.0, 1.0);
    let mut s = Scene::new("rankine", cond(u, 0.0));
    s.elements.push(SceneElement {
        id: "src".into(),
        name: "Source".into(),
        visible: true,
        locked: false,
        element: Element::Source {
            position: Vec2::new(-b, 0.0),
            strength: lambda,
        },
    });
    s.elements.push(SceneElement {
        id: "snk".into(),
        name: "Sink".into(),
        visible: true,
        locked: false,
        element: Element::Sink {
            position: Vec2::new(b, 0.0),
            strength: lambda,
        },
    });
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve().clone();
    // Equal source and sink: no net outflow, no warning about it.
    assert!(sol.diagnostics.net_outflow.abs() < 1e-12);
    assert!(!sol.warnings.iter().any(|w| w.code == "netOutflow"));

    let x_stag = (b * b + lambda * b / (PI * u)).sqrt();
    for x in [x_stag, -x_stag] {
        let p = sim.probe(Vec2::new(x, 0.0)).unwrap();
        assert!(p.speed < 1e-10, "stagnation at x = {x}: speed {}", p.speed);
        assert!((p.cp.unwrap() - 1.0).abs() < 1e-9);
    }
    // The dividing streamline through the stagnation point is closed.
    let bounds = Bounds {
        min: Vec2::new(-5.0, -5.0),
        max: Vec2::new(5.0, 5.0),
    };
    let cfg = StreamlineConfig::for_bounds(bounds);
    let lines = sim.streamlines(
        &cfg,
        &SeedingConfig {
            separation: 0.05,
            ..Default::default()
        },
    );
    assert!(lines.len() > 10, "{} streamlines", lines.len());
}

#[test]
fn vortex_in_a_freestream_reports_its_circulation() {
    let mut s = Scene::new("vortex", cond(1.0, 0.0));
    s.elements.push(SceneElement {
        id: "v".into(),
        name: "Vortex".into(),
        visible: true,
        locked: false,
        element: Element::Vortex {
            position: Vec2::ZERO,
            circulation: -2.0,
        },
    });
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve().clone();
    assert!((sol.diagnostics.total_circulation + 2.0).abs() < 1e-12);
    // A clockwise vortex moves fluid in −x *below* itself, so that is where
    // it cancels the stream: the stagnation point is at (0, −r) with
    // r = |Γ|/(2πU). Above the vortex the speeds add to 2U instead.
    let r = 2.0 / (2.0 * PI);
    let below = sim.probe(Vec2::new(0.0, -r)).unwrap();
    assert!(
        below.speed < 1e-10,
        "speed at the stagnation point {}",
        below.speed
    );
    let above = sim.probe(Vec2::new(0.0, r)).unwrap();
    assert!(
        (above.speed - 2.0).abs() < 1e-10,
        "speed above the vortex {}",
        above.speed
    );
}

#[test]
fn single_source_in_a_stream_warns_about_net_outflow() {
    let mut s = Scene::new("halfbody", cond(1.0, 0.0));
    s.elements.push(SceneElement {
        id: "src".into(),
        name: "Source".into(),
        visible: true,
        locked: false,
        element: Element::Source {
            position: Vec2::ZERO,
            strength: 1.0,
        },
    });
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve();
    assert!(sol.warnings.iter().any(|w| w.code == "netOutflow"));
    assert_eq!(
        sol.status,
        SolveStatus::Success,
        "an info note must not degrade the status"
    );
}

// ───────────────────────── Field sampling ─────────────────────────

#[test]
fn sampled_field_masks_the_body_and_reports_a_sane_range() {
    let mut sim = Simulation::with_scene(cylinder_scene(100, CirculationSetting::None), no_clock);
    sim.solve();
    let grid = GridDefinition::new(
        Bounds {
            min: Vec2::new(-3.0, -3.0),
            max: Vec2::new(3.0, 3.0),
        },
        61,
        61,
    );
    let f = sim
        .sample_scalar(
            FieldType::Cp,
            &grid,
            &FieldEvalOptions::for_rendering(grid.cell()),
        )
        .unwrap();
    assert_eq!(f.values.len(), 61 * 61);
    let masked = f
        .mask
        .iter()
        .filter(|m| **m == aeroflow_flow_core::MASK_BODY)
        .count();
    assert!(masked > 50, "cylinder interior should be masked: {masked}");
    assert!(f.max <= 1.0 + 1e-6, "Cp cannot exceed 1: {}", f.max);
    // With the near-wall band masked, the closest unmasked sample sits about
    // one cell (0.1) off the crown, where the exact speed is U(1 + a²/r²) =
    // 1.826 and Cp = −2.33; observed −2.375. It must stay above the wall value
    // of −3 and well below the freestream 0.
    assert!(
        f.min > -3.0 && f.min < -2.0,
        "Cp min {} inconsistent with the crown",
        f.min
    );
    assert!(f.softened);
}

// ───────────────────────── Performance (§65) ─────────────────────────

/// Benchmark table for PRD §65. Run with:
/// `cargo test -p aeroflow-solver --release --test analytical bench -- --ignored --nocapture`
#[test]
#[ignore]
fn bench_panel_counts() {
    use std::time::Instant;
    fn now_ms() -> f64 {
        use std::sync::OnceLock;
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
    }
    println!("| panels | prepare | assemble+LU | solve | forces | total |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for n in [10usize, 50, 100, 250, 500, 1000] {
        let mut sim = Simulation::with_scene(naca_scene("0012", n, 5.0), now_ms);
        let t = sim.solve().diagnostics.timings;
        println!(
            "| {n} | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms |",
            t.prepare_ms, t.assemble_ms, t.solve_ms, t.forces_ms, t.total_ms
        );
        // Field evaluation at render resolution.
        let grid = GridDefinition::new(
            Bounds {
                min: Vec2::new(-1.0, -1.0),
                max: Vec2::new(2.0, 1.0),
            },
            256,
            171,
        );
        let t0 = Instant::now();
        let _ = sim.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::for_rendering(grid.cell()),
        );
        let approx = t0.elapsed().as_secs_f64() * 1e3;
        let t0 = Instant::now();
        let _ = sim.sample_scalar(
            FieldType::VelocityMagnitude,
            &grid,
            &FieldEvalOptions::EXACT,
        );
        let exact = t0.elapsed().as_secs_f64() * 1e3;
        println!("|   field 256×171: far-field {approx:.1} ms, exact {exact:.1} ms |");
    }
}

// ───────────────────────── Lagally forces vs. pressure integration ─────────────────────────

/// In still fluid the total momentum flux at infinity vanishes, so the force on
/// a singularity near a body must be equal and opposite to the pressure force on
/// the body. The two sides come from independent computations — Lagally at a
/// point vs. pressure integrated over panels — so agreement validates both.
fn momentum_balance(element: Element, n: usize) -> (Vec2, Vec2) {
    let mut s = Scene::new(
        "balance",
        FlowConditions {
            velocity: 0.0,
            angle: 0.0,
            density: 1.2,
            pressure: 0.0,
        },
    );
    let mut b = SceneBody::new("cyl", "Cylinder", BodyGeometry::Circle { radius: 1.0 });
    b.panels.count = n;
    b.circulation = CirculationSetting::None;
    s.bodies.push(b);
    s.elements.push(SceneElement {
        id: "e".into(),
        name: "E".into(),
        visible: true,
        locked: false,
        element,
    });
    let mut sim = Simulation::with_scene(s, no_clock);
    let sol = sim.solve();
    assert_ne!(sol.status, SolveStatus::Error, "{:?}", sol.error);
    let body = Vec2::new(sol.bodies[0].forces.fx, sol.bodies[0].forces.fy);
    (sol.elements[0].force, body)
}

/// Exact force on a source `m` at distance `c` from the centre of a cylinder of
/// radius `a` in still fluid (Milne-Thomson circle theorem: image source `m` at
/// `a²/c`, image sink `−m` at the centre). Directed towards the cylinder.
fn source_near_cylinder_exact(m: f64, c: f64, a: f64, rho: f64) -> f64 {
    let b = a * a / c;
    rho * m * m / (2.0 * PI) * (1.0 / (c - b) - 1.0 / c)
}

/// Exact force on a vortex `Γ` at distance `c` from a non-circulating cylinder
/// (images: `−Γ` at `a²/c`, `+Γ` at the centre). Perpendicular to the radius.
fn vortex_near_cylinder_exact(g: f64, c: f64, a: f64, rho: f64) -> f64 {
    let b = a * a / c;
    rho * g * g / (2.0 * PI) * (1.0 / (c - b) - 1.0 / c)
}

#[test]
fn source_near_a_cylinder_matches_the_circle_theorem_and_balances() {
    let pos = Vec2::new(2.0, 0.6);
    let (on_source, on_body) = momentum_balance(
        Element::Source {
            position: pos,
            strength: 3.0,
        },
        240,
    );
    let exact = source_near_cylinder_exact(3.0, pos.norm(), 1.0, 1.2);
    println!(
        "source near cylinder: Lagally {:.5}, body {:.5}, exact {exact:.5}",
        on_source.norm(),
        on_body.norm()
    );
    // Observed at N = 240: Lagally +0.56 %, body −0.49 % (first-order convergence).
    assert!(
        (on_source.norm() - exact).abs() / exact < 1e-2,
        "Lagally vs circle theorem"
    );
    assert!(
        (on_body.norm() - exact).abs() / exact < 1e-2,
        "body pressure vs circle theorem"
    );
    // Directed exactly towards the cylinder centre, and equal-and-opposite.
    assert!(on_source.normalized().dot(-pos.normalized()) > 1.0 - 1e-6);
    assert!((on_source + on_body).norm() / exact < 1.5e-2);
}

#[test]
fn vortex_near_a_cylinder_matches_the_circle_theorem_and_balances() {
    let pos = Vec2::new(-1.8, 1.0);
    let (on_vortex, on_body) = momentum_balance(
        Element::Vortex {
            position: pos,
            circulation: 4.0,
        },
        240,
    );
    let exact = vortex_near_cylinder_exact(4.0, pos.norm(), 1.0, 1.2);
    println!(
        "vortex near cylinder: Lagally {:.5}, body {:.5}, exact {exact:.5}",
        on_vortex.norm(),
        on_body.norm()
    );
    assert!(
        (on_vortex.norm() - exact).abs() / exact < 1e-2,
        "Lagally vs circle theorem"
    );
    assert!(
        (on_body.norm() - exact).abs() / exact < 1e-2,
        "body pressure vs circle theorem"
    );
    // Radial: the vortex is pulled towards the cylinder like its image.
    assert!(on_vortex.normalized().dot(pos.normalized()).abs() > 1.0 - 1e-6);
    assert!((on_vortex + on_body).norm() / exact < 1.5e-2);
}

#[test]
fn doublet_near_a_cylinder_balances_the_pressure_force_on_it() {
    let (on_doublet, on_body) = momentum_balance(
        Element::Doublet {
            position: Vec2::new(2.2, -0.4),
            strength: 2.0,
            orientation: 0.5,
        },
        240,
    );
    println!("doublet near cylinder: Lagally {on_doublet:?}, body {on_body:?}");
    let rel = (on_doublet + on_body).norm() / on_doublet.norm();
    assert!(rel < 2e-2, "momentum imbalance {rel}");
}

#[test]
fn source_near_a_cylinder_balance_converges_with_panel_count() {
    let imbalance = |n| {
        let (a, b) = momentum_balance(
            Element::Source {
                position: Vec2::new(1.6, 0.0),
                strength: 2.0,
            },
            n,
        );
        (a + b).norm() / a.norm()
    };
    let (e60, e240) = (imbalance(60), imbalance(240));
    println!("source balance: N=60 {e60:.3e}, N=240 {e240:.3e}");
    assert!(e240 < e60, "no convergence: {e60} -> {e240}");
}
