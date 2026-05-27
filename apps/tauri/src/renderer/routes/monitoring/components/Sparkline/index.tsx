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
            <stop offset="0%" stopColor="rgba(168,85,247,0.6)" />
            <stop offset="100%" stopColor="rgba(168,85,247,0.1)" />
          </linearGradient>
          <linearGradient id="bar-fill-now" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="rgba(99,102,241,0.9)" />
            <stop offset="100%" stopColor="rgba(99,102,241,0.2)" />
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
