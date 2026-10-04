# Product Requirements Document (PRD)

# Interactive Bézier Geometry Editor & Real-Time Potential Flow

**Working name:** AeroFlow Geometry Studio
**Document type:** Product Requirements Document
**Version:** 1.0
**Status:** Implementation-ready
**Primary consumers:** Agentic software-engineering swarm, frontend agents, geometry/numerical-method agents, WASM agents, QA agents

---

# 1. Product Overview

## 1.1 Purpose

This product extends the existing **2D Potential Flow Simulator** with a fully interactive **parametric geometry editor**.

Instead of requiring users to upload an `(x, y)` coordinate file, users can construct aerodynamic bodies directly inside the application using **Bézier curves**.

The user should be able to:

* Create a new custom geometry.
* Add, remove and move Bézier nodes.
* Move Bézier control handles.
* Change node types.
* Create smooth or sharp corners.
* Insert and delete nodes.
* Adjust individual coordinates numerically.
* Move, rotate and scale the complete geometry.
* Mirror and duplicate geometry.
* Close/open a geometry.
* Change panel resolution.
* See the resulting panel discretization.
* Immediately solve the potential flow around the geometry.
* See velocity, streamlines, pressure, `Cp`, forces and aerodynamic coefficients update as the geometry changes.
* Manipulate geometry while the solver continuously updates in near real time.

The central product experience is:

> **Draw → manipulate → solve → visualize → iterate.**

The geometry editor and aerodynamic solver should feel like one integrated instrument rather than two separate applications.

---

# 2. Relationship to PRD 1

This PRD should be implemented as a second capability layer on top of the architecture defined in the first PRD.

The previous PRD provides:

* Potential-flow physics.
* Elementary analytical flow solutions.
* Panel-method solver.
* WASM numerical core.
* Velocity-field evaluation.
* Streamlines.
* Pressure.
* Pressure coefficient.
* Vorticity.
* Lift and drag.
* Aerodynamic coefficients.
* Multiple-body support.
* Visualization infrastructure.
* Simulation state.
* Solver diagnostics.
* Export/import.

This PRD adds:

> **A parametric Bézier-based geometry authoring system.**

The resulting architecture should therefore be:

```text
                 ┌──────────────────────────────┐
                 │       Geometry Editor        │
                 │                              │
                 │ Bézier nodes                │
                 │ Control handles             │
                 │ Transformations             │
                 │ Geometry constraints        │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
                 │      Geometry Pipeline       │
                 │                              │
                 │ Bézier → sampled curve      │
                 │ → panels                    │
                 │ → validation                │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
                 │     WASM Panel Solver        │
                 │                              │
                 │ Potential flow               │
                 │ Boundary conditions          │
                 │ Kutta condition              │
                 │ Pressure / Cp                │
                 │ Forces                       │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
                 │       Visualization          │
                 │                              │
                 │ Velocity                     │
                 │ Streamlines                  │
                 │ Pressure                     │
                 │ Cp                           │
                 │ Forces                       │
                 │ Aerodynamic plots             │
                 └──────────────────────────────┘
```

The **Bézier representation is authoritative**.

The panel representation is derived from it.

The solver must never modify the user's underlying Bézier geometry.

---

# 3. Product Vision

The application should feel like a combination of:

* a professional vector/curve editor,
* a lightweight CAD geometry editor,
* an aerodynamic analysis tool,
* and an interactive educational potential-flow simulator.

The defining interaction should be:

> **Grab a point on the geometry, move it, and immediately see the aerodynamic consequences.**

For example:

1. User creates an airfoil.
2. User drags the upper-surface control handle.
3. The airfoil becomes more cambered.
4. The geometry is automatically resampled.
5. The panel mesh updates.
6. The WASM solver recomputes the flow.
7. Pressure distribution changes.
8. Streamlines update.
9. Lift/Cp/velocity update.

The transition should feel continuous.

---

# 4. Goals

## G-001 — Interactive geometry creation

Allow users to construct closed 2D aerodynamic bodies without importing coordinate files.

## G-002 — Parametric geometry

Store geometry as Bézier curves rather than only as sampled points.

## G-003 — Professional editing

Provide familiar geometry-editing operations:

* select
* move
* insert
* delete
* drag handles
* transform
* scale
* rotate
* mirror
* duplicate
* align
* close/open
* undo/redo

## G-004 — Real-time aerodynamic feedback

Geometry changes should automatically trigger a new panel-method solution.

## G-005 — Numerical robustness

Geometry editing must not easily create invalid panel meshes or solver crashes.

## G-006 — Preserve aerodynamic correctness

The geometry editor must produce deterministic, well-defined panel geometry.

## G-007 — Educational transparency

Users should be able to understand how:

```text
Bézier curve
      ↓
sampled surface
      ↓
panels
      ↓
boundary conditions
      ↓
potential flow
      ↓
pressure
      ↓
forces
```

## G-008 — Agent-friendly implementation

The architecture must have explicit domain models, contracts, ownership boundaries and acceptance criteria.

---

# 5. Non-Goals

The following are explicitly outside the initial scope.

* Full CAD system.
* NURBS.
* 3D geometry.
* Boolean CAD operations.
* Solid modelling.
* Finite-element geometry.
* Mesh generation for viscous CFD.
* Automatic aerodynamic optimization.
* Structural analysis.
* Manufacturing export.
* Full SVG compatibility.
* Arbitrary spline mathematics beyond the required Bézier representation.
* Freeform sculpting using image/AI generation.

These can be considered future extensions.

---

# 6. Target Users

## 6.1 Aerospace / engineering student

Wants to draw an airfoil and understand how changing its shape affects:

* pressure
* velocity
* lift
* drag
* circulation
* flow topology.

## 6.2 Aerodynamics learner

Wants an intuitive visual relationship between geometry and potential flow.

## 6.3 Engineering user

Wants to quickly prototype shapes and inspect aerodynamic characteristics.

## 6.4 Showcase / advanced user

Wants a visually impressive browser-based demonstration of interactive aerodynamic design.

---

# 7. Core User Journey

## Journey A — Create an airfoil from scratch

```text
New Geometry
      ↓
Choose "Closed Bézier Shape"
      ↓
Create initial nodes
      ↓
Drag nodes
      ↓
Adjust handles
      ↓
Close geometry
      ↓
Set panel resolution
      ↓
Solve
      ↓
Inspect flow
```

## Journey B — Modify an existing geometry

```text
Select geometry
      ↓
Enter Edit Geometry mode
      ↓
Select node
      ↓
Move node
      ↓
Adjust tangent
      ↓
Solver automatically updates
      ↓
Observe Cp / lift / streamlines
```

## Journey C — Investigate geometry sensitivity

```text
Select upper-surface node
      ↓
Drag vertically
      ↓
Observe camber change
      ↓
Observe Cp change
      ↓
Observe CL change
```

