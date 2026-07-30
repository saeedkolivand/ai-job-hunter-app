// Single metric card — the repeated `mc-card` / `mc-stat__*` shape used across
// every /mission-control section (15 call sites).
export function Stat({
  num,
  unit,
  label,
  sub,
}: {
  num: string;
  unit?: string;
  label: string;
  sub?: string;
}) {
  return (
    <div className="mc-card">
      <p>
        <span className="mc-stat__num">{num}</span>
        {unit ? <span className="mc-stat__unit">{unit}</span> : null}
      </p>
      <p className="mc-stat__label">{label}</p>
      {sub ? <p className="mc-stat__sub">{sub}</p> : null}
    </div>
  );
}
