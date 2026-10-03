import { describe, expect, it } from 'vitest';
import { COLORMAPS, colormapLUT, defaultColormapFor, sampleColormap } from './colormaps';

describe('colormaps', () => {
  it('produces in-range colours for every map across the domain', () => {
    for (const { id } of COLORMAPS) {
      for (let i = 0; i <= 20; i++) {
        const c = sampleColormap(id, i / 20);
        for (const ch of c) {
          expect(ch).toBeGreaterThanOrEqual(0);
          expect(ch).toBeLessThanOrEqual(1);
        }
      }
    }
  });

  it('is monotonic in luminance for sequential maps', () => {
    const lum = ([r, g, b]: [number, number, number]) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
    for (const id of ['viridis', 'magma', 'cividis', 'greys'] as const) {
      let prev = -1;
      for (let i = 0; i <= 50; i++) {
        const l = lum(sampleColormap(id, i / 50));
        expect(l).toBeGreaterThanOrEqual(prev - 1e-6);
        prev = l;
      }
    }
  });

  it('clamps out-of-range and non-finite inputs', () => {
    expect(sampleColormap('viridis', -3)).toEqual(sampleColormap('viridis', 0));
    expect(sampleColormap('viridis', 9)).toEqual(sampleColormap('viridis', 1));
    expect(sampleColormap('viridis', NaN)).toEqual(sampleColormap('viridis', 0));
  });

  it('caches a 256-entry opaque RGBA table', () => {
    const lut = colormapLUT('turbo');
    expect(lut.length).toBe(1024);
    expect(lut[3]).toBe(255);
    expect(colormapLUT('turbo')).toBe(lut);
  });

  it('picks a diverging map for signed fields', () => {
    expect(COLORMAPS.find((c) => c.id === defaultColormapFor(true))?.diverging).toBe(true);
    expect(COLORMAPS.find((c) => c.id === defaultColormapFor(false))?.diverging).toBe(false);
  });
});