This interaction is a primary product feature, not merely a convenience.

---

# 8. Geometry Representation

## 8.1 Authoritative representation

A geometry consists of one or more Bézier segments.

The initial implementation should support:

* cubic Bézier segments.

A cubic Bézier segment is defined by:

```text
P0
P1
P2
P3
```

where:

* `P0` = segment start
* `P1` = first control point
* `P2` = second control point
* `P3` = segment end

The curve is:

```text
B(t) =
(1-t)^3 P0
+ 3(1-t)^2 t P1
+ 3(1-t)t^2 P2
+ t^3 P3

0 <= t <= 1
```

The geometry editor must preserve these parameters.

---

# 9. Geometry Domain Model

A geometry should conceptually be represented as:

```typescript
type Geometry = {
    id: string
    name: string

    closed: boolean

    nodes: GeometryNode[]
    segments: BezierSegment[]

    transform: Transform

    panelization: PanelizationSettings

    metadata: GeometryMetadata
}
```

---

# 10. Geometry Node

Each node represents a point through which the curve passes.

```typescript
type GeometryNode = {
    id: string

    position: Vec2

    inHandle: Handle
    outHandle: Handle

    nodeType: NodeType
}
```

Where:

```typescript
type NodeType =
    | "smooth"
    | "corner"
    | "symmetric"
```

---

# 11. Bézier Handles

Each node has:

```text
             incoming handle
                  ●
                  |
                  |
                  N
                  |
                  |
                  ●
             outgoing handle
```

The UI should visually expose handles when the node is selected.

A handle contains:

```typescript
type Handle = {
    enabled: boolean
    position: Vec2
}
```

The implementation may internally use relative handle coordinates instead of absolute coordinates.

---

# 12. Node Types

## 12.1 Smooth

The incoming and outgoing tangents remain collinear.

Moving one handle may automatically preserve tangent continuity.

## 12.2 Corner

Incoming and outgoing handles are independent.

This allows:

* sharp trailing edges
* sharp leading edges
* corners
* polygon-like features.

## 12.3 Symmetric

Handles remain opposite and equal relative to the node.

Useful for maintaining smooth geometry.

---

# 13. Node Editing

Users must be able to:

* select node
* move node
* move handle
* move both handles
* independently edit handles
* convert smooth ↔ corner
* convert corner ↔ smooth
* delete node
* insert node
* duplicate geometry
* select multiple nodes
* move multiple nodes.

---

# 14. Node Insertion

Users must be able to insert a new node into an existing Bézier segment.

Recommended workflow:

```text
Hover curve
      ↓
Highlight insertion location
      ↓
Click
      ↓
Split Bézier segment
      ↓
Create two valid Bézier segments
      ↓
Insert node
```

The operation must preserve the existing curve as closely as mathematically possible.

The preferred implementation is **exact cubic Bézier subdivision**, rather than simply sampling the curve and reconstructing it.

---

# 15. Node Deletion

Deleting a node must produce a valid neighbouring connection.

The implementation must define deterministic behavior for:

* deleting an interior node
* deleting the first node
* deleting the last node
* deleting nodes from closed loops
* deleting until the geometry becomes invalid.

The application should prevent deletion if doing so would create fewer than the minimum required nodes.

---

# 16. Geometry Creation Modes

The user should have several ways to create geometry.

## 16.1 Blank geometry

Creates an empty editor.

## 16.2 Basic primitives

Initial templates:

* circle
* ellipse
* symmetric airfoil
* NACA-like profile
* rounded body
* rectangle with rounded corners.

These should be implemented as Bézier geometry.

## 16.3 Draw mode

User clicks points to create nodes.

The application automatically creates Bézier segments between them.

## 16.4 Pen tool

A familiar vector-editor interaction:

```text
Click → node
Click-drag → node + tangent
Click → next node
```

## 16.5 Import existing coordinates

The previous PRD's coordinate-file import remains available.

Imported coordinates should be converted into an editable Bézier representation.

This conversion must be explicitly presented as an approximation unless the input itself represents an exact Bézier geometry.

---

# 17. Curve Editing Tools

The editor should support a standard toolbar.

```text
Select
Node
Pen
Add Node
Delete Node
Transform
Rotate
Scale
Mirror
Measure
```

Future tools:

```text
Fillet
Chamfer
Offset
Smooth
Fair
Optimize
```

---

# 18. Transform Operations

For an entire geometry:

## Move

Translate by:

```text
Δx
Δy
```

## Rotate

Rotate around:

* geometry center
* selected point
* user-defined origin.

## Scale

Support:

* uniform scaling
* independent X/Y scaling.

## Mirror

Support:

* horizontal mirror
* vertical mirror
* arbitrary axis in future.

## Numerical transformation

Properties panel should allow:

```text
X
Y
Rotation
Scale X
Scale Y
```

---

# 19. Snapping

Optional but recommended.

The editor should support:

* grid snapping
* point snapping
* horizontal snapping
* vertical snapping
* tangent alignment
* symmetry snapping.

Snapping should be configurable.

---

# 20. Constraints

The editor should support lightweight geometric constraints.

MVP:

* horizontal
* vertical
* coincident
* tangent
* symmetric.

Constraints should not become a full CAD constraint solver in the MVP.

---

# 21. Closed Geometry

Aerodynamic bodies normally require a closed boundary.

The application must clearly distinguish:

```text
Open curve
```

from:

```text
Closed body
```

The user should be able to toggle:

```text
Close Path
Open Path
```

When closing a curve:

* the final segment must be created explicitly;
* continuity should be preserved where possible;
* the UI should show the closing segment.

---

# 22. Trailing Edge Support

Aerodynamic geometries frequently have a sharp trailing edge.

The geometry editor must therefore support:

```text
smooth node
```

and:

```text
sharp node
```

at the same location.

Example:

```text
Upper surface
       \
        \
         ●  ← sharp trailing edge
        /
       /
Lower surface
```

This is important for the panel solver because the Kutta condition may need to be applied at the trailing edge.

The geometry model should explicitly mark a node as:

```typescript
isTrailingEdge: boolean
```

or derive the trailing edge through geometry analysis.

The UI should provide:

```text
Set as trailing edge
```

and:

```text
Auto-detect trailing edge
```

---

# 23. Panelization

The Bézier curve is not directly passed to the panel solver.

Instead:

```text
Bézier geometry
       ↓
curve sampling
       ↓
panel vertices
       ↓
panels
       ↓
panel solver
```

This separation is mandatory.

---

# 24. Panelization Controls

The user should be able to configure:

### Panel count

Example:

```text
32
64
128
256
512
1024
```

### Distribution

Options:

```text
Uniform
Cosine
Cosine clustered
Curvature adaptive
```

