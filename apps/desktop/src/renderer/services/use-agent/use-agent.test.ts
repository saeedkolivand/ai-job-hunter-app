import { describe, expect, it, vi } from 'vitest';

import type { AgentStepEvent } from '@ajh/shared';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-agent';
import { useAgentStepEvents } from './use-agent';

describe('use-agent services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// The subscription must attach ONCE per mount.
//
// `listen` (Tauri) attaches asynchronously, so an effect keyed on the caller's
// handler unsubscribes and re-subscribes with a real gap in between — and every
// caller's handler changes identity exactly when its run id arrives, i.e. the
// moment the run starts emitting. A `confirm_request` dropped in that gap
// leaves a SUSPENDED run rendered as a running one until the gate times out.
// ─────────────────────────────────────────────────────────────────────────────

describe('useAgentStepEvents — subscription stability', () => {
  function setup() {
    let deliver: ((event: unknown) => void) | undefined;
    const unsubscribe = vi.fn();
    const onStep = vi.fn((handler: (event: unknown) => void) => {
      deliver = handler;
      return unsubscribe;
    });
    const client = createMockClient({ 'agent.onStep': onStep });
    return { client, onStep, unsubscribe, deliver: (e: unknown) => deliver?.(e) };
  }

  const step = (jobId: string): AgentStepEvent => ({
    jobId,
    step: 1,
    text: '',
    tools: [],
    denied: [],
    kind: 'turn',
  });

  it('subscribes once even as the caller swaps handlers, and never detaches mid-run', () => {
    const { client, onStep, unsubscribe } = setup();
    const { rerender } = renderHookWithClient(
      ({ handler }: { handler: (event: AgentStepEvent) => void }) => {
        useAgentStepEvents(handler);
      },
      { client, initialProps: { handler: vi.fn() } }
    );

    expect(onStep).toHaveBeenCalledTimes(1);
    // What a real caller does when `agent.run` resolves: a new callback, keyed
    // on the run id it has just learned.
    rerender({ handler: vi.fn() });
    rerender({ handler: vi.fn() });
    expect(onStep).toHaveBeenCalledTimes(1);
    expect(unsubscribe).not.toHaveBeenCalled();
  });

  it('routes events to the LATEST handler, so subscribing once loses nothing', () => {
    const { client, deliver } = setup();
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHookWithClient(
      ({ handler }: { handler: (event: AgentStepEvent) => void }) => {
        useAgentStepEvents(handler);
      },
      { client, initialProps: { handler: first } }
    );

    rerender({ handler: second });
    deliver(step('job-1'));

    expect(second).toHaveBeenCalledWith(step('job-1'));
    expect(first).not.toHaveBeenCalled();
  });

  it('detaches on unmount', () => {
    const { client, unsubscribe } = setup();
    const { unmount } = renderHookWithClient(
      () => {
        useAgentStepEvents(vi.fn());
      },
      { client }
    );
    unmount();
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });
});
