/**
 * A small scientific line chart in SVG: axes with nice ticks, grid, multiple
 * series, optional inverted y (for Cp), and a hover read-out. Written in-house
 * so that it matches the app's typography and themes exactly and stays light.
 */
import { useMemo, useRef, useState } from 'react';
import { downloadText } from '@/export/download';

/** Serialise a chart's SVG with the current theme colours baked in (PRD §52 "plots as SVG"). */
export function exportSvg(svg: SVGSVGElement, filename: string): void {
  const clone = svg.cloneNode(true) as SVGSVGElement;
  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  clone.setAttribute('width', '960');
  clone.setAttribute('height', '450');
  const cs = getComputedStyle(document.documentElement);
  const resolve = (v: string) => v.replace(/var\((--[\w-]+)\)/g, (_, name: string) => cs.getPropertyValue(name).trim() || '#888');
  clone.querySelectorAll<SVGElement>('*').forEach((el) => {
    for (const attr of ['stroke', 'fill']) {
      const v = el.getAttribute(attr);
      if (v && v.includes('var(')) el.setAttribute(attr, resolve(v));
    }
  });
  const style = document.createElementNS('http://www.w3.org/2000/svg', 'style');
  style.textContent = `text{font-family:ui-monospace,Menlo,monospace;font-size:11px;fill:${cs.getPropertyValue('--text-muted').trim()}}.axis line,.axis path{stroke:${cs.getPropertyValue('--border-strong').trim()}}.gridline{stroke:${cs.getPropertyValue('--border').trim()}}.series{fill:none;stroke-width:1.8}.marker{fill:${cs.getPropertyValue('--surface').trim()};stroke-width:1.5}`;
  clone.insertBefore(style, clone.firstChild);
  const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
  rect.setAttribute('width', '100%');
  rect.setAttribute('height', '100%');
  rect.setAttribute('fill', cs.getPropertyValue('--surface').trim() || '#fff');
  clone.insertBefore(rect, clone.firstChild);
  downloadText(filename, new XMLSerializer().serializeToString(clone), 'image/svg+xml');
}

export interface Series {
  id: string;
  label: string;
  color: string;
  points: { x: number; y: number }[];
  /** Draw markers at points. */
  markers?: boolean;
  dashed?: boolean;
}

export interface LineChartProps {
  /** Enables a small "SVG" download affordance with this file name. */
  exportName?: string;
  series: Series[];
  xLabel: string;
  yLabel: string;
  invertY?: boolean;
  /** Force a symmetric y range about zero. */
  symmetricY?: boolean;
  xDomain?: [number, number];
  yDomain?: [number, number];
  height?: number;
  formatX?: (v: number) => string;
  formatY?: (v: number) => string;
  /** Horizontal reference lines, e.g. y = 0. */
  referenceY?: number[];
  ariaLabel?: string;
}

function niceTicks(min: number, max: number, count = 5): number[] {
  if (!(max > min)) return [min];
  const raw = (max - min) / count;
  const pow = Math.pow(10, Math.floor(Math.log10(raw)));
  const m = raw / pow;
  const step = (m < 1.5 ? 1 : m < 3 ? 2 : m < 7 ? 5 : 10) * pow;
  const out: number[] = [];
  for (let v = Math.ceil(min / step) * step; v <= max + 1e-9 * step; v += step) out.push(parseFloat(v.toPrecision(10)));
  return out;
}

const defaultFmt = (v: number) => (Math.abs(v) >= 1000 || (Math.abs(v) < 0.01 && v !== 0) ? v.toExponential(1) : parseFloat(v.toPrecision(4)).toString());

