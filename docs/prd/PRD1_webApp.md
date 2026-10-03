# 2D Potential Flow Simulator – Product Requirements
## Overview
We will build an interactive web application that solves incompressible, irrotational (potential) 2D flow around user-defined objects. The user can create flows by placing elementary flow sources (point source/sink, vortex, uniform stream, doublet, etc.) or by importing custom shapes (via an (x,y) coordinate file, e.g. an airfoil cross-section). Elementary flows have known analytic solutions and can be superposed to form complex flows. For imported shapes, the app will use a panel method: discretize the boundary into small panels and solve Laplace’s equation for the potential. All heavy computation (solving the flow) will run in WebAssembly (WASM) for speed, allowing interactive updates as the user edits the scene.

The interface will be dynamic and modern: users can add, drag, rotate, or delete any object in the 2D domain and immediately see updated results. Visualization modes include velocity field, streamlines, pressure contours, vorticity, and other aerodynamic fields. Classical aerodynamic outputs (pressure distribution, pressure coefficient, lift/drag forces and coefficients) will be computed and displayed for each object and in total. The UI will follow best practices for scientific apps (e.g. sidepanels for inputs/controls and plots) to be friendly and professional. For example, NASA’s FoilSim uses separate view, control, input and output panels; we will adopt a similar clean layout. All user interactions (adding objects, switching views, zoom/pan, etc.) will be intuitive and responsive, with smooth graphics (leveraging WebGL where appropriate).

## Functional Requirements
Add Elementary Flow Elements: Users can insert flow elements such as point sources, sinks, vortices, uniform streams, or doublets. Each element has editable parameters (strength, orientation, position). The flow field is the superposition of these elements (valid since potential flows superpose).

Import Custom Shape (Panel Method): Users can upload a 2D body defined by coordinates (e.g. a CSV of (x,y) points). This body’s boundary is discretized into panels (line segments). The app solves Laplace’s equation using the panel method. In practice, constant-strength sources or vortices are placed on each panel, and linear equations enforce the no-penetration condition on the body surface. The Kutta condition (smooth flow at a sharp trailing edge) is applied for lifting shapes. Once solved, the velocity on each panel is known.

Interactive Object Editing: Any number of objects (elements or imported bodies) can coexist. The user can select and drag objects on the canvas. Objects have handles or controls for moving, rotating, scaling, or deleting. Editing a parameter (e.g. moving a source) triggers an immediate re-solve of the flow. This requires a highly interactive UI (e.g. HTML5 canvas/SVG with drag-and-drop). Multiple bodies influence each other (multi-body flow), which the solver must handle.

Visualization Modes: The user can toggle different views of the computed flow:

Velocity Field: Show a vector plot or color map of velocity magnitude (|v|).
Streamlines: Compute and draw streamlines (curves everywhere tangent to the velocity). Streamlines in potential flow are constant-ψ curves. We may animate particles along streamlines for effect (GPU-based tracing at high FPS).
Pressure Field: Using Bernoulli’s equation, compute static pressure everywhere. Display as a color contour (or grayscale) on the domain or object.
Pressure Coefficient (Cp): Compute Cp = (p–p∞)/(½ρU∞²) at sampled points. Show Cp on surfaces or as a field.
Vorticity: Compute the vorticity ω = ∇×v. In ideal potential flow ω=0 everywhere except at inserted vortices, so this view will highlight only those regions.
Surface Plots: On each body, plot pressure or Cp vs. surface coordinate (upper vs. lower surface), similar to FoilSim’s surface plots.
Other Fields: Optionally equipotential lines or stream function contours.
Aerodynamic Metrics and Outputs:

Forces (Lift & Drag): Integrate pressure on each panel of a body: (L = \sum p,n_y ,\Delta s), (D = \sum p,n_x,\Delta s) (where $(n_x,n_y)$ is the panel normal). Display lift and drag force (per unit span) for each object and total.
Coefficients: Compute lift coefficient (C_L = L/(0.5ρU^2 c)) and drag coefficient (C_D = D/(0.5ρU^2 c)), where $c$ is a reference length (e.g. chord). Show $C_L,C_D$ numerically.
Plots: Provide common aerodynamics plots, e.g. drag polar (plot of $C_L$ vs $C_D$ over AoA), and lift/drag vs angle-of-attack or other parameters. These help users understand performance trends.
Pressure Coefficient: Plot Cp distributions as line graphs (upper vs lower surface).
User Interface Features:

Sidebar Controls: Panels with buttons to add elements, select visualization mode, and input parameters (freestream speed $U_\infty$, density ρ, etc.).
Object Properties Panel: When an object is selected, show its properties (strength, angle, etc.) in a form for editing. Changes update the flow.
Zoom/Pan: The user can zoom and pan the view window (similar to FoilSim’s view).
Interactive Legend/Help: Tooltips or legend explaining colors and arrows. Help overlay for new users.
Dynamic Updates: All changes (adding/moving objects or sliders) immediately recalc and redraw. This requires fast computation (WASM).
Performance and Responsiveness: The solver must run quickly enough for interactive use. Target re-compute times on the order of tens to hundreds of milliseconds for typical problem sizes, so users experience near real-time updates. Large grids or many panels may run asynchronously (Web Worker) to keep the UI fluid.

## Technical Architecture
WebAssembly Solver: The core flow solver will be written in a high-performance language (Rust or C++) and compiled to WASM. For elemental flows, velocity is computed from closed-form formulas. For shapes, use a boundary-element panel method solver. The architecture can follow examples like FlexFoil: a Rust panel-solver core with WASM bindings and a React front-end. FlexFoil even targets 60 Hz updates for airfoil manipulation, so similar performance is feasible.

Data Flow: Objects and parameters are maintained in JavaScript. When the scene changes, the JS code packages object data (panel coordinates or element lists) and calls the WASM solver. WASM returns fields (e.g. velocity vectors at sample points or panel velocities). JS then renders the results. Memory can be shared via HEAPF32 buffers (as in other WASM CFD demos).

Panel Method Solver: Solve Laplace’s equation on each body: discretize into N panels. Solve the linear system for panel singularity strengths. Enforce Kutta and far-field conditions. Compute velocities on panels/points. Use efficient solvers (LU or iterative). This is a “boundary element” approach. We can leverage libraries or write custom code. Example code from XFoil-like solvers may be ported.

Visualization Engine: Use HTML5 Canvas or WebGL to draw fields. For vector/contour fields, WebGL shaders (via regl, Three.js, or raw WebGL) can render quickly. For example, a GPU-based particle tracer (like Deltares’ streamline visualizer) can animate streamlines smoothly. 2D graphs (plots of Cp, drag polars, etc.) can use a charting library or custom canvas.

