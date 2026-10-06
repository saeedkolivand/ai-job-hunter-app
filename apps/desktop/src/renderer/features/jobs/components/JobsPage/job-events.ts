import { act } from '@testing-library/react';

/** Set by the suite's `useJobEvents` mock when the page subscribes. */
export const jobEvents = { handler: null as ((event: unknown) => void) | null };

export function fireJobEvent(event: unknown) {
  act(() => {
    jobEvents.handler?.(event);
  });
}
