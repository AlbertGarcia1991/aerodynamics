/**
 * Numeric input with an optional slider (PRD §33: keyboard input, arrow keys,
 * sliders where useful). Commits typed values on blur/Enter; slider drags are
 * *transient* updates inside a transaction so one gesture = one undo entry.
 */
import { useEffect, useState } from 'react';
import { useSimulationStore } from '@/state/simulationStore';

export interface NumberFieldProps {
  label: string;
  symbol?: string;
  unit?: string;
  value: number;
  onChange: (value: number, transient: boolean) => void;
  step?: number;
  min?: number;
  max?: number;
  /** Show a slider between `sliderMin` and `sliderMax` (soft bounds). */
  sliderMin?: number;
  sliderMax?: number;
  digits?: number;
  disabled?: boolean;
  help?: string;
  /** Bracket slider drags in an undo transaction (default true). */
  transactional?: boolean;
}

function format(v: number, digits: number): string {
  if (!Number.isFinite(v)) return '';
  const abs = Math.abs(v);
  if (abs !== 0 && (abs >= 1e6 || abs < 1e-4)) return v.toExponential(Math.max(1, digits - 1));
  return String(parseFloat(v.toFixed(digits)));
}

export function NumberField({
  label,
  symbol,
  unit,
  value,
  onChange,
  step = 0.1,
  min = -Infinity,
  max = Infinity,
  sliderMin,
  sliderMax,
  digits = 4,
  disabled,
  help,
  transactional = true,
}: NumberFieldProps) {
  const [text, setText] = useState(format(value, digits));
  const [focused, setFocused] = useState(false);
  useEffect(() => {
    if (!focused) setText(format(value, digits));
  }, [value, digits, focused]);

  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  const commitText = () => {
    const parsed = Number(text.replace(',', '.'));
    if (Number.isFinite(parsed) && parsed !== value) onChange(clamp(parsed), false);
    else setText(format(value, digits));
  };
  const nudge = (dir: number, big: boolean) => {
    const next = clamp(value + dir * step * (big ? 10 : 1));
    onChange(parseFloat(next.toFixed(10)), false);
  };

  const hasSlider = sliderMin !== undefined && sliderMax !== undefined;
  const sMin = sliderMin ?? 0;
  const sMax = sliderMax ?? 1;
  const sliderValue = Math.min(sMax, Math.max(sMin, value));

  return (
    <label className={`field${hasSlider ? '' : ' field--inline'}`} title={help}>
      <span className="field__label">
        {label}
        {symbol && <span className="sym">{symbol}</span>}
      </span>
      <span className="field__input">
        <input
          type="text"
          inputMode="decimal"
          value={text}
          disabled={disabled}
          onChange={(e) => setText(e.target.value)}
          onFocus={() => setFocused(true)}
          onBlur={() => {
            setFocused(false);
            commitText();
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
            else if (e.key === 'ArrowUp') {
              e.preventDefault();
              nudge(1, e.shiftKey);
            } else if (e.key === 'ArrowDown') {
              e.preventDefault();
              nudge(-1, e.shiftKey);
            }
          }}
          aria-label={`${label}${unit ? ` (${unit})` : ''}`}
        />
        <span className="field__unit">{unit}</span>
      </span>
      {hasSlider && (
        <input
          className="field__slider"
          type="range"
          min={sMin}
          max={sMax}
          step={(sMax - sMin) / 400}
          value={sliderValue}
          disabled={disabled}
          aria-label={`${label} slider`}
          onPointerDown={() => transactional && useSimulationStore.getState().beginTransaction()}
          onPointerUp={() => transactional && useSimulationStore.getState().commitTransaction()}
          onKeyUp={() => transactional && useSimulationStore.getState().commitTransaction()}
          onChange={(e) => onChange(clamp(Number(e.target.value)), true)}
        />
      )}
    </label>
  );
}