UI Framework: A modern JS framework (e.g. React, Vue) will manage components. FlexFoil uses React+TypeScript for its UI – we could follow suit. UI components (buttons, sliders, dialogs) should have a clean design (Material, Ant Design, or TailwindCSS).

File I/O: Accept standard coordinate file formats (CSV, .dat). After upload, parse the file into point arrays. Optionally smooth or spline the points, then generate panels along the curve.

Threading: If solver time grows, use Web Workers to run WASM off the main thread. Meanwhile, the UI can show a loading indicator or continue accepting UI actions.

## Aerodynamic Calculations and Visualization
Bernoulli & Pressure: From the computed velocity field, use Bernoulli’s equation (constant total pressure) to get static pressure everywhere. Then compute the pressure coefficient $C_p = (p - p_\infty)/(½ρU_\infty^2)$. Show Cp on bodies. In ideal potential flow, $C_p = 1 - (u/U_\infty)^2$ (for incompressible flow).

Forces (Lift/Drag): Integrate pressure on each panel: $$L = \sum (p - p_\infty),n_y,\Delta s,\quad D = \sum (p - p_\infty),n_x,\Delta s,$$ where $(n_x,n_y)$ is the outward normal. In inviscid theory (potential flow), total drag on a closed body is theoretically zero (d’Alembert’s paradox). In practice, asymmetrical pressure distributions or prescribed circulation yield nonzero lift. We will report the computed pressure forces as “lift” and “drag” (knowing drag may be nearly zero without viscosity). This approach matches panel solvers which compute lift via pressure.

Coefficients and Plots: Compute and display dimensionless coefficients ($C_L,C_D$). Provide plots of lift and drag vs. angle of attack or element strength. Provide a drag polar (plot $C_L$ vs $C_D$ over a range). Also chart surface pressure/velocity along bodies (upper/lower surfaces). These are standard outputs in foil analysis tools.

Streamlines/Vorticity: Solve for stream function $\psi$ or numerically integrate streamlines: $\frac{dx}{ds}=u,\frac{dy}{ds}=v$. A high-quality solution (smooth curves) helps interpret flow. For vortices, compute local circulation. Optionally display “vorticity magnitude” (mostly zero except at vortices).

## User Interface Design
Layout & Controls: We will split the screen into panels (similar to FoilSim): a View Window (main canvas) showing geometry and flow, a Control Panel (top or side) with buttons for modes and adding objects, an Input Panel (side or bottom) for numeric parameters, and an Output Panel for graphs and results. This familiar layout makes it easy to switch views and see corresponding controls.

Interactivity Patterns:

Adding objects: Toolbar icons (“Add Source”, “Add Vortex”, “Import Shape”) that let the user place objects in the view.
Selection: Clicking an object highlights it and shows its properties in the input panel.
Dragging: Click-and-drag moves or reorients an object. Zoom and pan via mouse wheel and drag on background. (See NASA FoilSim: dragging objects and zooming is supported.)
Sliders/Inputs: Use sliders for continuous values (e.g. strength). Numeric input fields allow precise entry.
Mode Toggle: Buttons or tabs to switch visual mode (e.g. velocity vs pressure vs streamlines).
Visualization Style: Use smooth color gradients and vector arrows. For example, color velocity magnitude from blue (low) to red (high). Streamlines or particles colored by speed or time. Overlays should not obscure geometry: allow changing opacity.

Aesthetic: Adopt a clean, flat modern style. Contrasting colors for fields vs. background. Animations (like launching a new particle) should be subtle. The UI should feel “refreshing” and professional, with consistent fonts/icons. Provide light/dark themes if possible.

## Technology Stack & Tools
Languages/Frameworks: JavaScript/TypeScript for the UI (React/Vue). Rust for the flow solver (compiled to WASM). WASM binds to JS via Emscripten or wasm-bindgen (as FlexFoil does).
Graphics: HTML5 Canvas and/or WebGL for drawing. Use WebGL for heavy rendering (e.g. dynamic streamlines with many particles). For static graphs (Cp plots, polars), a 2D plotting library or custom canvas code.
Libraries: Consider math libraries for linear algebra (solver), although custom code may suffice. UI libraries (e.g. React + Material-UI) for widgets. Testing: use known analytical cases (flow past cylinder, uniform flow) to verify correctness.
References
Key principles and examples inform these requirements. Potential flow theory and superposition are standard. Panel-method basics (Laplace eq, discretized panels, Kutta condition) are summarized by ANSYS. Pressure coefficient definition is $C_p=(p-p_\infty)/(½ρU^2)$. D’Alembert’s paradox (inviscid drag=0) is noted in literature. For UI design, NASA’s FoilSim provides a well-known example (view/control/output layout and plots). WebAssembly-based CFD demos (e.g. FlexFoil, Lattice-Boltzmann in WASM) show real-time browser computation is feasible. GPU-accelerated streamline visualization (Deltares WebGL Streamline) illustrates how to animate flow traces efficiently. All these guide the architecture and features of our web app.


# Product Requirements Document (PRD)

# 2D Potential Flow Simulator

**Document status:** Draft — implementation-ready
**Audience:** Agentic software-engineering swarm, product/design agents, frontend agents, numerical-methods agents, WASM agents, QA agents
**Product type:** Browser-based scientific simulation and visualization application
**Primary domain:** 2D incompressible potential flow / introductory computational aerodynamics

---

## 1. Product Definition

### 1.1 Product name

**Working name:** `AeroFlow`
**Note:** The implementation swarm may rename the product if a final brand is supplied.

### 1.2 One-sentence description

A professional browser-based interactive environment for constructing, solving, and visualizing 2D potential-flow problems using analytical elementary solutions and panel methods, with high-performance numerical computation executed through WebAssembly.

### 1.3 Product vision

The application should make 2D potential flow feel like an interactive engineering instrument rather than a traditional calculator.

A user should be able to:

1. Open the application.
2. Add a freestream, source, sink, vortex, doublet, or custom body.
3. Drag the objects around the domain.
4. Change their parameters.
5. Immediately see the flow respond.
6. Switch between velocity, streamlines, pressure, vorticity, and aerodynamic views.
7. Select an individual body and inspect its forces and aerodynamic coefficients.
8. Combine multiple bodies and flow elements.
9. Import an arbitrary `(x,y)` geometry and have it solved using a panel method.
10. Export the resulting geometry, solution, plots, and simulation configuration.

The experience should sit conceptually between an educational aerodynamics simulator and a lightweight professional engineering analysis tool.

---

# 2. Product Goals

## 2.1 Primary goals

