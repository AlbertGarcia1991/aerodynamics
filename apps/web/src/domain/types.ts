/**
 * Shared domain model — the TypeScript mirror of the Rust serde schema
 * (PRD §68 "Shared Contracts").
 *
 * Every interface here corresponds to a `#[derive(Serialize, Deserialize)]`
 * type in `crates/solver`, `crates/flow-core` or `crates/geometry`, with
 * `rename_all = "camelCase"`. Angles are **radians** everywhere in this model;
 * the UI converts to degrees at the edges (PRD §69 SI units).
 *
 * Conventions (PRD §14.1): +x right, +y up, positive angles counter-clockwise,
 * circulation Γ > 0 counter-clockwise so that lift L = −ρU∞Γ.
 */

export interface Vec2 {
  x: number;
  y: number;
}

// ───────────────────────────── Scene ─────────────────────────────

export interface FlowConditions {
  /** U∞ [m/s] */
  velocity: number;
  /** α [rad], counter-clockwise positive */
  angle: number;
  /** ρ [kg/m³] */
  density: number;
  /** p∞ [Pa] */
  pressure: number;
}

export type Element =
  | { type: 'uniformFlow'; velocity: number; direction: number }
  | { type: 'source'; position: Vec2; strength: number }
  | { type: 'sink'; position: Vec2; strength: number }
  | { type: 'vortex'; position: Vec2; circulation: number }
  | { type: 'doublet'; position: Vec2; strength: number; orientation: number };

export type ElementKind = Element['type'];

export interface SceneElement {
  id: string;
  name: string;
  visible: boolean;
  locked: boolean;
  element: Element;
}

export type NodeType = 'smooth' | 'corner' | 'symmetric';

/**
 * A Bézier anchor. Handles are stored **relative to the node** so moving a node
 * moves its handles; a zero offset means "no handle" (a straight neighbour).
 */
export interface BezierNode {
  id: string;
  position: Vec2;
  inHandle: Vec2;
  outHandle: Vec2;
  nodeType: NodeType;
}

/**
 * Authoritative editable shape (PRD2 §8–§10). Cubic segment `i` runs from node
 * `i` to node `i+1` (wrapping when `closed`); segments are derived, never
 * stored, so they cannot disagree with the nodes. Panels are derived from this
 * by sampling — the solver never sees the Bézier data.
 */
export interface BezierGeometry {
  kind: 'bezier';
  closed: boolean;
  nodes: BezierNode[];
  /** Max distance to the source points when fitted from imported coordinates. */
  fitError?: number | null;
}

export type BodyGeometry =
  | BezierGeometry
  | { kind: 'points'; points: Vec2[] }
  | { kind: 'naca4'; code: string; chord: number }
  | { kind: 'circle'; radius: number }
  | { kind: 'ellipse'; semiAxisX: number; semiAxisY: number }
  | { kind: 'joukowski'; thickness: number; camber: number };

export type BodyGeometryKind = BodyGeometry['kind'];

export type CirculationSetting =
  | { mode: 'auto' }
  | { mode: 'kutta' }
  | { mode: 'none' }
  | { mode: 'prescribed'; circulation: number };

export type PanelDistribution = 'asImported' | 'uniform' | 'cosine' | 'curvature' | 'auto';

export interface PanelSettings {
  count: number;
  distribution: PanelDistribution;
}

export interface ReferenceOverride {
  chord?: number | null;
  /** Body-local coordinates. */
  point?: Vec2 | null;
}

export interface SceneBody {
  id: string;
  name: string;
  visible: boolean;
  locked: boolean;
  geometry: BodyGeometry;
  position: Vec2;
  /** Rotation about the body-local origin [rad], counter-clockwise. */
  rotation: number;
  scale: number;
  circulation: CirculationSetting;
  panels: PanelSettings;
  reference: ReferenceOverride;
  sourceName?: string | null;
}

/** Newest format this build reads. v2 adds `bezier` body geometry (PRD2 §45); v1 files load unchanged. */
export const SCENE_FORMAT_VERSION = 2 as const;
/** Written for scenes without Bézier bodies, so they stay readable by the Rust solver and older builds. */
export const BASE_FORMAT_VERSION = 1 as const;
/** The format the Rust solver understands; Bézier bodies are sampled to `points` before it sees them. */
export const SOLVER_FORMAT_VERSION = 1 as const;