Recommended default:

> Cosine/curvature-aware distribution.

This should provide greater resolution near high-curvature regions and important aerodynamic features.

---

# 25. Visual Panel Overlay

The user should be able to toggle:

```text
Show Panels
```

When enabled, the geometry should display:

* panel lines
* panel indices
* panel normals
* panel centers
* optionally panel strength.

Example:

```text
     ┌───┐
   /       \
  / | | | | \
 <  | | | |  >
  \ | | | | /
   \_______/
```

This is particularly important for debugging and education.

---

# 26. Panel Quality Diagnostics

The geometry system must calculate:

* minimum panel length
* maximum panel length
* mean panel length
* panel aspect/length distribution
* curvature distribution
* minimum vertex separation
* self-intersections
* duplicate points
* zero-length panels.

Warnings should be displayed before or during solving.

Example:

> ⚠ Very short panel detected near trailing edge.

or:

> ⚠ Geometry contains a self-intersection and cannot be solved reliably.

---

# 27. Adaptive Resampling

The system should support adaptive sampling.

A possible algorithm:

```text
Evaluate curvature
       ↓
Identify high-curvature regions
       ↓
Increase sampling density
       ↓
Generate panel vertices
       ↓
Check panel quality
       ↓
Solve
```

The user should be able to choose:

```text
Panelization:
    Manual
    Adaptive
```

---

# 28. Geometry → Panel Pipeline

The complete pipeline is:

```text
Geometry state
      ↓
Validate Bézier topology
      ↓
Apply transform
      ↓
Evaluate Bézier curves
      ↓
Generate sample points
      ↓
Apply panel distribution
      ↓
Remove invalid points
      ↓
Build panel objects
      ↓
Orient normals
      ↓
Detect trailing edge
      ↓
Validate panel mesh
      ↓
Send to WASM
```

This pipeline must be deterministic.

---

# 29. Real-Time Solver Integration

Every meaningful geometry change should trigger a new solution.

Example:

```text
Mouse move
   ↓
Geometry changes
   ↓
Debounce / frame scheduling
   ↓
Geometry sampling
   ↓
Panel generation
   ↓
WASM solver
   ↓
Solution
   ↓
Visualization update
```

The solver should not run on every raw mouse event if this would exceed the available computation budget.

Instead, changes should be coalesced.

---

# 30. Interactive Editing Strategy

During continuous dragging:

```text
pointer move
    ↓
update geometry immediately
    ↓
schedule solve
    ↓
cancel obsolete solve
    ↓
run newest geometry
```

If a solver invocation is already running:

```text
Old solve
    ↓
marked obsolete
    ↓
new geometry queued
    ↓
new solve
```

The UI must never display an old solution as if it represented the current geometry.

Each geometry state should have a monotonically increasing:

```typescript
revision: number
```

The solver result must contain the revision it corresponds to.

Results from older revisions must be discarded.

---

# 31. Solver Modes

The application should support two editing modes.

## 31.1 Preview mode

During rapid dragging:

* reduced panel count
* reduced field resolution
* fast solve
* simplified streamlines.

## 31.2 Accurate mode

After the pointer is released:

* full panel resolution
* full field resolution
* full aerodynamic calculations.

Example:

```text
Dragging:
    64 panels

Release:
    256 panels
```

This should make the editor feel responsive without sacrificing final numerical accuracy.

---

# 32. Solver Architecture

The existing panel solver from PRD 1 remains authoritative.

Recommended architecture:

```text
Frontend
   │
   │ GeometryModel
   ▼
Geometry Pipeline
   │
   │ PanelMesh
   ▼
Web Worker
   │
   ▼
WASM
   │
   ▼
Panel Solver
   │
   ├── velocity
   ├── pressure
   ├── Cp
   ├── forces
   └── diagnostics
   │
   ▼
Solution
   │
   ▼
Renderer
```

---

# 33. WASM Interface

The WASM API should receive the derived panel geometry rather than UI-specific Bézier structures.

Conceptual request:

```typescript
type PanelSolveRequest = {
    revision: number

    panels: Panel[]

    freestream: Freestream

    solver: SolverSettings
}
```

Where:

```typescript
type Panel = {
    start: Vec2
    end: Vec2
}
```

Optional metadata:

```typescript
type PanelMetadata = {
    bodyId: string
    panelIndex: number
    surfaceParameterStart: number
    surfaceParameterEnd: number
}
```

---

# 34. Solution Contract

The WASM solver returns:

```typescript
type PanelSolveResult = {
    revision: number

    status:
        | "success"
        | "warning"
        | "failed"
        | "cancelled"

    panels: PanelSolution[]

    bodies: BodyAerodynamicResult[]

    field?: FieldSolution

    diagnostics: SolverDiagnostics
}
```

---

# 35. Panel Solution

Each panel may contain:

```typescript
type PanelSolution = {
    velocityTangential: number
    velocityNormal: number

    pressure: number
    cp: number

    singularityStrength: number

    force: Vec2
}
```

---

# 36. Aerodynamic Results

For each geometry:

```typescript
type BodyAerodynamicResult = {
    bodyId: string

    lift: number
    drag: number

    liftCoefficient: number
    dragCoefficient: number

    moment: number
    momentCoefficient: number

    circulation: number

    referenceChord: number
}
```

All forces must be clearly labelled as:

> force per unit span

unless a different physical convention is explicitly configured.

---

# 37. Real-Time Visualization

The following visualizations from PRD 1 remain available.

## Geometry

* Bézier curve
* control points
* handles
* panel mesh
* normals
* trailing edge.

## Flow

* velocity magnitude
* velocity vectors
* streamlines
* particles
* pressure
* `Cp`
* vorticity
* potential
* stream function.

## Aerodynamics

* force vectors
* lift
* drag
* moment
* circulation
* coefficients.

---

# 38. Geometry Editing Overlay

When editing geometry, the flow visualization should remain visible.

Example:

```text
      → → → → → → → → → →

             ╭────────╮
          ╭──╯        ╰──╮
         ●                ●
          ╲              ╱
           ╰────────────╯

      → → → → → → → → → →
```

Nodes and handles should appear above the flow visualization.

The user must be able to switch between:

```text
Geometry Edit
```

and:

```text
Flow Inspect
```

without leaving the simulation.

---

# 39. Selection Model

Selection hierarchy:

```text
Scene
 └── Geometry
      ├── Node
      ├── Segment
      └── Handle
```

The user can select:

* geometry
* node
* segment
* multiple nodes.

Selected objects should have clear visual feedback.

---

# 40. Properties Panel

When geometry is selected:

```text
GEOMETRY

Name
Airfoil 1

Position
X: 0.000
Y: 0.000

Rotation
0.0°

Scale
X: 1.00
Y: 1.00

Closed
✓

Trailing Edge
Auto

Panels
256

Distribution
Cosine
```