| ID    | Goal                                                                                   |
| ----- | -------------------------------------------------------------------------------------- |
| G-001 | Provide an interactive 2D potential-flow simulation environment.                       |
| G-002 | Support analytical elementary flow solutions.                                          |
| G-003 | Support arbitrary user-defined closed geometries through panel methods.                |
| G-004 | Make simulation interaction sufficiently fast to feel real-time.                       |
| G-005 | Execute computationally expensive numerical operations in WebAssembly.                 |
| G-006 | Provide professional scientific visualizations.                                        |
| G-007 | Provide classical aerodynamic quantities and force calculations.                       |
| G-008 | Make numerical assumptions and limitations explicit to users.                          |
| G-009 | Provide a clean architecture suitable for future numerical solvers.                    |
| G-010 | Make the codebase modular enough for multiple development agents to work concurrently. |

---

# 3. Non-Goals

The initial product must **not** attempt to become a general CFD package.

### Explicitly out of scope for MVP

* Navier-Stokes CFD
* Turbulence modelling
* Compressible flow
* Shock waves
* Heat transfer
* 3D geometry
* Full 3D panel methods
* Unsteady viscous flow
* Boundary-layer solving
* Fluid-structure interaction
* Mesh-based finite-volume CFD
* GPU-based general-purpose CFD
* Automatic aerodynamic optimisation
* Structural analysis

Potential flow is inviscid and cannot represent boundary-layer physics; consequently, the product must not present its drag predictions as equivalent to viscous CFD or experimental aerodynamic drag. Potential flow is nevertheless valuable for understanding flow fields and lift, and for educational and preliminary aerodynamic analysis.

---

# 4. Target Users

## 4.1 Primary persona — Engineering student

Needs to understand:

* sources and sinks
* superposition
* circulation
* streamlines
* pressure distribution
* airfoil flow
* lift
* pressure coefficient
* panel methods

The interface should teach through interaction rather than requiring the user to understand the mathematics before using the application.

---

## 4.2 Secondary persona — Aerospace student/researcher

Wants to:

* import NACA or custom airfoils
* investigate angle of attack
* compare pressure distributions
* inspect Cp
* calculate lift
* investigate circulation
* compare panel resolutions
* export numerical results

---

## 4.3 Secondary persona — Engineer

Wants a fast exploratory tool for:

* conceptual analysis
* sanity checks
* visualisation
* educational demonstrations
* preliminary geometry comparisons

The application should expose numerical details rather than hiding the solver behind a purely visual interface.

---

# 5. Core Product Concepts

The application consists of five major concepts:

```text
                 ┌────────────────────┐
                 │    Simulation       │
                 │      Scene         │
                 └─────────┬──────────┘
                           │
          ┌────────────────┼─────────────────┐
          │                │                 │
          ▼                ▼                 ▼
   Elementary          Custom Bodies     Freestream
   Solutions           / Airfoils        Conditions
          │                │                 │
          └────────────────┼─────────────────┘
                           ▼
                  ┌─────────────────┐
                  │  WASM Solver    │
                  └────────┬────────┘
                           ▼
                  ┌─────────────────┐
                  │ Solution State  │
                  └────────┬────────┘
                           ▼
       ┌─────────────┬─────┼──────┬─────────────┐
       ▼             ▼     ▼      ▼             ▼
   Velocity       Pressure  Cp  Streamlines  Forces
```

---

# 6. Simulation Model

A **Simulation** contains:

```text
Simulation
├── Flow conditions
│   ├── Freestream velocity
│   ├── Freestream angle
│   ├── Density
│   ├── Reference pressure
│   └── Reference temperature [future]
│
├── Elementary elements
│   ├── Source
│   ├── Sink
│   ├── Vortex
│   ├── Doublet
│   └── Uniform flow
│
├── Bodies
│   ├── Imported geometry
│   └── Future generated geometry
│
├── Solver configuration
│   ├── Panel method
│   ├── Panel count
│   ├── Singularity type
│   ├── Kutta condition
│   └── Numerical tolerances
│
└── Visualization configuration
    ├── Active field
    ├── Colormap
    ├── Streamline settings
    ├── Vector settings
    └── Display overlays
```

---

# 7. Elementary Flow Elements

The architecture must treat elementary solutions as plugins/modules rather than hard-coding them into the UI.

## 7.1 Required MVP elements

### Uniform flow

Parameters:

```text
velocity
direction
```

---

### Point source

Parameters:

```text
x
y
strength
```

Strength convention must be documented and consistent.

---

### Point sink

Can internally be represented as a source with negative strength, but the UI should expose it as a separate element because this is more intuitive for users.

---

### Point vortex

Parameters:

```text
x
y
circulation
```

The application must define the positive circulation convention explicitly.

---

### Doublet

Parameters:

```text
x
y
strength
orientation
```

---

# 8. Future Elementary Solutions

The architecture should permit:

* source/sink pairs
* vortex pairs
* dipoles
* rotating cylinders
* source distributions
* custom singularity distributions
* analytic cylinder solutions
* conformal-mapping-based solutions

These should not be required for MVP.

---

# 9. Object Interaction

Every scene object must support:

```text
CREATE
SELECT
MOVE
ROTATE
EDIT
DUPLICATE
HIDE
DELETE
```

Where meaningful.

---

## 9.1 Selection

Clicking an object:

* highlights it
* displays its bounding/interaction handles
* opens its properties
* updates the output panel
* identifies its contribution to the simulation

---

## 9.2 Dragging

Objects can be moved directly on the simulation canvas.

Dragging should:

1. update the object's position continuously;
2. throttle/debounce expensive solves where necessary;
3. update the visual solution;
4. preserve UI responsiveness.

For example:

```text
pointer down
    ↓
object selected
    ↓
pointer move
    ↓
scene state updated
    ↓
solver update
    ↓
visualisation updated
```

---

# 10. Custom Geometry Import

## 10.1 Supported formats

MVP:

* `.csv`
* `.txt`
* `.dat`

Expected representation:

```text
x,y
1.0000,0.0000
0.9000,0.0120
0.5000,0.0500
...
```

The parser should also support whitespace-delimited coordinate files where practical.

---

## 10.2 Import workflow

```text
Upload
  ↓
Parse
  ↓
Validate
  ↓
Preview geometry
  ↓
Geometry configuration
  ↓
Panelisation
  ↓
Solve
```

---

## 10.3 Geometry validation

The importer must detect:

* insufficient points
* NaN values
* infinite values
* duplicate points
* zero-length segments
* self-intersections
* open contours
* extremely small segments
* extremely large coordinate ranges
* inconsistent coordinate scales

The user must receive actionable errors.

Example:

> Geometry contains 2 zero-length panels. Remove duplicate coordinates or enable automatic cleanup.

---

# 11. Geometry Processing

