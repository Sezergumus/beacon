import React, { useState } from "react";

interface LiveChartProps {
  values: number[];
  times: string[];
  /** Top of the y-axis. */
  max: number;
  /** Formats a value for the axis label and tooltip. */
  format: (v: number) => string;
  /** Tailwind text color class; the line and fill use currentColor. */
  colorClass: string;
  label: string;
}

/** A thin single-series area chart with a crosshair tooltip. */
export const LiveChart: React.FC<LiveChartProps> = ({
  values,
  times,
  max,
  format,
  colorClass,
  label,
}) => {
  const [hover, setHover] = useState<number | null>(null);
  const n = values.length;
  const safeMax = max > 0 ? max : 1;

  const x = (i: number) => (n < 2 ? 100 : (i / (n - 1)) * 100);
  const y = (v: number) => 100 - Math.max(0, Math.min(1, v / safeMax)) * 100;

  const line = values.map((v, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(2)},${y(v).toFixed(2)}`).join(" ");
  const area = n >= 2 ? `${line} L100,100 L0,100 Z` : "";

  const onMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (n === 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const frac = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    setHover(Math.round(frac * (n - 1)));
  };

  const hv = hover !== null && hover < n ? hover : null;

  return (
    <div className="pt-5">
      <div
        className={`relative h-36 ${colorClass}`}
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
        role="img"
        aria-label={`${label} over the last few minutes`}
      >
        <svg
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          className="absolute inset-0 w-full h-full overflow-visible"
        >
          {[0, 50, 100].map((gy) => (
            <line
              key={gy}
              x1="0"
              x2="100"
              y1={gy}
              y2={gy}
              stroke="rgb(var(--line))"
              strokeWidth="1"
              strokeOpacity="0.6"
              vectorEffect="non-scaling-stroke"
            />
          ))}
          {n >= 2 && (
            <>
              <path d={area} fill="currentColor" fillOpacity="0.14" />
              <path
                d={line}
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinejoin="round"
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
              />
            </>
          )}
        </svg>

        <span className="absolute left-0 -top-5 text-xs text-faint tabular-nums pointer-events-none">
          {format(safeMax)}
        </span>

        {n < 2 && (
          <div className="absolute inset-0 flex items-center justify-center text-sm text-faint">
            Waiting for numbers…
          </div>
        )}

        {hv !== null && n >= 2 && (
          <>
            <div
              className="absolute top-0 bottom-0 w-px bg-faint/60 pointer-events-none"
              style={{ left: `${x(hv)}%` }}
            />
            <div
              className="absolute w-3 h-3 -ml-1.5 -mt-1.5 rounded-full bg-current ring-2 ring-surface pointer-events-none"
              style={{ left: `${x(hv)}%`, top: `${y(values[hv])}%` }}
            />
            <div
              className="absolute -top-1 z-10 -translate-y-full rounded-xl bg-raised px-3 py-1.5 text-xs shadow-lg shadow-black/40 pointer-events-none whitespace-nowrap"
              style={{
                left: `${x(hv)}%`,
                transform: `translate(${x(hv) > 70 ? "-100%" : x(hv) < 30 ? "0" : "-50%"}, -100%)`,
              }}
            >
              <span className="font-bold text-ink">{format(values[hv])}</span>
              <span className="text-soft"> · {times[hv]}</span>
            </div>
          </>
        )}
      </div>
    </div>
  );
};
