/**
 * IPC codegen — Zod schema → Rust request struct.
 *
 * The renderer, the IPC contract, the Rust command, and the TS client are
 * otherwise hand-synced (4 files per capability). This makes the Zod schemas in
 * `src/schemas` the single source of truth for request shapes and emits the
 * matching Rust `Deserialize` structs, so the two can't drift.
 *
 * Run `pnpm gen:ipc` to regenerate, or `pnpm gen:ipc --check` to fail when the
 * committed output is stale (used in CI).
 *
 * The emitters live in `./gen-ipc-rust/`: `structs` (schema → struct modules),
 * `events` (event-channel + pipeline vocabulary), `constants` (the single-vocabulary
 * modules), `modules` (which schema lands in which file), `rust-literals` (shared
 * rustfmt-stable literal emitters). This file only wires them to output paths.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  genAiIntents,
  genBoardIds,
  genContextWindowBounds,
  genDateFilters,
  genGenerationDepths,
  genMatchTiers,
  genSlots,
  genStreamTimeouts,
} from './gen-ipc-rust/constants.js';
import { genEvents } from './gen-ipc-rust/events.js';
import { MODULES } from './gen-ipc-rust/modules.js';
import { genModule } from './gen-ipc-rust/structs.js';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const CONTRACTS_DIR = 'apps/desktop/src-tauri/src/ipc_contracts';

const check = process.argv.includes('--check');
let stale = false;

// Unified output list: the Zod-derived struct modules plus the constants modules
// (different sources of truth: src/events/, src/provider-slots.ts, …), written/checked
// by the same logic so `pnpm gen:ipc[:check]` covers all of them.
const outputs: { outFile: string; content: string }[] = [
  ...MODULES.map((mod) => ({ outFile: mod.outFile, content: genModule(mod) })),
  ...(
    [
      ['events', genEvents],
      ['provider_slots', genSlots],
      ['board_ids', genBoardIds],
      ['date_filters', genDateFilters],
      ['ai_intents', genAiIntents],
      ['ai_timeouts', genStreamTimeouts],
      ['context_window', genContextWindowBounds],
      ['generation_depths', genGenerationDepths],
      ['match_tiers', genMatchTiers],
    ] as const
  ).map(([name, gen]) => ({ outFile: `${CONTRACTS_DIR}/${name}.rs`, content: gen() })),
];

for (const { outFile, content: next } of outputs) {
  const target = join(REPO_ROOT, outFile);
  if (check) {
    let current: string;
    try {
      current = readFileSync(target, 'utf8');
    } catch {
      current = '';
    }
    if (current !== next) {
      stale = true;
      console.error(`✗ stale: ${outFile} — run \`pnpm gen:ipc\``);
    }
  } else {
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, next);
    console.log(`✓ wrote ${outFile}`);
  }
}

if (check && stale) process.exit(1);
if (check) console.log('✓ IPC codegen output is up to date');