The application should support:

### Automatic closure

If the first and last points are sufficiently close:

```text
distance(p0, pn) < tolerance
```

the geometry can be automatically closed.

Otherwise the user should be asked whether the geometry should be closed.

---

## 11.1 Panel generation

Imported coordinates should be converted into:

```text
Panel
├── start point
├── end point
├── midpoint
├── length
├── tangent
└── outward normal
```

Panel ordering must be validated.

The solver must know whether the geometry is clockwise or counter-clockwise and establish a consistent outward-normal convention.

---

# 12. Panel Method

The panel solver is a core computational subsystem.

The architecture must allow different panel formulations behind a common interface.

## 12.1 MVP solver

Recommended initial implementation:

**2D constant-strength source/vortex panel method with optional circulation/Kutta condition.**

The panel-method implementation must:

1. discretize the body;
2. construct influence coefficients;
3. enforce the no-penetration boundary condition;
4. solve the resulting linear system;
5. apply a Kutta condition for lifting bodies;
6. calculate tangential velocity;
7. calculate pressure coefficient;
8. integrate pressure forces.

Panel methods discretize an aerodynamic surface into panels and solve for singularity strengths; lifting configurations require an appropriate circulation/Kutta treatment.

---

# 13. Multiple Bodies

Multiple custom bodies must be supported.

For:

```text
Body A
Body B
Body C
```

the solver must account for mutual influence.

The implementation must **not** independently solve each body and then simply add the results.

Instead:

```text
all panels
    ↓
global influence matrix
    ↓
global boundary-condition system
    ↓
global solution
```

This is important for multi-body configurations.

---

# 14. Freestream Conditions

Global simulation parameters:

```text
U∞
α
ρ
p∞
```

Minimum MVP:

* velocity magnitude
* angle of attack
* density
* reference pressure

---

## 14.1 Coordinate system

Define globally:

```text
+X = right
+Y = up
positive angle of attack = counter-clockwise
```

This convention must be used consistently throughout the UI, solver and exported data.

---

# 15. Physical Assumptions

The MVP solver assumes:

```text
steady
2D
incompressible
inviscid
irrotational except at explicit vortices
constant density
```

These assumptions must be visible in an "Assumptions" or "Model" section.

---

# 16. Field Calculations

The solver should expose a generic evaluation API:

```text
evaluate_velocity(x, y)
evaluate_pressure(x, y)
evaluate_cp(x, y)
evaluate_stream_function(x, y)
evaluate_potential(x, y)
evaluate_vorticity(x, y)
```

The UI should not need to know how the quantities are computed.

---

# 17. Velocity Field

The user can display:

### Magnitude

```text
|V|
```

### Components

```text
u
v
```

### Vector field

Arrows indicating:

```text
direction
magnitude
```

The visualization should support adjustable vector density.

---

# 18. Streamlines

Streamlines are tangent to the local velocity field.

The application should support:

* automatic streamline seeding
* manual seed placement
* adjustable number of streamlines
* forward/backward integration
* streamline coloring by velocity
* optional animated particles

Lines of constant stream function correspond to streamlines in potential flow.

---

# 19. Pressure

Pressure should be calculated from the potential-flow Bernoulli relationship.

The UI should offer:

```text
Pressure
Pressure coefficient
```

separately.

---

# 20. Pressure Coefficient

Define:

$$
C_p =
\frac{p-p_\infty}
{\frac12\rho_\infty U_\infty^2}
$$

This definition must be used consistently across the solver, graphs and exports.

For incompressible inviscid flow:

$$
C_p = 1-\left(\frac{V}{U_\infty}\right)^2
$$

under the usual steady Bernoulli assumptions.

---

# 21. Vorticity

Display:

$$
\omega_z =
\frac{\partial v}{\partial x}
-
\frac{\partial u}{\partial y}
$$

For ideal potential flow, vorticity is zero away from explicit vortex singularities.

The UI should explain this rather than showing a misleading "empty" visualization.

---

# 22. Additional Classical Views

Architecture should support:

* velocity potential `φ`
* stream function `ψ`
* equipotential lines
* velocity magnitude
* velocity components
* pressure
* `Cp`
* vorticity
* circulation
* surface tangential velocity
* surface pressure
* surface `Cp`
* lift
* drag
* moment
* aerodynamic coefficients

---

# 23. Aerodynamic Forces

For each body, integrate pressure forces over the surface.

The implementation should operate on **pressure relative to freestream pressure** where appropriate:

$$
\Delta p = p-p_\infty
$$

Then integrate:

$$
\mathbf F =
-\oint \Delta p \mathbf n\,ds
$$

The resulting Cartesian components should be reported as:

```text
Fx
Fy
```

and transformed into wind-axis quantities:

```text
Lift
Drag
```

---

# 24. Force Display

For every body:

```text
BODY 01

Lift       +12.41 N/m
Drag        +0.03 N/m
Fx          ...
Fy          ...

CL          0.842
CD          0.002
Cm          ...
```

Also display:

```text
TOTAL

Lift
Drag
Fx
Fy
```

The UI must distinguish:

* force per unit span
* dimensional force
* coefficient

---

# 25. Important Physical Limitation

The application must clearly communicate that ideal inviscid potential flow does not model viscous drag.

For closed bodies in the classical steady potential-flow formulation, d'Alembert's paradox results in zero theoretical drag under the relevant assumptions.

Therefore the product should **not** imply that:

```text
CD ≈ 0
```

is a prediction of real-world aerodynamic drag.

Instead:

> `Potential-flow drag is inviscid pressure drag. Viscous skin friction and boundary-layer separation are not modelled.`

This warning should appear contextually when users inspect drag results.

---

# 26. Aerodynamic Coefficients

For a reference chord `c`:

$$
C_L =
\frac{L}
{\frac12\rho U_\infty^2 c}
$$

$$
C_D =
\frac{D}
{\frac12\rho U_\infty^2 c}
$$

The reference length must be configurable.

For imported geometries, the application should provide:

```text
Reference length
Reference area
Reference point
```

---

# 27. Moment

MVP should calculate:

$$
M_z =
\oint (\mathbf r-\mathbf r_{ref})\times d\mathbf F
$$

with:

```text
reference point
moment
Cm
```

The reference point must be configurable.

---

# 28. Surface Plots

When a body is selected, display:

### Required

* `Cp vs x/c`
* surface velocity vs `x/c`
* pressure vs `x/c`

### Optional MVP

* upper/lower surface differentiation
* force distribution
* panel strength

NASA's FoilSim provides a useful precedent for surface pressure/velocity plots and drag-polar style outputs.

---

# 29. Polar Analysis

The application should support parameter sweeps.

Example:

