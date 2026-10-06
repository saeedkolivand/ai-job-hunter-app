/**
 * Mock factories + shared state for the `useHelpChat` tests. `vi.mock` is hoisted
 * per test file, so each file wires these in with
 * `vi.mock('<pkg>', async (importOriginal) => (await import('./test-mocks')).<factory>(...))`.
 * Kept free of any import of the hook so a factory never awaits a module that
 * itself imports the mocked package.
 */
import { vi } from 'vitest';

import type * as PromptsGenerate from '@ajh/prompts/generate';

/**
 * What the hook HANDED `buildHelpDataGlance`, recorded without changing what it
 * builds. The rendered glance cannot answer "how much did the hook disclose":
 * the prompt renders at most 10 autopilots itself, so a hook that passed 500
 * would produce a byte-identical string. This is the boundary where the
 * disclosure actually happens, so this is where it is measured.
 */
export const glanceRecorder = { inputs: [] as unknown[] };

/**
 * Opt-in for ONE test: hand the hook the real, mutable i18n instance.
 *
 * react-i18next v17 does not return the instance from `useTranslation()` — it
 * returns a per-render COPY of it, so `i18n.language` inside a closure is
 * frozen at the render that made the closure and a mid-flight switch is
 * invisible. That freeze is a dependency's implementation detail (v16 returned
 * the live instance, and the app's own instance is live), which is exactly what
 * a test of OUR invariant must not lean on. Off for every other test here.
 */
export const live = { i18n: false };

/** The generation half is the seam: stub it so the test asserts what the hook
 *  FEEDS the model (entries, glance, history) rather than re-testing streaming. */
export const generateMock = (): Record<string, unknown> => ({
  generateHelpAnswer: vi.fn().mockResolvedValue('Open the document and click Export.'),
});

export const promptsMock = (actual: typeof PromptsGenerate): Record<string, unknown> => ({
  ...actual,
  buildHelpDataGlance: (input: Parameters<typeof actual.buildHelpDataGlance>[0]) => {
    glanceRecorder.inputs.push(input);
    return actual.buildHelpDataGlance(input);
  },
});

/** Just the two members this file re-wraps — the mock factory's return is not
 *  checked against the real module, and callers keep the real module's types. */
interface TranslationsModule {
  default: unknown;
  useTranslation: (...args: unknown[]) => { t: unknown; i18n: unknown; ready: boolean };
}

export const translationsMock = (actual: TranslationsModule): Record<string, unknown> => ({
  ...actual,
  useTranslation: (...args: unknown[]) => {
    const result = actual.useTranslation(...args);
    if (!live.i18n) return result;
    // The same array-plus-properties shape react-i18next returns, with the
    // frozen copy swapped for the instance the test can actually mutate.
    return Object.assign([result.t, actual.default, result.ready], {
      t: result.t,
      i18n: actual.default,
      ready: result.ready,
    });
  },
});