When a node is selected:

```text
NODE

Position
X: 0.532
Y: 0.041

Type
Smooth

Incoming Handle
X
Y

Outgoing Handle
X
Y

Trailing Edge
☐
```

---

# 41. Canvas Interaction

The main canvas must support:

* click selection
* box selection
* drag
* zoom
* pan
* node dragging
* handle dragging
* double-click node insertion
* right-click contextual menu
* keyboard shortcuts.

Recommended shortcuts:

| Action                 | Shortcut               |
| ---------------------- | ---------------------- |
| Select                 | `V`                    |
| Pen                    | `P`                    |
| Add node               | `A`                    |
| Delete                 | `Delete`               |
| Undo                   | `Ctrl/Cmd + Z`         |
| Redo                   | `Ctrl/Cmd + Shift + Z` |
| Duplicate              | `Ctrl/Cmd + D`         |
| Fit view               | `F`                    |
| Toggle panels          | `P`                    |
| Toggle geometry editor | `G`                    |
| Escape tool            | `Esc`                  |

---

# 42. Context Menu

Right-clicking a node should provide:

```text
Move
Convert to Smooth
Convert to Corner
Convert to Symmetric
Insert Node
Delete Node
Set as Trailing Edge
Clear Trailing Edge
```

Right-clicking a geometry:

```text
Duplicate
Mirror
Rotate
Scale
Close Path
Open Path
Reverse Orientation
Show Panels
Hide Panels
Delete
```

---

# 43. Undo / Redo

Every geometry operation must be undoable.

Operations include:

* node movement
* handle movement
* node insertion
* node deletion
* node type changes
* transformations
* geometry creation
* geometry deletion
* panel settings.

Undo must operate on the geometry model, not on rendered output.

Recommended model:

```text
Command
   ↓
apply()
   ↓
new immutable geometry state
```

---

# 44. Autosave

The application should maintain a local working state.

Recommended:

```text
autosave every N seconds
```

and:

```text
autosave after meaningful edit
```

The autosaved state should include:

* Bézier geometry
* transforms
* solver settings
* visualization state
* camera position
* selected object.

---

# 45. File Format

The previous `.aeroflow.json` format should be extended.

Example:

```json
{
  "version": 2,
  "scene": {
    "objects": [
      {
        "type": "bezier-body",
        "id": "body-001",
        "name": "Custom Airfoil",
        "closed": true,
        "nodes": [],
        "segments": [],
        "transform": {
          "x": 0,
          "y": 0,
          "rotation": 0,
          "scaleX": 1,
          "scaleY": 1
        },
        "panelization": {
          "count": 256,
          "distribution": "cosine"
        }
      }
    ]
  }
}
```

The file format must be versioned.

---

# 46. Export

Users should be able to export the geometry as:

### Bézier project

```text
.aeroflow.json
```

### Sampled coordinates

```text
.csv
```

### Panel coordinates

```text
.csv
```

### SVG

Optional future feature.

### Analysis

```text
Cp.csv
forces.csv
surface.csv
```

---

# 47. Import

The editor should accept:

```text
CSV
TXT
DAT
```

with:

```text
x y
```

coordinates.

The imported shape can be converted into a Bézier approximation.

The UI must explicitly tell the user:

> Imported coordinate geometry has been approximated using Bézier curves.

The user should be able to control approximation tolerance.

---

# 48. Geometry Approximation

When converting coordinate points to Bézier curves:

```text
Input points
      ↓
Remove invalid points
      ↓
Detect closure
      ↓
Estimate tangents
      ↓
Fit Bézier segments
      ↓
Calculate fitting error
      ↓
Display result
```

The UI should expose:

```text
Approximation tolerance
```

and optionally:

```text
Number of Bézier segments
```

---

# 49. Geometry Validation

Before solving, validate:

### Topology

* sufficient nodes
* valid segment connectivity
* valid closed/open state.

### Numerical validity

* finite coordinates
* no NaN
* no infinite values.

### Curve validity

* no degenerate segments
* no zero-length segments
* no pathological handles.

### Aerodynamic validity

* closed geometry when panel solver requires a body
* sensible orientation
* no severe self-intersections
* no duplicated points after discretization.

---

# 50. Self-Intersection Detection

Self-intersections must be detected at the sampled/panelized representation.

If detected:

```text
⚠ Invalid geometry

The current shape intersects itself.
The panel solver may produce invalid results.

[Show intersections]
[Edit geometry]
```

The solver should not silently return potentially invalid aerodynamic results.

---

# 51. Geometry Orientation

The system should determine whether the geometry is:

```text
clockwise
```

or:

```text
counter-clockwise
```

The orientation must be normalized for the panel solver.

The user should optionally be able to:

```text
Reverse Geometry
```

---

# 52. Trailing Edge Detection

Automatic trailing-edge detection should use geometric criteria.

Potential criteria:

* minimum local thickness
* sharpness
* maximum chordwise position
* tangent discontinuity.

The user must be able to override automatic detection.

---

# 53. Reference Chord

For aerodynamic coefficients, the geometry should expose:

```text
reference chord
```

The system should calculate an initial estimate from the geometry's bounding box or user-defined leading/trailing edges.

Users can override it.

---

# 54. Coordinate System

Use:

```text
+X = right
+Y = up
```

Angles:

```text
positive AoA = counter-clockwise
```

All geometry coordinates use a consistent Cartesian system.

Screen coordinates must be transformed separately.

---

# 55. Geometry Transform vs Geometry Editing

Important architectural distinction:

### Geometry editing

Changes the actual Bézier control points.

### Transform

Changes the placement of the geometry.

For example:

```text
Bézier definition
       +
transform matrix
       =
world geometry
```

This allows moving/rotating an airfoil without destroying its underlying shape.

---

# 56. Rendering Architecture

Use separate rendering layers.

```text
Layer 1 — Background
Layer 2 — Grid
Layer 3 — Flow field
Layer 4 — Streamlines
Layer 5 — Geometry
Layer 6 — Panels
Layer 7 — Bézier handles
Layer 8 — Selection
Layer 9 — Interaction overlays
```

The geometry editing layer must not require rerendering the entire flow visualization for every pointer event.

---

# 57. Rendering Performance

Target:

> 60 FPS during geometry manipulation.

The UI should remain responsive even if the solver cannot complete 60 solutions per second.

The architecture should therefore distinguish:

```text
interaction frame rate
```

from:

```text
solver update rate
```

Example:

```text
60 FPS interaction
       +
10–30 solver updates/sec
```

can still feel real time.

---

# 58. Solver Scheduling

Use a scheduler:

```typescript
type SolveScheduler = {
    requestSolve(reason: SolveReason): void
    cancelPending(): void
}
```

Possible reasons:

```text
geometry-change
flow-condition-change
panel-settings-change
visualization-change
initialization
```

Visualization-only changes should **not** trigger a new panel solve.

---

# 59. Solve Caching

The application should cache expensive intermediate data where practical.

Potential cache keys:

```text
geometryRevision
panelizationSettings
solverSettings
freestream
```

For example, changing:

```text
Cp view → velocity view
```

must not trigger another panel solve.

Changing:

```text
geometry
```

must invalidate the geometry-dependent solution.

---

# 60. Partial Recalculation

The implementation may eventually support partial updates.

However, MVP does **not** require incremental matrix updates.

Correctness takes priority.

MVP behavior:

```text
geometry changes
      ↓
re-panelize
      ↓
rebuild influence matrix
      ↓
solve
```

Future optimization can introduce matrix reuse.

---

# 61. Preview Solver

The preview solver should have configurable settings:

```text
previewPanelCount = 64
previewFieldResolution = low
previewStreamlines = reduced
```

The final solver uses:

```text
panelCount = user setting
fieldResolution = user setting
```

---

# 62. Flow Visualization During Editing

The flow should update progressively.

Possible states:

```text
Current valid solution
       ↓
geometry changed
       ↓
"Updating…"
       ↓
new solution
       ↓
new visualization
```

The previous valid solution may remain visible during computation, but it must be visibly marked as stale if retained.

Example:

> Updating flow…

---

# 63. Numerical Stability During Editing

Geometry editing can create temporarily invalid shapes.

The application must not crash.

For example:

```text
node dragged through another node
```

may produce an invalid body.

The UI should show:

```text
Invalid geometry
```

rather than crashing the WASM solver.

The solver should receive only validated geometry.

---

# 64. Error Handling

Errors should be categorized.

## Geometry error

```text
Self-intersection detected.
```

## Panelization error

```text
Unable to generate valid panels.
```

## Solver error

```text
Linear system could not be solved reliably.
```

## Performance warning

```text
High panel count may reduce interactive performance.
```

---

# 65. Educational Feedback

The application should explain relevant aerodynamic behavior without overwhelming the user.

For example:

> Increasing camber changes the surface velocity distribution and therefore changes pressure and lift.

For potential-flow limitations:

> This solver models inviscid potential flow. Viscous effects, boundary-layer separation and realistic profile drag are not represented.

---

# 66. Force Visualization

For each geometry:

```text
       ↑ Lift
       │
       ●────→ Drag
      body
```

The force vector should be displayed relative to the selected coordinate system.

The UI should distinguish:

```text
Body-axis forces
```

from:

```text
Wind-axis Lift / Drag
```

---

# 67. Live Metrics

A compact live-results panel should show:

```text
CL        0.842
CD        0.012
Cm       -0.041
Γ         0.527

Lift      18.42 N/m
Drag       0.26 N/m
```

Values should update automatically after each valid solve.

---

# 68. Surface Analysis

For the selected geometry:

```text
Cp vs x/c
Velocity vs x/c
Pressure vs x/c
```

should update as the user changes the geometry.

Upper and lower surfaces should be visually distinguishable.

The graph should identify the current node/position where possible.

---

# 69. Geometry-to-Plot Interaction

Selecting a point on the `Cp` graph should optionally highlight the corresponding location on the geometry.

Likewise:

```text
click geometry
      ↓
corresponding x/c highlighted on graph
```

This creates a bidirectional analysis workflow.

---

# 70. Comparison Mode

Future feature:

```text
Geometry A
Geometry B
```

displayed simultaneously.

Compare:

* geometry
* Cp
* CL
* CD
* velocity
* pressure.

Not required for MVP, but architecture should permit it.

---

# 71. Geometry History

The application should maintain geometry history.

Example:

```text
Version 1
Version 2
Version 3
Version 4
```

Potential future functionality:

```text
Compare current geometry with previous geometry.
```

---

# 72. Responsive Layout

Desktop-first.

Recommended layout:

```text
┌─────────────────────────────────────────────────────┐
│ Toolbar / Simulation Controls                       │
├───────────────┬──────────────────────┬──────────────┤
│               │                      │              │
│ Geometry      │                      │ Properties   │
│ Tools         │     Simulation       │              │
│               │       Canvas         │ Geometry     │
│ Object Tree   │                      │ Node         │
│               │                      │ Solver       │
│               │                      │ Results      │
├───────────────┴──────────────────────┴──────────────┤
│ Cp / Pressure / Aerodynamic Charts                  │
└─────────────────────────────────────────────────────┘
```

---

# 73. Geometry Toolbar

Suggested toolbar:

```text
[Select]
[Node]
[Pen]
[Add]
[Delete]
[Transform]
[Rotate]
[Scale]
[Mirror]
[Measure]
```

Separate aerodynamic toolbar:

```text
[Velocity]
[Streamlines]
[Pressure]
[Cp]
[Vorticity]
[Panels]
[Forces]
```

---

# 74. Interaction Modes

The application should explicitly separate:

### Inspect mode

User explores the flow.

### Geometry mode

User edits Bézier geometry.

### Transform mode

User moves/scales/rotates the body.

### Panel mode

User inspects panelization.

This reduces accidental geometry modification.

---

# 75. Keyboard and Mouse UX

Important principles:

* handles should have generous hit areas
* selected nodes should be visually obvious
* dragging should feel smooth
* snapping should be subtle
* precision editing must always be available
* accidental deletion should require confirmation only for destructive bulk operations.

---

# 76. Accessibility

The geometry editor must not depend exclusively on mouse interaction.

Keyboard alternatives should exist for:

* selecting nodes
* moving nodes
* changing coordinates
* changing handle coordinates
* inserting/deleting nodes
* changing node type.

Example:

```text
Arrow keys → move selected node
Shift + Arrow → larger movement
Alt + Arrow → fine movement
```

Numeric coordinate inputs provide a precision alternative.

---

# 77. Units

Geometry uses configurable units, with SI as the internal representation.

Recommended:

```text
meters
```

Internally all geometry should be normalized to a consistent unit system.

---

# 78. Scale and Numerical Conditioning

The application should detect extreme geometry scales.

Example:

```text
Geometry chord = 0.0000001 m
```

or:

```text
Geometry chord = 10,000,000 m
```

should trigger a warning or normalization strategy.

The numerical solver should operate in a numerically stable coordinate system where appropriate.

---

# 79. Multi-Body Support

The editor must support multiple Bézier bodies.

Example:

```text
Main wing
      +
Flap
      +
Slat
```

Each body has independent:

* geometry
* transform
* panelization
* visibility
* selection
* aerodynamic results.

The solver must consider all bodies simultaneously when computing the global flow.

---

# 80. Geometry Object Tree

Example:

```text
SCENE

▼ Geometry
   ├── Main Airfoil
   ├── Flap
   └── Slat

▼ Flow Elements
   ├── Freestream
   └── Vortex
```

Each object supports:

```text
visibility
lock
select
duplicate
delete
```

---

# 81. Locking Geometry

A geometry can be locked.

When locked:

* cannot be moved
* cannot have nodes edited
* remains part of the simulation.

Useful for:

```text
main wing locked
flap editable
```

---

# 82. Duplication

Duplicating a geometry must clone:

* Bézier nodes
* handles
* segments
* panelization settings
* transform.

It must assign a new object ID.

---

# 83. Symmetry

Future geometry tools should support symmetry.

Example:

```text
Create upper surface
      ↓
Mirror
      ↓
Generate lower surface
```

This can simplify creation of symmetric airfoils.

MVP can expose simple mirror operations rather than a complete parametric symmetry system.

---

# 84. Geometry Presets

Initial presets:

* Circle
* Ellipse
* NACA 0012
* NACA 2412
* Flat plate
* Rounded plate
* Generic airfoil.

These should be generated as Bézier geometry or converted into Bézier geometry before entering the editor.

---

# 85. NACA Geometry Workflow

Example:

```text
Add NACA 0012
      ↓
Bézier representation created
      ↓
User selects node
      ↓
Drag upper surface
      ↓
Shape changes
      ↓
Panel mesh changes
      ↓
Flow recalculated
```

This should demonstrate the core product value immediately.

---

# 86. Numerical Validation

The geometry editor requires numerical regression tests in addition to the solver tests from PRD 1.

## Geometry tests

### GEOM-TEST-001

Bézier endpoint evaluation.

### GEOM-TEST-002

Bézier tangent evaluation.

### GEOM-TEST-003

Bézier subdivision.

### GEOM-TEST-004

Node insertion preserves curve.

### GEOM-TEST-005

Node deletion creates valid topology.

### GEOM-TEST-006

Smooth node preserves tangent continuity.

### GEOM-TEST-007

Corner node permits tangent discontinuity.

### GEOM-TEST-008

Closed curve produces identical start/end position.

### GEOM-TEST-009

Transform operations preserve geometry relationships.

### GEOM-TEST-010

Panelization is deterministic.

---

# 87. Aerodynamic Regression Tests

The following must remain valid:

### Circle

Compare against analytical potential flow around a cylinder.

### Symmetric airfoil

At zero angle of attack:

```text
CL ≈ 0
```

within an established numerical tolerance.

### Cambered airfoil

Changing camber must change the computed pressure distribution.

### Geometry perturbation

A controlled geometry change must produce a corresponding solution change.

---

# 88. Interactive Regression Test

A key automated test:

```text
Create airfoil
      ↓
Move node
      ↓
Wait for solve
      ↓
Verify geometry revision increased
      ↓
Verify solver revision matches
      ↓
Verify Cp changed
      ↓
Verify aerodynamic results changed
```

This test validates the entire product chain.

---

# 89. Stale Result Test

Test:

```text
Geometry revision 10
      ↓
start solve

Geometry revision 11
      ↓
start solve

revision 10 finishes last
```

The application must reject revision 10.

Only revision 11 may be displayed as current.

---

# 90. Performance Requirements

## NFR-GEO-001

Geometry manipulation should maintain approximately 60 FPS under normal conditions.

## NFR-GEO-002

Editing a geometry with approximately 20–100 Bézier nodes should remain responsive.

## NFR-GEO-003

Preview solving should prioritize responsiveness over maximum numerical resolution.

## NFR-GEO-004

Final solving should use the configured accuracy.

## NFR-GEO-005

The UI thread must not be blocked by the panel solver.

---

# 91. Suggested Performance Targets

For a typical desktop:

| Operation       |             Target |
| --------------- | -----------------: |
| Node movement   | <16 ms UI response |
| Curve rendering |             <16 ms |
| Panelization    |     <20 ms typical |
| Preview solve   |    <100 ms typical |
| Full solve      |    <500 ms typical |
| UI blocking     |               0 ms |

These are engineering targets rather than hard guarantees across all hardware.

---

# 92. Web Worker Architecture

The WASM solver should execute in a Web Worker.

```text
Main Thread
    │
    ├── Geometry editor
    ├── UI
    └── Rendering
          │
          ▼
      Web Worker
          │
          ▼
         WASM
```

This ensures a heavy solve cannot freeze the editor.

---

# 93. Shared Memory

Where supported, the architecture may use efficient typed-array transfer or shared memory.

Potential buffers:

```text
geometry coordinates
panel coordinates
velocity field
Cp
pressure
streamlines
```

Do not expose internal solver memory directly to the UI.

---

# 94. Rendering Technology

The implementation should abstract rendering from geometry state.

Recommended:

```text
React
+
TypeScript
+
Canvas/WebGL/WebGPU
```

The geometry editor can use a vector-oriented rendering layer while the flow field can use GPU rendering.

Possible implementation:

```text
SVG / Canvas
    ↓
geometry editor

WebGL
    ↓
flow field
```

The exact rendering technology is an implementation decision as long as the architecture remains decoupled.

---

# 95. Recommended Repository Structure

```text
/
├── apps/
│   └── web/
│
├── crates/
│   ├── flow-core/
│   ├── panel-method/
│   ├── geometry-core/
│   ├── bezier/
│   ├── panelization/
│   ├── linear-algebra/
│   └── wasm-api/
│
├── packages/
│   ├── domain-model/
│   ├── geometry-editor/
│   ├── visualization/
│   ├── ui/
│   ├── charts/
│   └── file-format/
│
├── tests/
│   ├── geometry/
│   ├── numerical/
│   ├── integration/
│   ├── e2e/
│   └── fixtures/
│
├── examples/
│
└── docs/
```

---

# 96. Agent Ownership

## Agent A — Geometry Core

Owns:

* geometry model
* nodes
* segments
* transforms
* topology.

## Agent B — Bézier Mathematics

Owns:

* evaluation
* derivatives
* tangents
* subdivision
* fitting.

## Agent C — Panelization

Owns:

* sampling
* panel generation
* adaptive distribution
* panel diagnostics.

## Agent D — Panel Solver

Owns:

* influence coefficients
* linear system
* Kutta condition
* velocity
* Cp
* forces.

## Agent E — WASM

Owns:

* WASM bindings
* worker protocol
* serialization
* cancellation
* typed arrays.

## Agent F — Geometry UI

Owns:

* node editing
* handles
* selection
* pen tool
* transforms.

## Agent G — Visualization

Owns:

* flow rendering
* panels
* streamlines
* pressure
* Cp.

## Agent H — QA

Owns:

* numerical regression
* geometry tests
* E2E
* performance tests
* visual regression.

---

# 97. Shared Contracts

