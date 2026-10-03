//! Element metadata and contextual help, exported to the UI (PRD §7, §33, §56).
//!
//! The React properties panel builds its form **from this metadata** rather
//! than hard-coding a field per element type. Adding a singularity therefore
//! means adding one [`Element`] variant plus one descriptor here, and the UI
//! appears with the right label, symbol, SI unit, slider range and help text.
//! That is what PRD §7's "treat elementary solutions as plugins, not
//! hard-coded UI" asks for in practice.
//!
//! Help content lives beside the equations it describes so the two cannot
//! drift apart.

use crate::elements::ElementKind;
use crate::field::FieldType;

/// How a parameter should be presented and edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ParameterKind {
    /// A plain number with a slider.
    Scalar,
    /// An angle; the UI shows degrees and converts to radians.
    Angle,
    /// An `x` coordinate.
    PositionX,
    /// A `y` coordinate.
    PositionY,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ParameterDescriptor {
    /// Serialised field name on the [`crate::elements::Element`] variant.
    pub key: &'static str,
    pub label: &'static str,
    /// Mathematical symbol, rendered next to the input.
    pub symbol: &'static str,
    /// SI unit, or `"°"` for angles presented in degrees.
    pub unit: &'static str,
    pub kind: ParameterKind,
    /// Suggested slider bounds. Typed input is never clamped to these.
    pub soft_min: f64,
    pub soft_max: f64,
    pub step: f64,
    pub default: f64,
    pub help: &'static str,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ElementDescriptor {
    pub kind: ElementKind,
    /// Stable string tag matching the serde discriminant.
    pub tag: &'static str,
    pub name: &'static str,
    /// Single glyph used in the scene tree.
    pub glyph: &'static str,
    pub summary: &'static str,
    pub parameters: &'static [ParameterDescriptor],
    pub help: HelpTopic,
}

/// Contextual explanation, structured as PRD §56 requires.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct HelpTopic {
    pub id: &'static str,
    pub title: &'static str,
    /// What it means.
    pub meaning: &'static str,
    /// Why it matters.
    pub importance: &'static str,
    /// Mathematical definition, as LaTeX-free plain text with Unicode symbols.
    pub definition: &'static str,
}

const POS_X: ParameterDescriptor = ParameterDescriptor {
    key: "position.x",
    label: "Position X",
    symbol: "x",
    unit: "m",
    kind: ParameterKind::PositionX,
    soft_min: -10.0,
    soft_max: 10.0,
    step: 0.01,
    default: 0.0,
    help: "Horizontal location in the domain. Positive is to the right.",
};

const POS_Y: ParameterDescriptor = ParameterDescriptor {
    key: "position.y",
    label: "Position Y",
    symbol: "y",
    unit: "m",
    kind: ParameterKind::PositionY,
    soft_min: -10.0,
    soft_max: 10.0,
    step: 0.01,
    default: 0.0,
    help: "Vertical location in the domain. Positive is upward.",
};

static UNIFORM_PARAMS: &[ParameterDescriptor] = &[
    ParameterDescriptor {
        key: "velocity",
        label: "Speed",
        symbol: "U",
        unit: "m/s",
        kind: ParameterKind::Scalar,
        soft_min: -50.0,
        soft_max: 50.0,
        step: 0.1,
        default: 5.0,
        help: "Speed of this additional uniform stream. It is superposed on the global freestream.",
    },
    ParameterDescriptor {
        key: "direction",
        label: "Direction",
        symbol: "θ",
        unit: "°",
        kind: ParameterKind::Angle,
        soft_min: -180.0,
        soft_max: 180.0,
        step: 1.0,
        default: 0.0,
        help: "Flow direction, measured counter-clockwise from the +x axis.",
    },
];

