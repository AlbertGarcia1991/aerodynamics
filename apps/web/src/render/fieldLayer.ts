/**
 * WebGL2 scalar-field renderer (DEC-003).
 *
 * The sampled field is uploaded once per solve as an `R32F` texture with an
 * `R8` body mask; colour mapping, range clipping, contour banding and isolines
 * are evaluated in the fragment shader. Changing the colour map or the range
 * therefore costs one uniform update and zero recompute.
 *
 * Bilinear interpolation is done by hand from four `texelFetch` calls so that
 * masked cells can be excluded from the weights — otherwise body interiors
 * would bleed into the fluid at the surface.
 */
import type { Bounds, ScalarFieldResult } from '@/domain/types';
import type { Viewport } from '@/canvas/viewport';

const VERT = `#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

const FRAG = `#version 300 es
precision highp float;
precision highp int;
uniform sampler2D u_field;
uniform sampler2D u_mask;
uniform sampler2D u_lut;
uniform vec2 u_canvas;
uniform vec2 u_center;
uniform float u_scale;
uniform vec4 u_bounds;
uniform ivec2 u_dims;
uniform vec2 u_range;
uniform float u_opacity;
uniform float u_bands;
uniform float u_isolines;
uniform vec3 u_isoColor;
in vec2 v_uv;
out vec4 outColor;

float fetchW(ivec2 ij, float w, inout float wsum) {
  // Mask codes (R8, normalised): 0 fluid, 1/255 body, 2/255 near-wall (filled).
  // Only the body interior is excluded from the weights.
  float m = texelFetch(u_mask, ij, 0).r * 255.0;
  float keep = (m > 0.5 && m < 1.5) ? 0.0 : 1.0;
  float ww = w * keep;
  wsum += ww;
  return ww * texelFetch(u_field, ij, 0).r;
}

