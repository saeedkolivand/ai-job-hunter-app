// Decorative hand-drawn squiggle between page sections, rendered 6x.
export function Divider({ d }: { d: string }) {
  return (
    <div className="divider draw" aria-hidden="true">
      <svg viewBox="0 0 620 16" preserveAspectRatio="none">
        <path className="ink" d={d} />
      </svg>
    </div>
  );
}