static SOURCE_PARAMS: &[ParameterDescriptor] = &[
    POS_X,
    POS_Y,
    ParameterDescriptor {
        key: "strength",
        label: "Strength",
        symbol: "Λ",
        unit: "m²/s",
        kind: ParameterKind::Scalar,
        soft_min: 0.0,
        soft_max: 50.0,
        step: 0.1,
        default: 5.0,
        help: "Volumetric flow rate emitted per unit span. Larger values push the surrounding flow further aside.",
    },
];

static SINK_PARAMS: &[ParameterDescriptor] = &[
    POS_X,
    POS_Y,
    ParameterDescriptor {
        key: "strength",
        label: "Strength",
        symbol: "Λ",
        unit: "m²/s",
        kind: ParameterKind::Scalar,
        soft_min: 0.0,
        soft_max: 50.0,
        step: 0.1,
        default: 5.0,
        help: "Volumetric flow rate absorbed per unit span. Stored internally as a negative source strength.",
    },
];

static VORTEX_PARAMS: &[ParameterDescriptor] = &[
    POS_X,
    POS_Y,
    ParameterDescriptor {
        key: "circulation",
        label: "Circulation",
        symbol: "Γ",
        unit: "m²/s",
        kind: ParameterKind::Scalar,
        soft_min: -50.0,
        soft_max: 50.0,
        step: 0.1,
        default: 5.0,
        help: "Positive is counter-clockwise. A clockwise circulation (negative Γ) is what produces positive lift: L = −ρU∞Γ.",
    },
];

static DOUBLET_PARAMS: &[ParameterDescriptor] = &[
    POS_X,
    POS_Y,
    ParameterDescriptor {
        key: "strength",
        label: "Strength",
        symbol: "κ",
        unit: "m³/s",
        kind: ParameterKind::Scalar,
        soft_min: -100.0,
        soft_max: 100.0,
        step: 0.1,
        default: 10.0,
        help: "Doublet strength. With the axis aligned to the flow, κ = 2πU∞a² makes the streamlines close around a circle of radius a.",
    },
    ParameterDescriptor {
        key: "orientation",
        label: "Axis",
        symbol: "β",
        unit: "°",
        kind: ParameterKind::Angle,
        soft_min: -180.0,
        soft_max: 180.0,
        step: 1.0,
        default: 0.0,
        help: "Direction of the doublet axis. Align it with the freestream to form a circular body.",
    },
];