Agents must not invent incompatible domain models.

The following contracts must be established before parallel implementation begins:

```text
GeometrySchema
GeometryNodeSchema
BezierSegmentSchema
TransformSchema
PanelSchema
PanelizationSchema
SolverRequest
SolverResponse
BodyResult
SolverDiagnostics
SimulationState
```

These schemas are shared infrastructure.

---

# 98. Requirement IDs

Requirements must be traceable.

## Geometry

```text
GEO-001
GEO-002
...
```

## Bézier

```text
BEZ-001
BEZ-002
...
```

## Panelization

```text
PAN-001
PAN-002
...
```

## Solver

```text
SOL-001
SOL-002
...
```

## WASM

```text
WASM-001
...
```

## Visualization

```text
VIS-001
...
```

## UX

```text
UX-001
...
```

## Testing

```text
TEST-001
...
```

---

# 99. Core Functional Requirements

## GEO-001

User can create a new Bézier geometry.

## GEO-002

User can add nodes.

## GEO-003

User can delete nodes.

## GEO-004

User can move nodes.

## GEO-005

User can edit Bézier handles.

## GEO-006

User can change node type.

## GEO-007

User can insert nodes into existing curves.

## GEO-008

User can close and open paths.

## GEO-009

User can move a complete geometry.

## GEO-010

User can rotate a geometry.

## GEO-011

User can scale a geometry.

## GEO-012

User can mirror a geometry.

## GEO-013

User can duplicate a geometry.

## GEO-014

User can undo and redo geometry operations.

## GEO-015

User can numerically edit geometry coordinates.

---

# 100. Bézier Requirements

## BEZ-001

The geometry model must preserve cubic Bézier control points.

## BEZ-002

The curve evaluator must return position for arbitrary `t`.

## BEZ-003

The curve evaluator must return first derivative.

## BEZ-004

The implementation must support exact cubic Bézier subdivision.

## BEZ-005

Smooth nodes must support tangent continuity.

## BEZ-006

Corner nodes must support independent tangents.

## BEZ-007

Bézier geometry must remain independent from the panel mesh.

---

# 101. Panel Requirements

## PAN-001

Bézier curves must be converted into panels before solving.

## PAN-002

Panelization must be deterministic.

## PAN-003

Panel count must be configurable.

## PAN-004

Panel distribution must be configurable.

## PAN-005

Panel quality diagnostics must be available.

## PAN-006

Panel normals must be correctly oriented.

## PAN-007

Self-intersections must be detected.

## PAN-008

Degenerate panels must be rejected.

---

# 102. Real-Time Requirements

## RT-001

Geometry edits must trigger solver updates.

## RT-002

Solver updates must execute outside the main UI thread.

## RT-003

Obsolete solver results must be discarded.

## RT-004

Preview and final solver resolutions must be supported.

## RT-005

The user must receive visual feedback while solving.

---

# 103. Visualization Requirements

## VIS-001

Display Bézier curves.

## VIS-002

Display selected nodes.

## VIS-003

Display control handles.

## VIS-004

Display panelization.

## VIS-005

Display velocity field.

## VIS-006

Display streamlines.

## VIS-007

Display pressure.

## VIS-008

Display Cp.

## VIS-009

Display force vectors.

## VIS-010

Display aerodynamic results.

---

# 104. File Requirements

## FILE-001

Save Bézier geometry.

## FILE-002

Load Bézier geometry.

## FILE-003

Export sampled coordinates.

## FILE-004

Export panel coordinates.

## FILE-005

Import coordinate geometry.

## FILE-006

Version the file format.

---

# 105. QA Requirements

## TEST-001

All Bézier mathematical operations must have unit tests.

## TEST-002

Panelization must have deterministic regression tests.

## TEST-003

Known analytical aerodynamic cases must be validated.

## TEST-004

Geometry editing must have E2E tests.

## TEST-005

Stale solver results must be tested.

## TEST-006

Undo/redo must be tested.

## TEST-007

Invalid geometry must never crash the application.

## TEST-008

Performance must be benchmarked.

---

# 106. MVP

The MVP must contain:

### Geometry

* cubic Bézier curves
* nodes
* control handles
* smooth nodes
* corner nodes
* node insertion
* node deletion
* node movement
* handle movement
* closed paths
* transforms.

### Panelization

* Bézier → panels
* configurable panel count
* cosine distribution
* panel visualization
* validation.

### Solver

* WASM
* Web Worker
* panel method
* Kutta condition
* pressure
* Cp
* lift
* drag
* moment.

### Visualization

* geometry
* panels
* velocity
* streamlines
* pressure
* Cp
* force vectors.

### UX

* undo/redo
* properties panel
* object tree
* keyboard shortcuts
* save/load.

---

# 107. Post-MVP

Potential extensions:

* curvature-adaptive panelization
* advanced tangent constraints
* symmetry tools
* geometry comparison
* geometry morphing
* SVG import/export
* aerodynamic optimization
* automated airfoil fitting
* geometry parameter sweeps
* sensitivity analysis
* shape optimization.
* boundary-layer coupling
* viscous corrections.

---

# 108. Example MVP Scenario

A user opens the application.

They select:

```text
New → Airfoil
```

The application creates:

```text
NACA-like Bézier airfoil
```

The user selects a node on the upper surface.

They drag it upward.

The system performs:

```text
Node position changed
        ↓
Bezier curve updated
        ↓
Geometry revision +1
        ↓
Curve resampled
        ↓
Panels regenerated
        ↓
Panel mesh validated
        ↓
WASM solve
        ↓
Cp calculated
        ↓
Pressure calculated
        ↓
Forces calculated
        ↓
Visualization updated
```

The user sees the changed:

* streamlines
* velocity field
* surface pressure
* `Cp`
* lift
* drag
* moment.

No page reload occurs.

No manual "Solve" button is required for normal editing.

---

# 109. Example Advanced Scenario

The user creates:

```text
Main wing
Flap
```

They position the flap below the main wing.

They change:

```text
Flap angle = 20°
```

The global panel system solves both bodies together.

The application displays:

```text
Main Wing
CL = ...
CD = ...

Flap
CL = ...
CD = ...

Total
CL = ...
CD = ...
```

The user then drags the flap.

The aerodynamic results update continuously.

---

# 110. Critical Architectural Principle

The implementation must **not** make the panel mesh the primary geometry model.

Incorrect:

```text
Node movement
   ↓
modify panel vertices
```

Correct:

```text
Node movement
   ↓
modify Bézier geometry
   ↓
resample
   ↓
regenerate panels
```

This is one of the most important architectural constraints in this PRD.

---

# 111. Critical Numerical Principle

The implementation must **not** sacrifice numerical correctness merely to achieve visually continuous updates.

The system should distinguish:

```text
interactive preview solution
```

from:

