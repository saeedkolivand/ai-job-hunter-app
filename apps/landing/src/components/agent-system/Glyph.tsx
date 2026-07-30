import type { AgentRole } from '@/data/agent-fleet';

// Per-role inline SVG icon (ported verbatim from the hand-authored page).
export function Glyph({ role }: { role: AgentRole }) {
  if (role === 'author') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          className="ink"
          style={{ stroke: 'currentColor' }}
          d="M5 19l2-6L17 3l3 3L10 16l-6 2z"
        />
        <path className="ink" style={{ stroke: 'currentColor' }} d="M14 6l3 3" />
      </svg>
    );
  }
  if (role === 'critic') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle className="ink" style={{ stroke: 'currentColor' }} cx="10" cy="10" r="6" />
        <path className="ink" style={{ stroke: 'currentColor' }} d="M14.5 14.5L20 20" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        className="ink"
        style={{ stroke: 'currentColor' }}
        d="M12 3l8 4v5c0 5-4 8-8 9-4-1-8-4-8-9V7z"
      />
    </svg>
  );
}