/// Metadata for every MVP element, in the order the Add menu should list them.
pub fn element_descriptors() -> &'static [ElementDescriptor] {
    static DESCRIPTORS: &[ElementDescriptor] = &[
        ElementDescriptor {
            kind: ElementKind::UniformFlow,
            tag: "uniformFlow",
            name: "Uniform Flow",
            glyph: "→",
            summary: "A constant stream superposed on the freestream.",
            parameters: UNIFORM_PARAMS,
            help: HelpTopic {
                id: "uniformFlow",
                title: "Uniform flow",
                meaning: "Fluid moving everywhere at the same speed and direction. It is the simplest potential flow and the background on which everything else is built.",
                importance: "Every aerodynamic problem starts with a freestream. Adding a second uniform stream is the clearest way to see that potential flows superpose: the velocities simply add.",
                definition: "u = U·cos θ,  v = U·sin θ.  φ = U(x cos θ + y sin θ),  ψ = U(y cos θ − x sin θ).",
            },
        },
        ElementDescriptor {
            kind: ElementKind::Source,
            tag: "source",
            name: "Source",
            glyph: "◉",
            summary: "Emits fluid radially at a fixed volumetric rate.",
            parameters: SOURCE_PARAMS,
            help: HelpTopic {
                id: "source",
                title: "Point source",
                meaning: "A point from which fluid flows radially outward at a fixed volumetric rate per unit span. The velocity is purely radial and falls off as 1/r.",
                importance: "Sources are how a panel method represents thickness. A source placed in a freestream creates a half-body; a source/sink pair creates a closed oval. Both show how solid shapes emerge from singularities with no wall anywhere in the mathematics.",
                definition: "V_r = Λ/(2πr),  V_θ = 0.  φ = (Λ/2π)·ln r,  ψ = (Λ/2π)·θ.  The flux through any circle enclosing the source is exactly Λ.",
            },
        },
        ElementDescriptor {
            kind: ElementKind::Sink,
            tag: "sink",
            name: "Sink",
            glyph: "◎",
            summary: "Absorbs fluid radially at a fixed volumetric rate.",
            parameters: SINK_PARAMS,
            help: HelpTopic {
                id: "sink",
                title: "Point sink",
                meaning: "A source with the sign reversed: fluid flows radially inward. It is listed separately because thinking in terms of 'a sink' is more natural than 'a negative source'.",
                importance: "A source and an equal sink in a freestream produce a Rankine oval — a genuinely closed body. Moving them together is the limit that defines the doublet, and hence the cylinder.",
                definition: "V_r = −Λ/(2πr).  Identical to a source of strength −Λ.",
            },
        },
        ElementDescriptor {
            kind: ElementKind::Vortex,
            tag: "vortex",
            name: "Vortex",
            glyph: "↻",
            summary: "Circulates fluid about a point; the origin of lift.",
            parameters: VORTEX_PARAMS,
            help: HelpTopic {
                id: "vortex",
                title: "Point vortex",
                meaning: "Fluid circulating about a point with purely tangential velocity falling off as 1/r. The flow is irrotational everywhere except at the singular point itself.",
                importance: "Circulation is lift. The Kutta–Joukowski theorem makes that exact: a body with bound circulation Γ in a stream U∞ experiences lift ρU∞Γ per unit span, whatever its shape. Adding a vortex to a cylinder's flow tilts the stagnation points and generates lift with no change of geometry — the Magnus effect.",
                definition: "V_θ = Γ/(2πr),  V_r = 0, with Γ > 0 counter-clockwise.  φ = (Γ/2π)·θ,  ψ = −(Γ/2π)·ln r.  Lift follows as L = −ρU∞Γ under this sign convention.",
            },
        },
        ElementDescriptor {
            kind: ElementKind::Doublet,
            tag: "doublet",
            name: "Doublet",
            glyph: "⬭",
            summary: "A source/sink pair in the limit of zero separation.",
            parameters: DOUBLET_PARAMS,
            help: HelpTopic {
                id: "doublet",
                title: "Doublet",
                meaning: "The limit of a source and sink brought together while their strengths grow so that the product κ = Λ·ℓ stays finite. The velocity falls off as 1/r², faster than a source.",
                importance: "A doublet aligned with a uniform stream produces exactly the flow around a circular cylinder — the closed-form solution every panel method is checked against. It is the bridge between abstract singularities and a recognisable body.",
                definition: "φ = (κ/2π)·x/r²,  ψ = −(κ/2π)·y/r².  Combined with a stream U∞ along the axis, κ = 2πU∞a² gives a cylinder of radius a, with surface speed 2U∞·|sin θ| and Cp = 1 − 4sin²θ.",
            },
        },
    ];
    DESCRIPTORS
}

pub fn element_descriptor(kind: ElementKind) -> &'static ElementDescriptor {
    element_descriptors()
        .iter()
        .find(|d| d.kind == kind)
        .expect("every ElementKind has a descriptor")
}