void main() {
  vec2 world = vec2(u_center.x + (v_uv.x - 0.5) * u_canvas.x / u_scale,
                    u_center.y + (v_uv.y - 0.5) * u_canvas.y / u_scale);
  vec2 f = (world - u_bounds.xy) / (u_bounds.zw - u_bounds.xy);
  if (f.x < 0.0 || f.x > 1.0 || f.y < 0.0 || f.y > 1.0) discard;
  vec2 g = f * vec2(u_dims - 1);
  ivec2 i0 = clamp(ivec2(floor(g)), ivec2(0), u_dims - 2);
  vec2 t = clamp(g - vec2(i0), 0.0, 1.0);
  float wsum = 0.0;
  float v = 0.0;
  v += fetchW(i0, (1.0 - t.x) * (1.0 - t.y), wsum);
  v += fetchW(i0 + ivec2(1, 0), t.x * (1.0 - t.y), wsum);
  v += fetchW(i0 + ivec2(0, 1), (1.0 - t.x) * t.y, wsum);
  v += fetchW(i0 + ivec2(1, 1), t.x * t.y, wsum);
  if (wsum <= 0.001) discard;
  v /= wsum;
  if (isnan(v) || isinf(v)) discard;
  float tn = clamp((v - u_range.x) / (u_range.y - u_range.x), 0.0, 1.0);
  float tq = tn;
  if (u_bands > 0.5) tq = (floor(tn * u_bands) + 0.5) / u_bands;
  vec4 col = texture(u_lut, vec2(tq, 0.5));
  if (u_isolines > 0.5 && u_bands > 0.5) {
    float s = tn * u_bands;
    float edge = min(fract(s), 1.0 - fract(s));
    float aa = max(fwidth(s), 1e-4);
    float line = 1.0 - smoothstep(0.0, aa * 1.6, edge);
    // Suppress lines at the clipped extremes, where tn saturates.
    line *= step(0.001, tn) * step(tn, 0.999);
    col.rgb = mix(col.rgb, u_isoColor, line * 0.8);
  }
  outColor = vec4(col.rgb * u_opacity, u_opacity);
}`;

export interface FieldRenderOptions {
  vp: Viewport;
  bounds: Bounds;
  range: [number, number];
  opacity: number;
  bands: number;
  isolines: boolean;
  isoColor: [number, number, number];
}

export class FieldLayer {
  private gl: WebGL2RenderingContext | null;
  private program: WebGLProgram | null = null;
  private uniforms = new Map<string, WebGLUniformLocation | null>();
  private texField: WebGLTexture | null = null;
  private texMask: WebGLTexture | null = null;
  private texLut: WebGLTexture | null = null;
  private dims: [number, number] = [0, 0];
  private hasField = false;
  readonly supported: boolean;

  constructor(private canvas: HTMLCanvasElement) {
    this.gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false, preserveDrawingBuffer: true });
    this.supported = !!this.gl;
    if (this.gl) this.init(this.gl);
  }

  private init(gl: WebGL2RenderingContext) {
    const compile = (type: number, src: string) => {
      const sh = gl.createShader(type)!;
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
        const log = gl.getShaderInfoLog(sh);
        gl.deleteShader(sh);
        throw new Error(`Shader compile failed: ${log}`);
      }
      return sh;
    };
    const prog = gl.createProgram()!;
    gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
    gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(`Program link failed: ${gl.getProgramInfoLog(prog)}`);
    this.program = prog;
    for (const name of ['u_field', 'u_mask', 'u_lut', 'u_canvas', 'u_center', 'u_scale', 'u_bounds', 'u_dims', 'u_range', 'u_opacity', 'u_bands', 'u_isolines', 'u_isoColor']) {
      this.uniforms.set(name, gl.getUniformLocation(prog, name));
    }
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, 'a_pos');
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);

    const mk = () => {
      const t = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      return t;
    };
    this.texField = mk();
    this.texMask = mk();
    this.texLut = mk();
    gl.bindTexture(gl.TEXTURE_2D, this.texLut);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  }

  resize(width: number, height: number, dpr: number): void {
    const w = Math.max(1, Math.round(width * dpr));
    const h = Math.max(1, Math.round(height * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
    this.gl?.viewport(0, 0, w, h);
  }

  setField(f: ScalarFieldResult | null): void {
    const gl = this.gl;
    if (!gl) return;
    if (!f) {
      this.hasField = false;
      return;
    }
    // NaN cannot be stored meaningfully; the mask already marks those cells.
    const values = new Float32Array(f.values.length);
    for (let i = 0; i < values.length; i++) values[i] = Number.isFinite(f.values[i]) ? f.values[i] : 0;
    gl.bindTexture(gl.TEXTURE_2D, this.texField);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R32F, f.nx, f.ny, 0, gl.RED, gl.FLOAT, values);
    gl.bindTexture(gl.TEXTURE_2D, this.texMask);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, f.nx, f.ny, 0, gl.RED, gl.UNSIGNED_BYTE, f.mask);
    this.dims = [f.nx, f.ny];
    this.hasField = true;
  }

  setColormap(lut: Uint8Array): void {
    const gl = this.gl;
    if (!gl) return;
    gl.bindTexture(gl.TEXTURE_2D, this.texLut);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 256, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, lut);
  }

  clear(): void {
    const gl = this.gl;
    if (!gl) return;
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }

  render(o: FieldRenderOptions): void {
    const gl = this.gl;
    if (!gl || !this.program) return;
    this.clear();
    if (!this.hasField) return;
    gl.useProgram(this.program);
    const u = (n: string) => this.uniforms.get(n) ?? null;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texField);
    gl.uniform1i(u('u_field'), 0);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.texMask);
    gl.uniform1i(u('u_mask'), 1);
    gl.activeTexture(gl.TEXTURE2);
    gl.bindTexture(gl.TEXTURE_2D, this.texLut);
    gl.uniform1i(u('u_lut'), 2);
    gl.uniform2f(u('u_canvas'), o.vp.width, o.vp.height);
    gl.uniform2f(u('u_center'), o.vp.center.x, o.vp.center.y);
    gl.uniform1f(u('u_scale'), o.vp.scale);
    gl.uniform4f(u('u_bounds'), o.bounds.min.x, o.bounds.min.y, o.bounds.max.x, o.bounds.max.y);
    gl.uniform2i(u('u_dims'), this.dims[0], this.dims[1]);
    const lo = o.range[0];
    const hi = o.range[1] > lo ? o.range[1] : lo + 1e-9;
    gl.uniform2f(u('u_range'), lo, hi);
    gl.uniform1f(u('u_opacity'), o.opacity);
    gl.uniform1f(u('u_bands'), o.bands);
    gl.uniform1f(u('u_isolines'), o.isolines ? 1 : 0);
    gl.uniform3f(u('u_isoColor'), o.isoColor[0], o.isoColor[1], o.isoColor[2]);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  dispose(): void {
    const gl = this.gl;
    if (!gl) return;
    gl.deleteTexture(this.texField);
    gl.deleteTexture(this.texMask);
    gl.deleteTexture(this.texLut);
    if (this.program) gl.deleteProgram(this.program);
    this.gl = null;
  }
}
