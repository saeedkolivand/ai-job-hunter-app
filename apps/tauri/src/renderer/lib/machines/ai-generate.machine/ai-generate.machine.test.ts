import { describe, expect, it } from 'vitest';

import { isBusy, isError, transition } from '@/lib/machine';

import { aiGenerateMachine } from './ai-generate.machine';

describe('aiGenerateMachine', () => {
  it('walks the happy path', () => {
    let s = transition(aiGenerateMachine, 'idle', 'SUBMIT');
    expect(s).toBe('configuring');
    s = transition(aiGenerateMachine, s, 'EXTRACTION_DONE');
    expect(s).toBe('extracting');
    s = transition(aiGenerateMachine, s, 'STREAM_DONE');
    expect(s).toBe('generating');
    s = transition(aiGenerateMachine, s, 'STREAM_DONE');
    expect(s).toBe('done');
    expect(transition(aiGenerateMachine, s, 'RESET')).toBe('idle');
  });

  it('routes any busy step to error and back to idle', () => {
    expect(transition(aiGenerateMachine, 'extracting', 'ERROR')).toBe('error');
    expect(transition(aiGenerateMachine, 'error', 'RESET')).toBe('idle');
  });

  it('classifies busy and error states', () => {
    expect(isBusy(aiGenerateMachine, 'generating')).toBe(true);
    expect(isBusy(aiGenerateMachine, 'idle')).toBe(false);
    expect(isError(aiGenerateMachine, 'error')).toBe(true);
  });
});