/// Help entries for concepts that are not elements (PRD §56).
pub fn concept_help() -> &'static [HelpTopic] {
    static TOPICS: &[HelpTopic] = &[
        HelpTopic {
            id: "panelMethod",
            title: "Panel method",
            meaning: "A way of solving flow around an arbitrary shape by covering its surface with small straight segments — panels — each carrying an unknown singularity strength. Requiring that no fluid passes through any panel gives one equation per panel, and solving that linear system gives the whole flow field.",
            importance: "It turns 'solve a partial differential equation in the whole plane' into 'solve a few hundred linear equations on the boundary'. That is why a panel method runs in milliseconds in a browser while a mesh-based CFD solver does not.",
            definition: "This solver uses the Hess–Smith formulation: constant-strength source panels (one σⱼ per panel) plus one uniform vortex strength γ per body. The unknowns satisfy N flow-tangency conditions V·n̂ = 0 at the panel midpoints, plus one Kutta condition per lifting body.",
        },
        HelpTopic {
            id: "kuttaCondition",
            title: "Kutta condition",
            meaning: "A physical requirement, added by hand, that the flow leave a sharp trailing edge smoothly instead of whipping around it at infinite speed.",
            importance: "Potential flow alone does not determine how much circulation a body carries — any value satisfies the equations. Without extra information an airfoil would have no definite lift. The Kutta condition is what viscosity does in reality, expressed as a single equation, and it is what makes lift computable.",
            definition: "Here it is imposed as equal and opposite tangential velocity on the two panels adjacent to the trailing edge: V_t,1 + V_t,N = 0. That makes the pressures on the upper and lower surface match at the trailing edge.",
        },
        HelpTopic {
            id: "circulation",
            title: "Circulation",
            meaning: "The line integral of velocity around a closed loop: a measure of net 'swirl' enclosed by that loop.",
            importance: "Circulation is the single number that determines lift in two-dimensional inviscid flow. Two very different shapes with the same bound circulation generate the same lift.",
            definition: "Γ = ∮ V·dl, positive counter-clockwise. For a solved body, Γ = γ·perimeter. Kutta–Joukowski then gives L = −ρU∞Γ under this sign convention.",
        },
        HelpTopic {
            id: "cp",
            title: "Pressure coefficient",
            meaning: "Pressure expressed as a dimensionless number, so that results can be compared between different speeds, fluids and sizes.",
            importance: "Cp is the standard currency of aerodynamic data. Cp = 1 marks a stagnation point; Cp = 0 means freestream pressure; strongly negative Cp means locally fast, low-pressure flow — which is where lift comes from and where a real boundary layer is most likely to separate.",
            definition: "Cp = (p − p∞)/(½ρU∞²). For steady incompressible inviscid flow this reduces to Cp = 1 − (V/U∞)². It cannot be formed when U∞ = 0.",
        },
        HelpTopic {
            id: "lift",
            title: "Lift",
            meaning: "The component of the integrated surface pressure force perpendicular to the freestream.",
            importance: "Potential flow predicts lift well for attached flow at modest angles of attack, which is why panel methods remain in use for preliminary design. It is computed here two independent ways — pressure integration and Kutta–Joukowski — and the solver reports how closely they agree.",
            definition: "L = −∮(p − p∞)·n̂ ds, then rotated into wind axes: L = −Fx·sin α + Fy·cos α. Per unit span, so the unit is N/m.",
        },
        HelpTopic {
            id: "drag",
            title: "Drag (and why it is nearly zero)",
            meaning: "The component of the integrated pressure force parallel to the freestream.",
            importance: "In steady inviscid potential flow around a closed body the pressure drag is exactly zero — d'Alembert's paradox. The small non-zero value reported here is purely discretisation error, not a physical prediction. Real drag comes from skin friction and boundary-layer separation, neither of which this model contains.",
            definition: "D = Fx·cos α + Fy·sin α. Treat a computed CD as a numerical-accuracy indicator, not as an aerodynamic result.",
        },
        HelpTopic {
            id: "elementForces",
            title: "Forces on singularities (Lagally theorem)",
            meaning: "A source, sink, vortex or doublet has no surface to integrate pressure over, yet it still exchanges momentum with the flow around it. The Lagally theorem gives that force from the velocity induced at the singularity by everything else — the freestream, the other elements and the bodies.",
            importance: "It shows where lift comes from without any body at all: a clockwise vortex in a stream is pushed upwards by exactly ρU∞|Γ|. It also shows that forces come in equal and opposite pairs — a source near a body is pulled towards it while the body is pulled towards the source. Read the arrows as the force needed to hold each singularity fixed; a free vortex or source in a real fluid would instead be carried along by the flow.",
            definition: "With V the velocity induced at the element by everything else: vortex F = ρ V × Γẑ = ρΓ(V_y, −V_x); source F = −ρΛV (sink: +ρΛV); doublet F = ρκ (ê·∇)V with ê the doublet axis. A uniform stream has no location and no force. Per unit span, N/m.",
        },
        HelpTopic {
            id: "streamlines",
            title: "Streamlines",
            meaning: "Curves everywhere tangent to the velocity field. In a steady flow they are the paths fluid particles actually follow.",
            importance: "Streamlines are the fastest way to read a flow: where they crowd together the flow is fast and the pressure low; where they spread apart it is slow. A closed streamline is a body surface, which is how singularities turn into shapes.",
            definition: "dx/ds = u/|V|, dy/ds = v/|V|. In potential flow streamlines coincide with contours of the stream function ψ, but this solver integrates the velocity field directly so that sources (whose ψ is multivalued) and bodies are handled robustly.",
        },
        HelpTopic {
            id: "vorticity",
            title: "Vorticity",
            meaning: "Local rotation rate of the fluid: twice the angular velocity of an infinitesimal fluid element.",
            importance: "Potential flow is irrotational *by construction*, so vorticity is exactly zero throughout the fluid. All the circulation sits in the singularities — point vortices, and the vortex sheet bound to each body's surface. Seeing that explicitly is useful: it shows that lift in this model comes entirely from concentrated circulation, not from distributed rotation.",
            definition: "ω_z = ∂v/∂x − ∂u/∂y. For display, each point vortex is shown with a finite core of radius a, giving ω = Γa²/(π(r² + a²)²) — a distribution whose integral over the plane is exactly Γ.",
        },
        HelpTopic {
            id: "assumptions",
            title: "Model assumptions",
            meaning: "The flow is steady, two-dimensional, incompressible, inviscid and irrotational except at explicitly placed vortices, with constant density.",
            importance: "These assumptions are what make the problem solvable in milliseconds, and they are also the limits of the answer. No boundary layer means no friction drag, no separation, no stall, and no maximum lift. Compressibility is absent, so results drift from reality above roughly Mach 0.3.",
            definition: "∇²φ = 0 with V = ∇φ, subject to V·n̂ = 0 on every body and V → U∞ at infinity.",
        },
    ];
    TOPICS
}

