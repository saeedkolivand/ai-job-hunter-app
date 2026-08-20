import { describe, expect, it } from 'vitest';

import { EVENT_SOURCE_EMAIL, EVENT_SOURCE_EMAIL_REJECT, EVENT_SOURCE_USER } from './index.js';

// Pins the exported `EVENT_SOURCE_*` values against hand-written literals — the
// Rust mirror (`applications/status_events.rs`, `pub(crate)` so it can't share
// this export) pins its own constants against these SAME literals. A rename on
// either side fails a test instead of silently disabling the timeline's
// Accept/Reject adjudication (a provisional row would render as settled
// history with no test failure to catch it). Compared against string literals,
// not against each other — two renamed-in-lockstep constants would still pass
// a same-value comparison while breaking the Rust side.
describe('EVENT_SOURCE_* — pinned against the Rust mirror', () => {
  it('EVENT_SOURCE_USER is "user"', () => {
    expect(EVENT_SOURCE_USER).toBe('user');
  });

  it('EVENT_SOURCE_EMAIL is "email"', () => {
    expect(EVENT_SOURCE_EMAIL).toBe('email');
  });

  it('EVENT_SOURCE_EMAIL_REJECT is "email_reject"', () => {
    expect(EVENT_SOURCE_EMAIL_REJECT).toBe('email_reject');
  });
});
