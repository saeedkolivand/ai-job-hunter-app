interface Props {
  data: number[];
}

export function Sparkline({ data }: Props) {
  const max = Math.max(1, ...data);
  const now = new Date().getHours();

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
              fill={i === now ? 'url(#bar-fill-now)' : 'url(#bar-fill)'}
            />
          );
        })}
      </svg>
      {/* Hour labels */}
      <div className="mt-1 flex justify-between text-[9px] text-foreground/25 px-0.5">
        {['12am', '3am', '6am', '9am', '12pm', '3pm', '6pm', '9pm'].map((l) => (
          <span key={l}>{l}</span>
        ))}
      </div>
    </div>
  );
}