pub fn help_topic(id: &str) -> Option<&'static HelpTopic> {
    concept_help().iter().find(|t| t.id == id).or_else(|| {
        element_descriptors()
            .iter()
            .map(|d| &d.help)
            .find(|t| t.id == id)
    })
}

/// Short note shown next to a field selector.
pub fn field_note(field: FieldType) -> &'static str {
    match field {
        FieldType::VelocityMagnitude => "Flow speed |V|. Crowded streamlines and high speed go together.",
        FieldType::VelocityU => "Streamwise velocity component along +x.",
        FieldType::VelocityV => "Transverse velocity component along +y.",
        FieldType::Pressure => "Static pressure from Bernoulli's equation; the ambient p∞ is included.",
        FieldType::Cp => "Dimensionless pressure. Cp = 1 at a stagnation point, 0 in the freestream.",
        FieldType::Vorticity => "Exactly zero in the fluid: potential flow is irrotational. Shown with finite vortex cores so the circulation is visible.",
        FieldType::Potential => "Velocity potential φ, with V = ∇φ. Equipotentials cross streamlines at right angles.",
        FieldType::StreamFunction => "Stream function ψ. Its contours are streamlines; it jumps across a branch cut when net source strength is non-zero.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_element_kind_has_a_descriptor() {
        for kind in ElementKind::ALL {
            let d = element_descriptor(kind);
            assert_eq!(d.kind, kind);
            assert_eq!(
                d.tag,
                kind.as_str(),
                "tag must match the serde discriminant"
            );
            assert!(!d.name.is_empty() && !d.summary.is_empty());
            assert!(!d.parameters.is_empty());
        }
        assert_eq!(element_descriptors().len(), ElementKind::ALL.len());
    }

    #[test]
    fn every_parameter_is_fully_specified() {
        for d in element_descriptors() {
            for p in d.parameters {
                assert!(!p.key.is_empty(), "{} has an unnamed parameter", d.name);
                assert!(!p.label.is_empty());
                assert!(!p.symbol.is_empty());
                assert!(!p.unit.is_empty());
                assert!(!p.help.is_empty());
                assert!(p.soft_min < p.soft_max, "{}/{}: bad range", d.name, p.key);
                assert!(p.step > 0.0);
                assert!(
                    p.default >= p.soft_min && p.default <= p.soft_max,
                    "{}/{}: default {} outside [{}, {}]",
                    d.name,
                    p.key,
                    p.default,
                    p.soft_min,
                    p.soft_max
                );
            }
        }
    }

    #[test]
    fn angle_parameters_are_declared_in_degrees() {
        for d in element_descriptors() {
            for p in d.parameters {
                if p.kind == ParameterKind::Angle {
                    assert_eq!(p.unit, "°", "{}/{} must present degrees", d.name, p.key);
                }
            }
        }
    }

    #[test]
    fn positioned_elements_expose_position_parameters() {
        for kind in [
            ElementKind::Source,
            ElementKind::Sink,
            ElementKind::Vortex,
            ElementKind::Doublet,
        ] {
            let d = element_descriptor(kind);
            assert!(d.parameters.iter().any(|p| p.key == "position.x"));
            assert!(d.parameters.iter().any(|p| p.key == "position.y"));
        }
        // Uniform flow has no location.
        let u = element_descriptor(ElementKind::UniformFlow);
        assert!(!u.parameters.iter().any(|p| p.key.starts_with("position")));
    }

    #[test]
    fn help_topics_follow_the_required_three_part_structure() {
        let all: Vec<&HelpTopic> = concept_help()
            .iter()
            .chain(element_descriptors().iter().map(|d| &d.help))
            .collect();
        assert!(all.len() >= 14, "only {} topics", all.len());
        for t in all {
            assert!(!t.id.is_empty() && !t.title.is_empty());
            assert!(t.meaning.len() > 40, "{} meaning too thin", t.id);
            assert!(t.importance.len() > 40, "{} importance too thin", t.id);
            assert!(t.definition.len() > 20, "{} definition too thin", t.id);
        }
    }

    #[test]
    fn required_help_ids_are_present() {
        // PRD §56 lists the topics that must have contextual help.
        for id in [
            "source",
            "sink",
            "vortex",
            "doublet",
            "circulation",
            "panelMethod",
            "kuttaCondition",
            "cp",
            "lift",
            "drag",
            "streamlines",
            "vorticity",
        ] {
            assert!(help_topic(id).is_some(), "missing help topic: {id}");
        }
        assert!(help_topic("nonexistent").is_none());
    }

    #[test]
    fn help_ids_are_unique() {
        let mut ids: Vec<&str> = concept_help()
            .iter()
            .map(|t| t.id)
            .chain(element_descriptors().iter().map(|d| d.help.id))
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate help ids");
    }

    #[test]
    fn every_field_type_has_a_note() {
        for f in FieldType::ALL {
            assert!(field_note(f).len() > 20, "{f:?} note too thin");
        }
    }

    #[test]
    fn drag_help_states_the_inviscid_limitation() {
        // PRD §25 requires this warning to exist and be unambiguous.
        let t = help_topic("drag").unwrap();
        let text = format!("{} {}", t.meaning, t.importance).to_lowercase();
        assert!(text.contains("d'alembert"));
        assert!(text.contains("skin friction") || text.contains("friction"));
        assert!(text.contains("separation"));
    }

    #[test]
    fn vortex_help_states_the_sign_convention() {
        let t = help_topic("vortex").unwrap();
        assert!(t.definition.contains("counter-clockwise"));
        assert!(t.definition.contains("−ρU∞Γ") || t.definition.contains("L = −ρU∞Γ"));
    }
}
