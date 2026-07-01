import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { aiGenerateMachine } from '@/lib/machines/ai-generate.machine';

import { useMachine } from './use-machine';

describe('useMachine', () => {
  it('starts in the initial state', () => {
    const { result } = renderHook(() => useMachine(aiGenerateMachine, 'idle'));
    expect(result.current[0]).toBe('idle');
    expect(result.current[2]).toEqual({ busy: false, error: false });
  });

  it('transitions on send and reflects busy/error flags', () => {
    const { result } = renderHook(() => useMachine(aiGenerateMachine, 'idle'));

    act(() => result.current[1]('SUBMIT'));
    expect(result.current[0]).toBe('configuring');
    expect(result.current[2].busy).toBe(true);

    act(() => result.current[1]('ERROR'));
    expect(result.current[0]).toBe('error');
    expect(result.current[2].error).toBe(true);
  });

  it('ignores undefined transitions', () => {
    const { result } = renderHook(() => useMachine(aiGenerateMachine, 'idle'));
    act(() => result.current[1]('STREAM_DONE'));
    expect(result.current[0]).toBe('idle');
  });
});
