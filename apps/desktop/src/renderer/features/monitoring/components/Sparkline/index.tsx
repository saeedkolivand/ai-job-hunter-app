interface Props {
  data: number[];
}

export function Sparkline({ data }: Props) {
  const max = Math.max(1, ...data);
  // The data is hours-ago indexed (bin 23 = the most recent hour), so the "now"
  // bar is the last bucket — not `new Date().getHours()`, which mislabeled every tick.
  const nowIndex = data.length - 1;

  return (
    <div className="relative">
      <svg viewBox="0 0 480 64" preserveAspectRatio="none" className="h-20 w-full">
        <defs>
          <linearGradient id="bar-fill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="color-mix(in srgb, var(--color-brand) 60%, transparent)" />
            <stop
              offset="100%"
              stopColor="color-mix(in srgb, var(--color-brand) 10%, transparent)"
            />
          </linearGradient>
          <linearGradient id="bar-fill-now" x1="0" x2="0" y1="0" y2="1">
            <stop
              offset="0%"
              stopColor="color-mix(in srgb, var(--color-brand-2) 90%, transparent)"
            />
            <stop
              offset="100%"
              stopColor="color-mix(in srgb, var(--color-brand-2) 20%, transparent)"
            />
          </linearGradient>
        </defs>
        {data.map((v, i) => {
          const slotW = 480 / data.length;
          const x = i * slotW + slotW * 0.2;
          const w = slotW * 0.6;
          const h = Math.max(2, (v / max) * 56);
          return (
            <rect
              key={i}
              x={x}
              y={64 - h}
              width={w}
              height={h}
              rx={2}
              fill={i === nowIndex ? 'url(#bar-fill-now)' : 'url(#bar-fill)'}
            />
          );
        })}
      </svg>
      {/* Relative-time axis: leftmost bar ≈24h ago, rightmost is the current hour.
          Compact notation kept language-neutral, matching the prior hardcoded labels. */}
      <div className="mt-1 flex justify-between text-[9px] text-foreground/25 px-0.5">
        {['-24h', '-18h', '-12h', '-6h', 'now'].map((l) => (
          <span key={l}>{l}</span>
        ))}
      </div>
    </div>
  );
}