```text
AoA:
-10°
-8°
-6°
...
+10°
```

For each point:

```text
CL
CD
Cm
```

Generate:

```text
CL vs α
CD vs α
Cm vs α
CL vs CD
```

The sweep should run through the WASM solver rather than blocking the UI thread.

---

# 30. Main User Interface

Recommended structure:

```text
┌──────────────────────────────────────────────────────────────┐
│ Top Navigation / Simulation Controls                         │
├────────────┬─────────────────────────────────┬───────────────┤
│            │                                 │               │
│  Objects   │                                 │  Properties   │
│  / Tools   │       SIMULATION CANVAS         │      /        │
│            │                                 │  Results      │
│            │                                 │               │
│            │                                 │               │
├────────────┴─────────────────────────────────┴───────────────┤
│ Visualization / Plot / Analysis Panel                        │
└──────────────────────────────────────────────────────────────┘
```

The canvas should dominate the interface.

---

# 31. Object Panel

Example:

```text
SCENE

☼ Freestream
○ Source 01
○ Vortex 01
◇ Airfoil 01
◇ Airfoil 02
```

Each object should have:

* visibility toggle
* selection
* name
* type
* optional lock
* delete
* duplicate

---

# 32. Add Object UX

Primary action:

```text
+ Add
```

Menu:

```text
Elementary
────────────
Uniform Flow
Source
Sink
Vortex
Doublet

Geometry
────────────
Import Geometry
Create Geometry [future]
```

---

# 33. Properties Panel

Selected object example:

```text
SOURCE 01

Position
X       [ 1.250 ]
Y       [ 0.000 ]

Strength
Γ       [ 5.000 ]

[Delete]
[Duplicate]
```

Changes should support:

* keyboard input
* arrow keys
* sliders where useful
* reset
* undo

---

# 34. Canvas Interaction

Required:

* pan
* zoom
* select
* drag
* multi-select
* object handles
* grid
* axes
* origin
* scale
* fit-to-view
* reset view

Keyboard shortcuts:

```text
Delete       Delete object
Esc          Clear selection
Space        Pan mode
Ctrl/Cmd+Z   Undo
Ctrl/Cmd+Y   Redo
F            Fit scene
```

---

# 35. Visualization Toolbar

Example:

```text
FIELD

[ Velocity ]
[ Streamlines ]
[ Pressure ]
[ Cp ]
[ Vorticity ]
[ Potential ]
[ Stream Function ]
```

Secondary controls should change according to the selected field.

---

# 36. Dynamic Legend

Every scalar visualization must have:

```text
field name
minimum
maximum
units
color scale
```

Example:

```text
Velocity magnitude

25.0 m/s ───────── 0.0 m/s
```

Color maps should be scientifically appropriate and should not rely exclusively on red/green distinctions.

---

# 37. Modern UI Requirements

The application should feel:

* clean
* technical
* lightweight
* modern
* friendly
* responsive
* visually calm

Avoid:

* excessive gradients
* skeuomorphic controls
* dense spreadsheet-like interfaces
* excessive modal dialogs
* unnecessary animation
* visually noisy dashboards

Use animation primarily to communicate:

* state changes
* transitions
* object movement
* active computation
* particle flow

---

# 38. Responsive Design

Desktop is the primary target.

### Desktop

Full application layout.

### Tablet

Collapsible side panels.

### Mobile

MVP should be functional but not necessarily provide the full desktop workflow.

The application should not require a minimum desktop-only viewport to render correctly.

---

# 39. Themes

Support:

```text
Light
Dark
System
```

The visualization color scheme should remain scientifically interpretable in both modes.

---

# 40. Architecture

Recommended architecture:

```text
┌──────────────────────────────────────────┐
│                React / TS                 │
│                                           │
│  UI       Scene       Visualization       │
│  State    State       Renderer            │
└────────────────────┬─────────────────────┘
                     │
              Solver API
                     │
┌────────────────────▼─────────────────────┐
│                Web Worker                │
│                                           │
│             WASM Interface               │
└────────────────────┬─────────────────────┘
                     │
┌────────────────────▼─────────────────────┐
│                  WASM                    │
│                                           │
│  Elementary Flow Engine                  │
│  Geometry Processing                     │
│  Panel Method                            │
│  Linear Algebra                          │
│  Field Evaluation                        │
│  Aerodynamic Integration                 │
└──────────────────────────────────────────┘
```

---

# 41. Recommended Technology Direction

The implementation swarm should evaluate, but the default recommendation is:

```text
Frontend:
React
TypeScript

State:
Zustand or equivalent lightweight state manager

Styling:
Tailwind CSS or equivalent design system

Rendering:
WebGL/WebGPU-capable rendering abstraction

Charts:
A dedicated scientific charting solution

Numerical core:
Rust

Compilation:
wasm-bindgen / wasm-pack or equivalent

Concurrency:
Web Worker + WASM

Testing:
Vitest/Jest
Playwright
Rust unit/integration tests
Numerical regression tests
```

The exact libraries should be treated as implementation decisions, not product requirements.

A Rust/WASM numerical core with a React frontend is particularly appropriate because an existing open-source project, FlexFoil, demonstrates a similar separation between solver, WASM bindings and React UI.

---

# 42. WASM Boundary

The WASM layer should expose a stable domain-oriented API.

Avoid exposing internal solver data structures directly to JavaScript.

Conceptually:

```typescript
interface FlowSolver {
    createSimulation(config): SimulationHandle;

    setObjects(objects): void;

    solve(): Solution;

    evaluateField(
        field: FieldType,
        grid: GridDefinition
    ): FieldResult;

    getBodyResults(
        bodyId: string
    ): BodyResult;

    runSweep(
        configuration: SweepConfiguration
    ): SweepResult;
}
```

---

# 43. Solver Result Model

The solver should return structured data:

```typescript
interface Solution {
    status: "success" | "warning" | "error";

    fields: {
        velocity?: Field2D;
        pressure?: Field2D;
        cp?: Field2D;
        vorticity?: Field2D;
        potential?: Field2D;
        streamFunction?: Field2D;
    };

    bodies: BodySolution[];

    global: GlobalResults;

    diagnostics: SolverDiagnostics;
}
```

---

# 44. Numerical Diagnostics

Every solve should produce diagnostics such as:

```text
converged
condition number [if available]
residual
panel count
solver time
geometry warnings
minimum panel length
maximum panel length
```

This is important for engineering credibility.

---

# 45. Solver Errors

Examples:

```text
Geometry is not closed.

Panel 37 has zero length.

Panel distribution is highly non-uniform.

Linear system is singular.

Kutta condition could not be satisfied.

Solver failed to converge.

Body geometry contains self-intersections.
```

Errors should identify the relevant object where possible.

