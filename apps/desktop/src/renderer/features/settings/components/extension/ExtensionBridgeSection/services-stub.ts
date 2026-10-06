import type * as ServicesModule from '@/services';

// Kept apart from ./test-support: the `vi.mock` factory loads this lazily while the
// subject (imported by test-support) is itself loading — sharing a module would deadlock.
// The live active provider the renderer would use for `ai_generate` today —
// controlled per-test (mirrors StepAction.test.tsx's pattern) so the ai-assist
// toggle's provider-configured gate is testable without a real Zustand
// `preferences-store`. Partial mock (see `servicesMock`) — every OTHER `@/services`
// hook stays real (backed by `createMockClient`; `useActiveConfig` reads
// `ai.activeConfig` off the mock client, driven per-test via `renderSection`'s
// `activeConfig` arg). Reset in `installBridgeHooks` — never a trailing
// manual-restore line, which leaks into later tests the moment an assertion throws first.
export const generateConfig = { current: { provider: 'openai', model: 'gpt-4o' } };

/** `vi.mock('@/services')` factory body: the real module with `useGenerateConfig` stubbed. */
export async function servicesMock(
  importOriginal: () => Promise<typeof ServicesModule>
): Promise<Record<string, unknown>> {
  const actual = await importOriginal();
  return { ...actual, useGenerateConfig: () => generateConfig.current };
}
