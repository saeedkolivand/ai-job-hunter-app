/**
 * Cross-package parity guard (extension PR 11 hardening — AI-prompt MEDIUM
 * fix): `ExtensionRewritePreset` (`@ajh/shared`) must always name EXACTLY
 * the same 5 ids as `packages/translations`' EN
 * `aiGenerate.rewrite.presetInstructions` — the wording source of truth the
 * in-app `RewritePopover` uses via `t(...)`, and the one
 * `extension_bridge::answer_rewrite::preset_instruction` (Rust) ports
 * verbatim (that module has its OWN `include_str!`-based parity test
 * against the same file — see its doc). Without this test, a
 * `translation.json` id change (or a 6th preset) would silently desync from
 * the TS union with nothing failing.
 *
 * `packages/shared` itself cannot host this test (its ESLint boundary bans
 * Node/`fs` imports — IPC contracts + Zod schemas only), so it lives here,
 * in the one package that already imports `ExtensionRewritePreset` for real
 * (`background.ts`/`popup.ts`/`messages.ts`) and already has Node globals
 * enabled for its own test files (see `eslint.config.mjs`).
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import type { ExtensionRewritePreset } from '@ajh/shared';

const here = dirname(fileURLToPath(import.meta.url));
const translationPath = join(
  here,
  '../../../../packages/translations/src/locales/en/translation.json'
);

describe('ExtensionRewritePreset parity with packages/translations', () => {
  it('the id set exactly matches ExtensionRewritePreset (exhaustive — a union drift fails tsc, not just this assertion)', () => {
    // Exhaustive by construction: `Record<ExtensionRewritePreset, true>` is
    // excess/missing-key checked against the union by `tsc` — if the union
    // ever gains or loses an id without this object being updated to match,
    // this file fails to typecheck, independent of the runtime assertion
    // below (which catches the OTHER direction: a translations-only change).
    const idCoverage: Record<ExtensionRewritePreset, true> = {
      shorten: true,
      expand: true,
      rephrase: true,
      impact: true,
      grammar: true,
    };

    const translation = JSON.parse(readFileSync(translationPath, 'utf-8')) as {
      aiGenerate: { rewrite: { presetInstructions: Record<string, string> } };
    };
    const actualIds = Object.keys(translation.aiGenerate.rewrite.presetInstructions).sort();

    expect(actualIds).toEqual(Object.keys(idCoverage).sort());
  });
});