---

# 46. Performance Requirements

## 46.1 Interaction

Target:

```text
UI interaction: 60 FPS
```

when feasible.

---

## 46.2 Typical solve

For a normal interactive scene:

```text
target < 100 ms
```

for incremental/simple solves.

Larger computations may exceed this.

---

## 46.3 Large solves

For expensive operations:

```text
UI remains responsive
```

through:

```text
Web Worker
+
WASM
```

---

# 47. Rendering Strategy

Separate:

```text
simulation computation
```

from:

```text
visual rendering
```

The solver should not render anything.

The renderer should consume solution data.

This enables:

* alternative renderers
* headless tests
* exported data
* numerical testing
* future 3D visualisation

---

# 48. GPU Usage

GPU acceleration should primarily be considered for:

* field visualization
* contour rendering
* vectors
* particles
* streamlines
* large numbers of visual primitives

It should not be assumed that the GPU is required for the numerical panel solve.

GPU/WebGL particle techniques can support large numbers of streamline particles at interactive frame rates.

---

# 49. State Architecture

Use explicit state domains:

```text
SimulationState
UIState
ViewportState
VisualizationState
SolverState
HistoryState
```

Avoid putting everything into one global object.

---

# 50. Undo/Redo

The following operations should be undoable:

* add object
* delete object
* move object
* rotate object
* change strength
* change freestream
* import geometry
* change solver configuration

Undo should operate on **scene state**, not numerical solver internals.

---

# 51. Persistence

MVP should support saving a simulation.

Recommended format:

```text
.aeroflow.json
```

Example:

```json
{
  "version": 1,
  "flow": {
    "velocity": 10,
    "angle": 5,
    "density": 1.225
  },
  "objects": []
}
```

The format must be versioned.

---

# 52. Export

MVP:

* simulation JSON
* geometry CSV
* surface Cp CSV
* force results CSV
* field data CSV/JSON
* plots as PNG/SVG

Future:

* PDF report
* complete simulation bundle

---

# 53. Import Safety

Uploaded files are processed locally.

The application should:

* impose reasonable file-size limits
* reject malformed data
* avoid executing uploaded content
* never interpret uploaded files as code

---

# 54. Accessibility

Target WCAG 2.2 AA where practical.

Requirements:

* keyboard navigable controls
* visible focus states
* semantic labels
* accessible form controls
* sufficient contrast
* no information conveyed solely by colour
* screen-reader labels
* reduced-motion preference

Canvas information should have an accessible numerical alternative where feasible.

---

# 55. Educational UX

The product should not force educational content into the interface.

Instead provide contextual explanations.

Example:

```text
Why is drag approximately zero?

This simulation assumes inviscid potential flow.
Viscous boundary-layer effects are not included.
Therefore the classical solution may predict negligible
drag even though a real airfoil experiences drag.
```

---

# 56. Help System

Provide contextual help for:

* source
* sink
* vortex
* doublet
* circulation
* panel method
* Kutta condition
* Cp
* lift
* drag
* streamlines
* vorticity

Each explanation should contain:

```text
What it means
Why it matters
Mathematical definition
```

where appropriate.

---

# 57. First-Run Experience

On first launch:

```text
Welcome to AeroFlow

Build a flow in seconds.

[Try an example]
[Start empty]
```

Example simulations:

### Example 1

Uniform flow

### Example 2

Source + sink

### Example 3

Flow around cylinder

### Example 4

Vortex + freestream

### Example 5

NACA airfoil

The user should be able to open examples without uploading anything.

---

# 58. Empty State

The empty canvas should contain a subtle instruction:

> Add a flow element or import a geometry to begin.

Avoid presenting a completely blank interface.

---

# 59. Loading States

During solving:

```text
Solving...
```

For longer operations:

```text
Solving panel system
██████████░░░░░
```

The UI must remain interactive.

---

# 60. Solver Cancellation

Long-running operations must be cancellable.

Example:

```text
Running AoA sweep...

[Cancel]
```

The worker should support cancellation rather than simply ignoring the request.

---

# 61. Numerical Validation

This is a **mandatory** engineering requirement.

The numerical team must construct regression tests against analytical solutions.

---

## 61.1 Uniform flow

Verify:

```text
u = U∞
v = 0
```

for zero angle of attack.

---

## 61.2 Source

Compare computed velocity against the analytical radial solution.

---

## 61.3 Sink

Same as source with opposite sign.

---

## 61.4 Vortex

Verify:

```text
Vθ = Γ / (2πr)
```

subject to the chosen circulation convention.

---

## 61.5 Source + sink

Verify superposition.

---

## 61.6 Cylinder

Compare panel-method results against the analytical potential-flow solution around a cylinder.

This should be a core acceptance test.

---

## 61.7 Cylinder with circulation

Verify:

* pressure distribution
* lift
* circulation relationship

---

## 61.8 Airfoil

Compare selected results against a trusted reference implementation such as XFoil where appropriate.

The test must document differences caused by:

* panel formulation
* panel count
* geometry preprocessing
* wake treatment
* numerical precision

---

# 62. Numerical Acceptance Criteria

A numerical acceptance test must specify tolerance.

Example:

```text
Given:
unit cylinder
U∞ = 1
ρ = 1

Expected:
Cp distribution agrees with analytical solution
within defined RMS tolerance.
```

The tolerance must be established empirically and documented by the numerical team.

Do **not** hard-code arbitrary accuracy claims into the product without validation.

---

# 63. Testing Strategy

## Unit tests

Frontend:

* geometry parsing
* coordinate conversion
* state transitions
* reducers/actions
* formatting

WASM:

* elementary solutions
* geometry operations
* panel construction
* matrix construction
* linear solve
* force integration
* Cp
* field evaluation

---

## Integration tests

Verify:

```text
UI → Worker → WASM → Solution → Renderer
```

---

## End-to-end tests

Use Playwright or equivalent.

Required flows:

```text
create source
move source
change strength
add vortex
import geometry
solve airfoil
switch field
inspect forces
save simulation
load simulation
export data
```

---

# 64. Visual Regression

Capture baseline screenshots for:

* empty state
* elementary flow
* airfoil
* velocity field
* pressure field
* Cp
* dark theme
* light theme
* selected object
* error state

---

# 65. Performance Tests

Benchmark:

```text
10 panels
50 panels
100 panels
250 panels
500 panels
1000 panels
```

and document:

```text
matrix construction time
solve time
field evaluation time
serialization time
rendering time
```

---

# 66. Agent-Friendly Repository Architecture

Recommended:

