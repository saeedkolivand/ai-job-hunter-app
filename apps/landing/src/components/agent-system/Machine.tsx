import type { ReactElement } from 'react';

import type { MachineKind } from '@/data/agent-fleet';

// Per-station inline SVG icon (ported verbatim from the hand-authored page).
const MACHINE_PATHS: Record<MachineKind, ReactElement> = {
  router: <path className="ink ink-a" d="M12 30h28M26 16v28M14 22l-2-6 6 2M38 22l2-6-6 2" />,
  pen: (
    <>
      <path className="ink ink-g" d="M14 44l4-10L40 12l6 6L24 40z" />
      <path className="ink ink-g" d="M36 16l6 6" />
    </>
  ),
  mag: (
    <>
      <circle className="ink ink-p" cx="24" cy="24" r="13" />
      <path className="ink ink-p" d="M34 34l10 10" />
    </>
  ),
  tube: <path className="ink ink-a" d="M22 8h12M25 8v22a7 7 0 0014 0V8M26 26h12" />,
  broom: (
    <path
      className="ink ink-a"
      d="M30 8L18 30M14 42l8-14 12 6-6 14zM12 44l4-6M18 46l4-6M24 48l4-6"
    />
  ),
  quill: (
    <path className="ink ink-a" d="M40 10C24 14 14 28 14 42M14 42c10-2 22-10 26-22M20 36h10" />
  ),
  gate: <path className="ink ink-r" d="M12 40V14h24v26M12 22h24M20 14v26M28 14v26" />,
  rocket: (
    <path
      className="ink ink-a"
      d="M24 8c8 6 10 16 8 26l-8 6-8-6c-2-10 0-20 8-26zM24 24a3 3 0 100-6 3 3 0 000 6M18 34l-4 8 8-3M30 34l4 8-8-3"
    />
  ),
};

export function Machine({ kind }: { kind: MachineKind }) {
  return (
    <svg viewBox="0 0 56 56" aria-hidden="true">
      {MACHINE_PATHS[kind]}
    </svg>
  );
}