export function LineChart({
  exportName,
  series,
  xLabel,
  yLabel,
  invertY = false,
  symmetricY = false,
  xDomain,
  yDomain,
  formatX = defaultFmt,
  formatY = defaultFmt,
  referenceY = [],
  ariaLabel,
}: LineChartProps) {
  const [hover, setHover] = useState<{ px: number; x: number; values: { label: string; y: number; color: string }[] } | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const W = 640;
  const H = 300;
  const m = { l: 56, r: 16, t: 14, b: 40 };

  const { xs, ys } = useMemo(() => {
    const pts = series.flatMap((s) => s.points).filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y));
    const xmin = xDomain?.[0] ?? Math.min(...pts.map((p) => p.x), 0);
    const xmax = xDomain?.[1] ?? Math.max(...pts.map((p) => p.x), 1);
    let ymin = yDomain?.[0] ?? Math.min(...pts.map((p) => p.y));
    let ymax = yDomain?.[1] ?? Math.max(...pts.map((p) => p.y));
    if (!Number.isFinite(ymin) || !Number.isFinite(ymax)) {
      ymin = -1;
      ymax = 1;
    }
    if (ymax - ymin < 1e-12) {
      ymin -= 1;
      ymax += 1;
    }
    if (!yDomain) {
      const pad = (ymax - ymin) * 0.06;
      ymin -= pad;
      ymax += pad;
    }
    if (symmetricY) {
      const a = Math.max(Math.abs(ymin), Math.abs(ymax));
      ymin = -a;
      ymax = a;
    }
    return { xs: [xmin, xmax] as [number, number], ys: [ymin, ymax] as [number, number] };
  }, [series, xDomain, yDomain, symmetricY]);

  const sx = (x: number) => m.l + ((x - xs[0]) / (xs[1] - xs[0])) * (W - m.l - m.r);
  const sy = (y: number) => {
    const f = (y - ys[0]) / (ys[1] - ys[0]);
    return invertY ? m.t + f * (H - m.t - m.b) : H - m.b - f * (H - m.t - m.b);
  };

  const xt = niceTicks(xs[0], xs[1], 6);
  const yt = niceTicks(ys[0], ys[1], 5);

  const onMove = (e: React.MouseEvent<SVGSVGElement>) => {
    const svg = svgRef.current;
    if (!svg) return;
    const rect = svg.getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * W;
    const x = xs[0] + ((px - m.l) / (W - m.l - m.r)) * (xs[1] - xs[0]);
    const values = series
      .map((s) => {
        let best: { x: number; y: number } | null = null;
        let bd = Infinity;
        for (const p of s.points) {
          const d = Math.abs(p.x - x);
          if (d < bd) {
            bd = d;
            best = p;
          }
        }
        return best ? { label: s.label, y: best.y, color: s.color } : null;
      })
      .filter((v): v is { label: string; y: number; color: string } => v !== null);
    setHover(values.length ? { px, x, values } : null);
  };

  return (
    <>
    {exportName && (
      <button
        className="btn btn--ghost btn--sm"
        style={{ position: 'absolute', right: 8, top: 4, zIndex: 2 }}
        onClick={() => svgRef.current && exportSvg(svgRef.current, exportName)}
        aria-label="Download chart as SVG"
        title="Download chart as SVG"
      >
        SVG
      </button>
    )}
    <svg
      ref={svgRef}
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={ariaLabel ?? `${yLabel} versus ${xLabel}`}
      onMouseMove={onMove}
      onMouseLeave={() => setHover(null)}
    >
      <g className="axis">
        {yt.map((v) => (
          <g key={`y${v}`}>
            <line className="gridline" x1={m.l} x2={W - m.r} y1={sy(v)} y2={sy(v)} />
            <text x={m.l - 8} y={sy(v) + 3.5} textAnchor="end">{formatY(v)}</text>
          </g>
        ))}
        {xt.map((v) => (
          <g key={`x${v}`}>
            <line className="gridline" y1={m.t} y2={H - m.b} x1={sx(v)} x2={sx(v)} />
            <text x={sx(v)} y={H - m.b + 16} textAnchor="middle">{formatX(v)}</text>
          </g>
        ))}
        <path d={`M${m.l},${m.t}V${H - m.b}H${W - m.r}`} fill="none" />
        {referenceY.map((v) => v >= ys[0] && v <= ys[1] && (
          <line key={`ref${v}`} x1={m.l} x2={W - m.r} y1={sy(v)} y2={sy(v)} stroke="var(--text-faint)" strokeDasharray="4 4" />
        ))}
        <text x={(m.l + W - m.r) / 2} y={H - 6} textAnchor="middle" fontWeight={600}>{xLabel}</text>
        <text transform={`translate(14 ${(m.t + H - m.b) / 2}) rotate(-90)`} textAnchor="middle" fontWeight={600}>
          {yLabel}{invertY ? ' (inverted)' : ''}
        </text>
      </g>
      {series.map((s) => {
        const pts = s.points.filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y));
        if (pts.length === 0) return null;
        const d = pts.map((p, i) => `${i === 0 ? 'M' : 'L'}${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join('');
        return (
          <g key={s.id}>
            <path className="series" d={d} stroke={s.color} strokeDasharray={s.dashed ? '5 4' : undefined} />
            {s.markers && pts.map((p, i) => <circle key={i} className="marker" cx={sx(p.x)} cy={sy(p.y)} r={2.6} stroke={s.color} />)}
          </g>
        );
      })}
      {hover && (
        <g pointerEvents="none">
          <line x1={hover.px} x2={hover.px} y1={m.t} y2={H - m.b} stroke="var(--text-faint)" />
          <rect x={Math.min(hover.px + 8, W - 190)} y={m.t + 4} width={182} height={16 + hover.values.length * 15} rx={5} fill="var(--surface)" stroke="var(--border)" />
          <text x={Math.min(hover.px + 16, W - 182)} y={m.t + 18} fill="var(--text)">{xLabel.split(' ')[0]} = {formatX(hover.x)}</text>
          {hover.values.map((v, i) => (
            <text key={i} x={Math.min(hover.px + 16, W - 182)} y={m.t + 33 + i * 15} fill={v.color}>
              {v.label}: {formatY(v.y)}
            </text>
          ))}
        </g>
      )}
    </svg>
    </>
  );
}