```text
/
├── apps/
│   └── web/
│
├── crates/
│   ├── flow-core/
│   ├── panel-method/
│   ├── geometry/
│   ├── linear-algebra/
│   └── wasm-api/
│
├── packages/
│   ├── domain-model/
│   ├── visualization/
│   ├── ui/
│   └── file-format/
│
├── tests/
│   ├── numerical/
│   ├── integration/
│   └── fixtures/
│
├── examples/
│
├── docs/
│
└── README.md
```

The exact monorepo tooling can be selected by the implementation swarm.

---

# 67. Agent Ownership Boundaries

To allow parallel agent development:

### Agent: Domain Model

Owns:

```text
simulation schema
objects
units
serialization
```

### Agent: Geometry

Owns:

```text
coordinate parsing
validation
panel generation
geometry transformations
```

### Agent: Numerical Solver

Owns:

```text
elementary solutions
panel method
linear systems
field evaluation
forces
```

### Agent: WASM

Owns:

```text
bindings
memory management
worker interface
serialization boundary
```

### Agent: Visualization

Owns:

```text
field rendering
streamlines
vectors
contours
particles
```

### Agent: UI

Owns:

```text
layout
controls
properties
object tree
dialogs
design system
```

### Agent: Charts

Owns:

```text
Cp plots
polar plots
force plots
```

### Agent: QA

Owns:

```text
numerical regression
E2E
visual regression
performance
```

---

# 68. Shared Contracts

Agents must communicate through explicit interfaces.

The following must be defined before parallel implementation begins:

```text
SimulationSchema
ObjectSchema
GeometrySchema
SolverRequest
SolverResponse
FieldData
BodyResult
SolverDiagnostic
```

No agent should invent an incompatible version locally.

---

# 69. Units

The internal solver should use SI units.

Recommended:

```text
length       m
velocity     m/s
density      kg/m³
pressure     Pa
force        N/m
moment       N
circulation  m²/s
```

Because this is a 2D solver, dimensional forces are normally **per unit span**.

The UI must explicitly label this.

---

# 70. Numerical Precision

Default solver calculations should use `f64`/double precision.

Visualization buffers may use `f32` where appropriate for memory/performance.

A precedent exists for WASM scientific applications keeping numerical state in higher precision while exposing render-oriented float buffers to JavaScript.

---

# 71. Error Handling Philosophy

Never silently produce an invalid aerodynamic result.

If a calculation is questionable:

```text
WARNING
```

must be returned alongside the solution.

Examples:

```text
Panel aspect ratio is poor.

Geometry contains sharp numerical discontinuities.

Panel system is poorly conditioned.

Freestream velocity is zero.

Reference chord is zero.

Cp cannot be normalized because U∞ = 0.
```

---

# 72. Solver Status Model

```typescript
type SolverStatus =
    | "idle"
    | "queued"
    | "running"
    | "success"
    | "warning"
    | "error"
    | "cancelled";
```

---

# 73. Requirement ID Convention

All implementation requirements should use IDs:

```text
FR-*  Functional requirement
NFR-* Non-functional requirement
UX-*  UX requirement
NUM-* Numerical requirement
WASM-* WASM requirement
VIS-* Visualization requirement
FILE-* File requirement
TEST-* Testing requirement
SEC-* Security requirement
```

---

# 74. Core Functional Requirements

### FR-001

The system shall allow the user to create a simulation.

### FR-002

The system shall allow an arbitrary number of elementary flow objects subject to performance limits.

### FR-003

The system shall allow users to position elementary objects interactively.

### FR-004

The system shall allow users to edit object parameters.

### FR-005

The system shall allow users to delete and duplicate objects.

### FR-006

The system shall allow users to import coordinate-based geometry.

### FR-007

The system shall validate imported geometry.

### FR-008

The system shall panelize valid geometry.

### FR-009

The system shall solve valid custom geometries using a panel method.

### FR-010

The system shall support multiple bodies in a single simulation.

### FR-011

The system shall calculate velocity fields.

### FR-012

The system shall calculate streamlines.

### FR-013

The system shall calculate pressure.

### FR-014

The system shall calculate pressure coefficient.

### FR-015

The system shall calculate vorticity.

### FR-016

The system shall calculate aerodynamic forces.

### FR-017

The system shall calculate aerodynamic coefficients.

### FR-018

The system shall show per-body results.

### FR-019

The system shall show total results.

### FR-020

The system shall allow users to save simulations.

### FR-021

The system shall allow users to export numerical results.

---

# 75. WASM Requirements

### WASM-001

Numerical solver execution shall occur in WebAssembly.

### WASM-002

The UI shall not directly depend on WASM implementation details.

### WASM-003

The WASM module shall expose a stable typed API.

### WASM-004

Long-running solves shall execute outside the main UI thread.

### WASM-005

The WASM layer shall return structured diagnostics.

### WASM-006

The solver shall support cancellation for long-running operations.

### WASM-007

The WASM numerical core shall be independently testable without a browser.

---

# 76. Visualization Requirements

### VIS-001

The application shall render geometry.

### VIS-002

The application shall render velocity magnitude.

### VIS-003

The application shall render velocity vectors.

### VIS-004

The application shall render streamlines.

### VIS-005

The application shall render pressure.

### VIS-006

The application shall render Cp.

### VIS-007

The application shall render vorticity.

### VIS-008

The application shall render surface plots.

### VIS-009

The application shall support dynamic legends.

### VIS-010

The visualization shall update after simulation changes.

---

# 77. UX Requirements

### UX-001

Adding an object shall require no more than a few direct interactions.

### UX-002

Selected objects shall have a visually obvious selected state.

### UX-003

Object properties shall be editable numerically.

### UX-004

Objects shall be draggable.

### UX-005

The viewport shall support zoom and pan.

### UX-006

The application shall provide undo/redo.

### UX-007

Errors shall be actionable.

### UX-008

The interface shall maintain responsiveness during numerical computation.

---

# 78. Performance Requirements

### NFR-001

The UI should maintain approximately 60 FPS during ordinary interaction.

### NFR-002

Typical interactive solves should target sub-100-ms latency where numerical complexity permits.

### NFR-003

Long calculations must not block the browser main thread.

### NFR-004

The application must support at least the agreed baseline panel count established during performance benchmarking.

The exact maximum should be determined empirically rather than artificially specified in the PRD.

---

# 79. Definition of Done — MVP

MVP is complete when a user can:

1. Launch the application.
2. Add a uniform flow.
3. Add a source.
4. Add a sink.
5. Add a vortex.
6. Add a doublet.
7. Move each object.
8. Change each object's parameters.
9. Visualize velocity.
10. Visualize streamlines.
11. Visualize pressure.
12. Visualize Cp.
13. Visualize vorticity.
14. Import a closed `(x,y)` geometry.
15. Generate panels.
16. Solve the geometry.
17. Apply a Kutta condition where applicable.
18. Display surface Cp.
19. Display lift.
20. Display drag.
21. Display force coefficients.
22. Support multiple bodies.
23. Save a simulation.
24. Reload a simulation.
25. Export numerical data.
26. Pass all analytical numerical regression tests.
27. Pass the core E2E test suite.
28. Run numerical computation through WASM.
29. Remain responsive while solving.