export interface Scene {
  version: number;
  name: string;
  conditions: FlowConditions;
  elements: SceneElement[];
  bodies: SceneBody[];
}

export type SceneObject =
  | { kind: 'element'; object: SceneElement }
  | { kind: 'body'; object: SceneBody };

// ───────────────────────────── Solution ─────────────────────────────

export type SolveStatus = 'success' | 'warning' | 'error';

export interface Warning {
  code: string;
  severity: 'info' | 'warning';
  message: string;
  objectId: string | null;
}

export interface ReferenceValues {
  chord: number;
  area: number;
  point: Vec2;
}

export interface BodyForces {
  fx: number;
  fy: number;
  lift: number;
  drag: number;
  /** Pitching moment, positive nose-up [N·m/m]. */
  moment: number;
  cl: number | null;
  cd: number | null;
  cm: number | null;
  circulation: number;
  liftKuttaJoukowski: number;
  liftConsistency: number;
  reference: ReferenceValues;
}

export interface SurfacePoint {
  position: Vec2;
  normal: Vec2;
  tangent: Vec2;
  panelLength: number;
  arcLength: number;
  xOverC: number;
  surface: 'upper' | 'lower';
  tangentialVelocity: number;
  normalVelocity: number;
  pressure: number;
  cp: number | null;
  sourceStrength: number;
}

export type CirculationMode =
  | { mode: 'kutta' }
  | { mode: 'none' }
  | { mode: 'prescribed'; circulation: number };

export interface CornerInfo {
  index: number;
  turnAngle: number;
  includedAngle: number;
  position: Vec2;
}

export interface BodyResult {
  id: string;
  name: string;
  index: number;
  forces: BodyForces;
  surface: SurfacePoint[];
  panelCount: number;
  circulationMode: CirculationMode;
  kuttaApplicable: boolean;
  trailingEdge: CornerInfo | null;
  /** World-space contour. */
  polygon: Vec2[];
  vortexStrength: number;
  netSourceOutflow: number;
  notes: string[];
}

export interface TotalForces {
  fx: number;
  fy: number;
  lift: number;
  drag: number;
  moment: number;
  circulation: number;
}

export interface Timings {
  prepareMs: number;
  assembleMs: number;
  solveMs: number;
  forcesMs: number;
  totalMs: number;
}

export interface PanelWarning {
  code: string;
  message: string;
  body: number | null;
}

export interface PanelDiagnostics {
  panelCount: number;
  bodyCount: number;
  unknownCount: number;
  conditionNumber: number;
  relativeResidual: number;
  maxNormalVelocity: number;
  minPanelLength: number;
  maxPanelLength: number;
  warnings: PanelWarning[];
}

export interface Diagnostics {
  panel: PanelDiagnostics | null;
  timings: Timings;
  systemReused: boolean;
  elementCount: number;
  bodyCount: number;
  totalCirculation: number;
  netOutflow: number;
}

/** Lagally force on an elementary singularity: the force needed to hold it fixed. */
export interface ElementResult {
  id: string;
  name: string;
  position: Vec2;
  /** Force per unit span [N/m]. */
  force: Vec2;
  lift: number;
  drag: number;
  /** Velocity induced at the element by everything else [m/s]. */
  externalVelocity: Vec2;
}

export interface Solution {
  status: SolveStatus;
  bodies: BodyResult[];
  elements: ElementResult[];
  total: TotalForces;
  diagnostics: Diagnostics;
  warnings: Warning[];
  error: string | null;
  assumptions: string[];
}

// ───────────────────────────── Fields ─────────────────────────────

export type FieldType =
  | 'velocityMagnitude'
  | 'velocityU'
  | 'velocityV'
  | 'pressure'
  | 'cp'
  | 'vorticity'
  | 'potential'
  | 'streamFunction';

export interface Bounds {
  min: Vec2;
  max: Vec2;
}

export interface FieldRequest {
  field: FieldType;
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
  nx: number;
  ny: number;
  /** Rendering preset (softened, far-field approximated, near-wall masked). */
  render: boolean;
}

