/**
 * Vitest globalSetup — runs once in the main process before any test worker.
 *
 * Patches process.stderr.write to swallow the two jsdom "Not implemented"
 * noise lines produced by TanStack Router's scroll-restoration path:
 *  - "Not implemented: window.scrollTo"
 *  - "Not implemented: navigation (except hash changes)"
 *
 * These fire from jsdom's virtualConsole → console.error (or directly to
 * stderr when no virtualConsole listener is wired) and are not test failures.
 * Filtering at the stderr level is the only hook that catches them before
 * the worker's setup file runs.
 */
export default function setup() {
  const SUPPRESSED = ['Not implemented: window.scrollTo', 'Not implemented: navigation'];

  const origWrite = process.stderr.write.bind(process.stderr);
  // @ts-expect-error — overriding overloaded write signature
  process.stderr.write = (chunk: string | Buffer, ...rest: unknown[]) => {
    const text = typeof chunk === 'string' ? chunk : chunk.toString();
    if (SUPPRESSED.some((s) => text.includes(s))) return true;
    return (origWrite as typeof process.stderr.write)(
      chunk as string,
      ...(rest as Parameters<typeof process.stderr.write>).slice(1)
    );
  };
}
