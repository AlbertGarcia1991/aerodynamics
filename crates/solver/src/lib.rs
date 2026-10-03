//! # aeroflow-solver
//!
//! Orchestration layer: turns a user's [`Scene`] into a [`Solution`], caches
//! the influence matrix across solves, runs parameter sweeps, and exposes
//! field sampling, streamlines and point probes for the renderer.
//!
//! This is the crate the WASM boundary wraps. Nothing here knows about
//! JavaScript, and everything is exercised natively by the analytical
//! acceptance suite in `tests/analytical.rs` (PRD §61, WASM-007).

pub mod prepare;
pub mod scene;
pub mod simulation;
pub mod sweep;

pub use prepare::{prepare_body, PrepareError, PreparedBody};
pub use scene::{
    BodyGeometry, CirculationSetting, PanelSettings, ReferenceOverride, Scene, SceneBody,
    SceneElement, SCENE_FORMAT_VERSION,
};
pub use simulation::{
    no_clock, BodyResult, Diagnostics, ElementResult, NowFn, Probe, Simulation, Solution,
    SolveStatus, Timings, Warning, WarningSeverity, ASSUMPTIONS,
};
pub use sweep::{SweepBodyPoint, SweepConfig, SweepParameter, SweepPoint, SweepResult, SweepRun};

// Re-export the types a consumer needs to build scenes and read results
// without depending on every crate individually.
pub use aeroflow_flow_core::{
    Element, ElementKind, FieldEvalOptions, FieldType, FlowConditions, GridDefinition, ScalarField,
    SeedStrategy, SeedingConfig, Streamline, StreamlineConfig, Termination, VectorField,
};
pub use aeroflow_geometry::{Bounds, PanelDistribution, Vec2};
pub use aeroflow_panel_method::CirculationMode;
