/**
 * Perceptually ordered colour maps (PRD §36: "scientifically appropriate and
 * not relying exclusively on red/green").
 *
 * Sequential maps (viridis, magma, cividis, turbo, greys) for magnitudes;
 * the diverging coolwarm map for signed fields centred on zero. Viridis,
 * magma and turbo use published polynomial fits; the others interpolate
 * control points.
 */
import type { ColormapId } from '@/state/visualizationStore';

export type RGB = [number, number, number];

type Poly = { c: RGB[] };

const VIRIDIS: Poly = {
  c: [
    [0.2777273272234177, 0.005407344544966578, 0.3340998053353061],
    [0.1050930431085774, 1.404613529898575, 1.384590162594685],
    [-0.3308618287255563, 0.214847559468213, 0.09509516302823659],
    [-4.634230498983486, -5.799100973351585, -19.33244095627987],
    [6.228269936347081, 14.17993336680509, 56.69055260068105],
    [4.776384997670288, -13.74514537774601, -65.35303263337234],
    [-5.435455855934631, 4.645852612178535, 26.3124352495832],
  ],
};

const MAGMA: Poly = {
  c: [
    [-0.002136485053939582, -0.000749655052795221, -0.005386127855323933],
    [0.2516605407371642, 0.6775232436837668, 2.494026599312351],
    [8.353717279216625, -3.577719514958484, 0.3144679030132573],
    [-27.66873308576866, 14.26473078096533, -13.64921318813922],
    [52.17613981234068, -27.94360607168351, 12.94416944238394],
    [-50.76852536473588, 29.04658282127291, 4.23415299384598],
    [18.65570506591883, -11.48977351997711, -5.601961508734096],
  ],
};

function evalPoly(p: Poly, t: number): RGB {
  let r = 0;
  let g = 0;
  let b = 0;
  let tn = 1;
  for (const [cr, cg, cb] of p.c) {
    r += cr * tn;
    g += cg * tn;
    b += cb * tn;
    tn *= t;
  }
  return [clamp01(r), clamp01(g), clamp01(b)];
}

function turbo(t: number): RGB {
  const r = 0.13572138 + 4.6153926 * t - 42.66032258 * t ** 2 + 132.13108234 * t ** 3 - 152.94239396 * t ** 4 + 59.28637943 * t ** 5;
  const g = 0.09140261 + 2.19418839 * t + 4.84296658 * t ** 2 - 14.18503333 * t ** 3 + 4.27729857 * t ** 4 + 2.82956604 * t ** 5;
  const b = 0.1066733 + 12.64194608 * t - 60.58204836 * t ** 2 + 110.36276771 * t ** 3 - 89.90310912 * t ** 4 + 27.34824973 * t ** 5;
  return [clamp01(r), clamp01(g), clamp01(b)];
}

const COOLWARM_STOPS: [number, RGB][] = [
  [0.0, [59 / 255, 76 / 255, 192 / 255]],
  [0.25, [124 / 255, 159 / 255, 249 / 255]],
  [0.5, [221 / 255, 221 / 255, 221 / 255]],
  [0.75, [245 / 255, 156 / 255, 125 / 255]],
  [1.0, [180 / 255, 4 / 255, 38 / 255]],
];

const CIVIDIS_STOPS: [number, RGB][] = [
  [0.0, [0 / 255, 32 / 255, 76 / 255]],
  [0.2, [45 / 255, 70 / 255, 110 / 255]],
  [0.4, [100 / 255, 104 / 255, 118 / 255]],
  [0.6, [150 / 255, 141 / 255, 112 / 255]],
  [0.8, [204 / 255, 185 / 255, 94 / 255]],
  [1.0, [255 / 255, 234 / 255, 70 / 255]],
];

const GREYS_STOPS: [number, RGB][] = [
  [0.0, [0.09, 0.09, 0.11]],
  [1.0, [0.93, 0.93, 0.94]],
];

function interpolate(stops: [number, RGB][], t: number): RGB {
  if (t <= stops[0][0]) return stops[0][1];
  for (let i = 1; i < stops.length; i++) {
    const [t1, c1] = stops[i];
    if (t <= t1) {
      const [t0, c0] = stops[i - 1];
      const f = (t - t0) / (t1 - t0);
      return [c0[0] + (c1[0] - c0[0]) * f, c0[1] + (c1[1] - c0[1]) * f, c0[2] + (c1[2] - c0[2]) * f];
    }
  }
  return stops[stops.length - 1][1];
}

function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

export function sampleColormap(id: ColormapId, t: number): RGB {
  const u = clamp01(Number.isFinite(t) ? t : 0);
  switch (id) {
    case 'viridis':
      return evalPoly(VIRIDIS, u);
    case 'magma':
      return evalPoly(MAGMA, u);
    case 'turbo':
      return turbo(u);
    case 'coolwarm':
      return interpolate(COOLWARM_STOPS, u);
    case 'cividis':
      return interpolate(CIVIDIS_STOPS, u);
    case 'greys':
      return interpolate(GREYS_STOPS, u);
  }
}

const lutCache = new Map<ColormapId, Uint8Array>();

/** 256-entry RGBA lookup table, cached. */
export function colormapLUT(id: ColormapId): Uint8Array {
  const cached = lutCache.get(id);
  if (cached) return cached;
  const lut = new Uint8Array(256 * 4);
  for (let i = 0; i < 256; i++) {
    const [r, g, b] = sampleColormap(id, i / 255);
    lut[i * 4] = Math.round(r * 255);
    lut[i * 4 + 1] = Math.round(g * 255);
    lut[i * 4 + 2] = Math.round(b * 255);
    lut[i * 4 + 3] = 255;
  }
  lutCache.set(id, lut);
  return lut;
}

export function cssColor(c: RGB, alpha = 1): string {
  return `rgba(${Math.round(c[0] * 255)}, ${Math.round(c[1] * 255)}, ${Math.round(c[2] * 255)}, ${alpha})`;
}

export const COLORMAPS: { id: ColormapId; label: string; diverging: boolean }[] = [
  { id: 'viridis', label: 'Viridis', diverging: false },
  { id: 'magma', label: 'Magma', diverging: false },
  { id: 'cividis', label: 'Cividis', diverging: false },
  { id: 'turbo', label: 'Turbo', diverging: false },
  { id: 'coolwarm', label: 'Cool–warm', diverging: true },
  { id: 'greys', label: 'Greys', diverging: false },
];

/** Default map per field character: diverging fields get coolwarm. */
export function defaultColormapFor(diverging: boolean): ColormapId {
  return diverging ? 'coolwarm' : 'viridis';
}