export interface ScalarFieldResult {
  values: Float32Array;
  mask: Uint8Array;
  /** World-space extent of the sample grid; attached by the worker. */
  bounds: Bounds;
  nx: number;
  ny: number;
  min: number;
  max: number;
  robustMin: number;
  robustMax: number;
  softened: boolean;
  field: FieldType;
}

export interface VectorFieldResult {
  u: Float32Array;
  v: Float32Array;
  mask: Uint8Array;
  /** World-space extent of the sample grid; attached by the worker. */
  bounds: Bounds;
  nx: number;
  ny: number;
  maxMagnitude: number;
}

export type SeedStrategy = 'inflow' | 'grid' | 'evenlySpaced';

export interface SeedingConfig {
  strategy: SeedStrategy;
  count: number;
  separation: number;
  stopRatio: number;
  maxLines: number;
}

export interface StreamlineRequest {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
  seeding?: SeedingConfig;
  /** Manual seeds as a flat `[x0, y0, x1, y1, …]` list. */
  seeds?: number[];
  maxSteps?: number;
}

export interface StreamlinesResult {
  /** `x, y, speed` triples. */
  data: Float32Array;
  /** `offsets[i]..offsets[i+1]` is line `i`'s point range; length = count + 1. */
  offsets: Uint32Array;
  seedIndex: Uint32Array;
  count: number;
}

export interface Probe {
  position: Vec2;
  velocity: Vec2;
  speed: number;
  pressure: number;
  cp: number | null;
  potential: number;
  streamFunction: number;
  bodyId: string | null;
}

// ───────────────────────────── Sweeps ─────────────────────────────

export type SweepParameterKind =
  | 'angleOfAttack'
  | 'freestreamSpeed'
  | 'bodyCirculation'
  | 'elementStrength';

export interface SweepConfig {
  parameter: SweepParameterKind;
  bodyId?: string;
  elementId?: string;
  start: number;
  end: number;
  steps: number;
}

export interface SweepBodyPoint {
  id: string;
  name: string;
  lift: number;
  drag: number;
  moment: number;
  cl: number | null;
  cd: number | null;
  cm: number | null;
  circulation: number;
}

export interface SweepPoint {
  index: number;
  value: number;
  status: SolveStatus;
  bodies: SweepBodyPoint[];
  totalLift: number;
  totalDrag: number;
  error: string | null;
}

export interface SweepResult {
  config: SweepConfig;
  points: SweepPoint[];
  completed: boolean;
}

// ───────────────────────────── Metadata ─────────────────────────────

export type ParameterKind = 'scalar' | 'angle' | 'positionX' | 'positionY';

export interface ParameterDescriptor {
  /** Field path on the element, e.g. `strength` or `position.x`. */
  key: string;
  label: string;
  symbol: string;
  unit: string;
  kind: ParameterKind;
  softMin: number;
  softMax: number;
  step: number;
  default: number;
  help: string;
}

export interface HelpTopic {
  id: string;
  title: string;
  meaning: string;
  importance: string;
  definition: string;
}

export interface ElementDescriptor {
  kind: ElementKind;
  tag: ElementKind;
  name: string;
  glyph: string;
  summary: string;
  parameters: ParameterDescriptor[];
  help: HelpTopic;
}

export interface FieldInfo {
  id: FieldType;
  label: string;
  symbol: string;
  unit: string;
  diverging: boolean;
  note: string;
}

export interface SolverMetadata {
  version: string;
  descriptors: ElementDescriptor[];
  helpTopics: HelpTopic[];
  fieldTypes: FieldInfo[];
  assumptions: string[];
}

// ───────────────────────────── Geometry import ─────────────────────────────

export type IssueSeverity = 'info' | 'warning' | 'error';

export interface GeometryIssue {
  kind: string;
  severity: IssueSeverity;
  message: string;
  index: number | null;
  additionalOccurrences: number;
}

export interface ImportResult {
  points: Vec2[];
  name: string | null;
  notes: string[];
  issues: GeometryIssue[];
  kuttaApplicable: boolean;
  trailingEdge: CornerInfo | null;
  originalPointCount: number;
  panelCount: number;
  chord: number;
}

export interface ValidationResult {
  points: Vec2[];
  name: string | null;
  notes: string[];
  issues: GeometryIssue[];
  hasErrors: boolean;
}