```text
final accurate solution
```

A lower-resolution preview is acceptable.

An incorrect aerodynamic solution presented as accurate is not.

---

# 112. Critical UX Principle

The user should never have to think about the underlying computational pipeline during ordinary editing.

They should experience:

```text
Move shape
    ↓
Flow changes
```

rather than:

```text
Move shape
    ↓
Click Generate Panels
    ↓
Click Solve
    ↓
Click Calculate Cp
```

The computational pipeline should be automatic.

Advanced users can still expose these stages through diagnostic panels.

---

# 113. Solver Diagnostics

Advanced mode should display:

```text
Geometry
    Nodes: 24
    Bézier segments: 24

Panelization
    Panels: 256
    Min panel: 0.0012
    Max panel: 0.0184

Solver
    Matrix: 256 × 256
    Residual: ...
    Condition estimate: ...
    Solve time: 23 ms

Result
    CL: ...
    CD: ...
```

This is especially useful for engineering users.

---

# 114. Design Principles

The UI should be:

* modern
* clean
* technical
* responsive
* visually calm
* discoverable
* precise.

Avoid:

* excessive gradients
* unnecessary modal dialogs
* cluttered control panels
* tiny interaction handles
* excessive animation
* hiding important numerical information.

---

# 115. Educational Principle

The application should expose the relationship:

```text
Shape
  ↓
Boundary condition
  ↓
Velocity
  ↓
Pressure
  ↓
Force
```

The user should be able to understand not just *what* changes, but *why*.

---

# 116. Acceptance Criteria

The implementation is considered successful when a user can:

1. Open the application.
2. Create a new Bézier body.
3. Add at least four nodes.
4. Move nodes.
5. Move Bézier handles.
6. Create a sharp trailing edge.
7. Create a smooth leading edge.
8. Close the geometry.
9. Move the complete geometry.
10. Rotate it.
11. Scale it.
12. Undo and redo edits.
13. Generate panels automatically.
14. Inspect the panel mesh.
15. Change panel count.
16. Solve potential flow through WASM.
17. See the velocity field.
18. See streamlines.
19. See pressure.
20. See `Cp`.
21. See lift and drag.
22. See aerodynamic coefficients.
23. Modify a node while the solver is active.
24. See the aerodynamic solution update.
25. Receive an error for invalid geometry.
26. Save the Bézier geometry.
27. Reload it without losing control points.
28. Export coordinates.
29. Import a coordinate-defined geometry and convert it to Bézier geometry.
30. Run the application without the UI freezing during solver execution.

---

# 117. Definition of Done

The feature is complete when:

* [ ] Bézier domain model is implemented.
* [ ] Cubic Bézier evaluation is tested.
* [ ] Bézier subdivision is tested.
* [ ] Node editing works.
* [ ] Handle editing works.
* [ ] Smooth/corner nodes work.
* [ ] Node insertion works.
* [ ] Node deletion works.
* [ ] Geometry transformations work.
* [ ] Closed paths work.
* [ ] Geometry validation works.
* [ ] Bézier → panel conversion works.
* [ ] Panel visualization works.
* [ ] Panel solver accepts generated geometry.
* [ ] WASM solver runs in a worker.
* [ ] Solver revisions prevent stale results.
* [ ] Preview solving works.
* [ ] Final solving works.
* [ ] Flow visualization updates.
* [ ] Cp updates.
* [ ] Forces update.
* [ ] Multiple bodies work.
* [ ] Undo/redo works.
* [ ] Save/load works.
* [ ] Import/export works.
* [ ] Numerical regression passes.
* [ ] E2E tests pass.
* [ ] Performance benchmarks pass.
* [ ] Invalid geometry cannot crash the application.

---

# 118. Implementation Order

Agents should implement in this order.

## Phase 1 — Domain model

Implement:

```text
Vec2
Transform
Geometry
GeometryNode
BezierSegment
Handle
```

## Phase 2 — Bézier mathematics

Implement and test:

```text
evaluate(t)
derivative(t)
split(t)
length()
boundingBox()
```

## Phase 3 — Geometry editor

Implement:

```text
selection
node movement
handle movement
insertion
deletion
node types
```

## Phase 4 — Panelization

Implement:

```text
sampling
panel creation
normal generation
validation
diagnostics
```

## Phase 5 — Solver integration

Connect:

```text
Geometry
→ Panelization
→ WASM
→ Solution
```

## Phase 6 — Real-time scheduling

Implement:

```text
revision IDs
worker
cancellation
preview mode
solve scheduler
```

## Phase 7 — Visualization

Connect:

```text
flow
Cp
pressure
streamlines
forces
```

## Phase 8 — Persistence

Implement:

```text
save
load
import
export
```

## Phase 9 — QA

Implement:

```text
unit tests
numerical tests
integration tests
E2E tests
performance tests
visual regression
```

---

# 119. Future Architecture Direction

The architecture should leave room for a future progression:

```text
Bézier Geometry
      ↓
Potential Flow
      ↓
Panel Method
      ↓
Boundary Layer
      ↓
Viscous-Inviscid Interaction
      ↓
Advanced Aerodynamic Analysis
      ↓
Optimization
```

The geometry editor therefore becomes a foundational component of the larger aerodynamic platform.

---

# 120. Final Agent Instructions

Treat this PRD as an implementation contract.

Agents must:

1. Establish shared geometry schemas before parallel implementation.
2. Implement Bézier mathematics independently of the UI.
3. Make Bézier geometry the authoritative representation.
4. Keep panelization as a derived representation.
5. Keep the numerical solver independent from rendering.
6. Keep the UI independent from WASM implementation details.
7. Run the panel solver in a Web Worker.
8. Use revision IDs to prevent stale results.
9. Support low-resolution preview solves during manipulation.
10. Perform accurate solves after interaction settles.
11. Never silently accept invalid geometry.
12. Add numerical regression tests before optimizing performance.
13. Maintain deterministic geometry-to-panel conversion.
14. Keep the file format versioned.
15. Preserve compatibility with the first PRD's simulation architecture.
16. Prioritize numerical correctness over visual effects.
17. Prioritize interaction responsiveness over unnecessary solver resolution during dragging.
18. Keep the geometry editor usable independently from the aerodynamic visualization layer.

The defining product loop is:

```text
              ┌───────────────┐
              │  CREATE SHAPE │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │ EDIT BÉZIER   │
              │ NODES/HANDLES │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │ PANELIZATION  │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │  WASM SOLVER  │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │ FLOW + Cp +   │
              │ FORCES        │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │ USER OBSERVES │
              │ AND EDITS     │
              └───────┬───────┘
                      │
                      └──────────────► repeat
```

The ultimate experience should feel like an **interactive aerodynamic sketchpad**: the user draws and manipulates a shape, and the physics responds immediately.
