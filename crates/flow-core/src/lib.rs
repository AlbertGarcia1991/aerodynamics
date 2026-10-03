//! # aeroflow-flow-core
//!
//! Elementary potential-flow solutions, the constant-strength panel influence
//! kernel, field evaluation, streamline tracing and aerodynamic force
//! integration.
//!
//! This crate owns the *physics*. It knows nothing about linear systems (that
//! is `aeroflow-panel-method`) or about rendering (PRD §86.1: solver ≠
//! renderer), and it compiles and tests natively with no browser involved
//! (WASM-007).
//!
//! ## Conventions
//!
//! * SI units throughout; 2D forces are **per unit span** (PRD §69).
//! * `+x` right, `+y` up, angles counter-clockwise (PRD §14.1).
//! * Circulation `Γ > 0` is counter-clockwise, so `L = −ρU∞Γ`. See
//!   [`elements`] for the full table and the reasoning.
//! * All numerical state is `f64`; only render buffers drop to `f32`
//!   (PRD §70).

// Dense numerical kernels read most clearly as explicit index loops over
// matrices and panel arrays; the iterator rewrites clippy suggests obscure them.
#![allow(clippy::needless_range_loop)]

pub mod conditions;
pub mod descriptor;
pub mod elements;
pub mod field;
pub mod forces;
pub mod panel_kernel;
pub mod streamline;

pub use conditions::FlowConditions;
pub use descriptor::{
    concept_help, element_descriptor, element_descriptors, field_note, help_topic,
    ElementDescriptor, HelpTopic, ParameterDescriptor, ParameterKind,
};
pub use elements::{Element, ElementKind};
pub use field::{
    FieldEvalOptions, FieldType, FlowField, GridDefinition, PanelBody, ScalarField, VectorField,
    MASK_BODY, MASK_FLUID,
};
pub use forces::{
    integrate_forces, surface_points, total_forces, BodyForces, ChordFrame, ReferenceValues,
    Surface, SurfacePoint, TotalForces,
};
pub use panel_kernel::{PanelKernel, PanelPotentials};
pub use streamline::{
    trace, trace_set, SeedStrategy, SeedingConfig, Streamline, StreamlineConfig, Termination,
};
