// Throwaway target to smoke-test the `@claude review` workflow. Safe to delete.
// Contains a deliberate logic bug for the reviewer to catch.
export function daysBetween(start: Date, end: Date): number {
  // BUG: returns the difference in milliseconds, not days.
  return end.getTime() - start.getTime();
}