---

# 80. Recommended MVP Screens

The application does **not** need multiple traditional pages.

The primary experience should be a single simulation workspace.

Additional screens/routes may include:

```text
/
    Landing / new simulation

/simulate
    Main simulator

/examples
    Example simulations

/help
    Documentation

/about
    Product / solver information
```

---

# 81. Example Simulation: Flow Around Cylinder

This should be one of the first showcase examples.

Initial scene:

```text
Freestream
U∞ = 1 m/s

Cylinder
R = 1 m
```

Visualizations:

```text
Streamlines
Velocity
Cp
Pressure
```

Expected behaviour:

* symmetric pressure distribution at zero AoA
* zero lift
* approximately zero inviscid drag
* velocity acceleration around the cylinder

This example provides both an educational experience and a numerical validation case.

---

# 82. Example Simulation: Airfoil

Initial configuration:

```text
NACA 0012
U∞ = 10 m/s
α = 5°
```

Display:

```text
Geometry
Streamlines
Cp
Lift
Drag
CL
CD
```

This becomes the canonical demonstration of the panel-method workflow.

---

# 83. Example Simulation: Source + Sink

Example:

```text
Source
      ↓
Freestream →   ●       ●
                    Sink
```

Purpose:

* demonstrate superposition
* demonstrate stagnation points
* demonstrate streamlines
* demonstrate elementary solutions

---

# 84. Product Analytics

If analytics are eventually introduced, track only product-level events such as:

```text
simulation_created
object_added
geometry_imported
solver_completed
solver_failed
visualization_changed
simulation_exported
```

Do not collect uploaded geometry contents or numerical data without explicit product/privacy requirements.

---

# 85. Future Roadmap

## Phase 1 — Potential Flow Core

```text
Elementary solutions
Panel method
WASM
Core visualization
Forces
Cp
```

## Phase 2 — Better Aerodynamic Analysis

```text
AoA sweeps
Polars
Geometry tools
Improved panel distributions
Wake modelling
Improved Kutta treatment
```

## Phase 3 — Boundary Layer

Potentially:

```text
boundary-layer solver
transition models
skin friction
separation estimates
```

## Phase 4 — Viscous-Inviscid Interaction

Potentially integrate:

```text
potential flow
+
boundary layer
```

## Phase 5 — Numerical CFD

Potential future direction:

```text
Euler
Navier-Stokes
RANS
```

These phases must remain architecturally separate from the MVP.

---

# 86. Key Engineering Decisions to Preserve

The implementation swarm must not compromise these principles:

### 1. Solver ≠ renderer

Numerical computation and visualization remain separate.

### 2. WASM ≠ UI

The UI communicates through a stable domain API.

### 3. Geometry ≠ panel solver

Geometry preprocessing should be independently testable.

### 4. Scene state ≠ solution state

A simulation configuration and its numerical solution are different entities.

### 5. Numerical correctness precedes visual polish

A beautiful visualization of an incorrect solution is unacceptable.

### 6. Every important numerical result needs provenance

The application should be able to explain:

```text
what was calculated
from what assumptions
using what solver configuration
```

---

# 87. Critical Product Decisions Still Open

These should be resolved by the implementation/product agents before locking the architecture.

## DEC-001 — Panel formulation

Choose between:

```text
source panel
vortex panel
source + vortex
constant-strength
linear-strength
```

Recommended MVP direction:

**constant-strength source/vortex formulation with explicit Kutta/circulation treatment.**

---

## DEC-002 — Streamline implementation

Choose between:

```text
CPU integration
GPU integration
hybrid
```

Recommended:

**CPU/WASM for authoritative numerical field data + GPU rendering/particle tracing for visualization.**

---

## DEC-003 — Rendering API

Evaluate:

```text
Canvas 2D
WebGL
WebGPU
```

Recommended architecture should abstract the renderer so WebGL/WebGPU can evolve independently.

---

## DEC-004 — Geometry editing

MVP should import geometry rather than provide a complete CAD editor.

A lightweight geometry editor can be added later.

---

## DEC-005 — Mobile

Desktop-first is recommended because the application is an engineering analysis tool.

---

# 88. Product Principles

The final implementation should follow these principles:

> **Interactive first.**

Users should manipulate the flow rather than configure it through forms alone.

> **Numerically honest.**

The application must clearly communicate what potential flow can and cannot model.

> **Professional, not intimidating.**

The UI should look like a serious engineering instrument while remaining approachable to students.

> **Fast feedback.**

Moving an object should make the flow respond almost immediately whenever computationally feasible.

> **Visual understanding.**

The flow field should be the primary interface, not a secondary chart.

> **Composable.**

Elementary solutions and bodies should combine naturally.

> **Extensible.**

The architecture should allow future numerical methods without rewriting the application.

---

# 89. Research Basis

The PRD's technical direction is informed by established potential-flow theory, panel-method practice, scientific visualization approaches, and existing browser-based aerodynamic tools.

Potential-flow solutions can be superposed to construct more complex flows, which directly supports the application's elementary-element model.

Panel methods discretize geometry into panels and solve for singularity distributions while enforcing boundary conditions; lifting bodies require appropriate Kutta/circulation treatment.

NASA's FoilSim provides a useful precedent for combining a primary visualization area with controls and aerodynamic output plots such as surface pressure and drag polars.

Existing browser-based aerodynamic projects demonstrate that substantial numerical computation and visualization can be implemented with WebAssembly and a modern frontend architecture.

---

# 90. Final Implementation Instruction to the Agentic Swarm

The implementation swarm should treat this PRD as the **product contract**, not as a suggestion list.

Before implementation:

1. Convert every requirement into an actionable engineering task.
2. Resolve all `DEC-*` decisions.
3. Define shared TypeScript/Rust schemas.
4. Define the WASM API.
5. Define numerical conventions.
6. Establish analytical regression tests.
7. Establish repository ownership boundaries.
8. Implement the numerical core independently.
9. Validate the solver before building advanced visualization.
10. Implement the UI against mocked solver responses.
11. Integrate the WASM implementation.
12. Run numerical, integration, E2E, visual and performance tests.
13. Do not mark a feature complete until its acceptance criteria are demonstrably satisfied.

**Highest-priority constraint: numerical correctness and architectural separation must not be sacrificed for visual polish.**
