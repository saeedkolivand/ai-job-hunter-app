import type { AgentRole } from '@/data/agent-fleet';

import { Glyph } from './Glyph';

// Hoisted to module scope (was defined inside AgentFleet's render body) so it
// keeps one component identity across renders — previously every render
// created a *new* NodeButton function, so React unmounted + remounted every
// node on each re-render, restarting focus and transitions. The one
// deliberately non-behavior-preserving change in this split.
export interface NodeButtonProps {
  name: string;
  roleClass: AgentRole;
  selected: boolean;
  onSelect: (name: string) => void;
  onHover: (name: string | null) => void;
}

export function NodeButton({ name, roleClass, selected, onSelect, onHover }: NodeButtonProps) {
  return (
    <button
      type="button"
      className={`node ${roleClass}${roleClass === 'author' ? ' node-author' : ''}`}
      data-name={name}
      aria-expanded={selected}
      onClick={() => onSelect(name)}
      onMouseEnter={() => onHover(name)}
      onMouseLeave={() => onHover(null)}
      onFocus={() => onHover(name)}
      onBlur={() => onHover(null)}
    >
      <span className="ng">
        <Glyph role={roleClass} />
      </span>
      <span>{name}</span>
    </button>
  );
}
