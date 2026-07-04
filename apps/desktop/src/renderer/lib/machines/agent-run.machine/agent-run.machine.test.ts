import { describe, expect, it } from 'vitest';

import type { AgentStepEvent } from '@ajh/shared';

import { isBusy, isError, transition } from '@/lib/machine';

import { agentRunMachine, stepToEvent } from './agent-run.machine';

const turn = (tools: string[], text = ''): AgentStepEvent => ({
  jobId: 'job-1',
  step: 1,
  text,
  tools,
  denied: [],
  kind: 'turn',
});

const proposal = (text = 'final'): AgentStepEvent => ({
  jobId: 'job-1',
  step: 5,
  text,
  tools: [],
  denied: [],
  kind: 'proposal',
});

describe('agentRunMachine', () => {
  it('progresses through the prep lifecycle', () => {
    let s = transition(agentRunMachine, 'idle', 'START');
    expect(s).toBe('planning');
    s = transition(agentRunMachine, s, 'RESEARCH');
    expect(s).toBe('researching');
    s = transition(agentRunMachine, s, 'MATCH');
    expect(s).toBe('matching');
    s = transition(agentRunMachine, s, 'DRAFT');
    expect(s).toBe('drafting');
    s = transition(agentRunMachine, s, 'PROPOSE');
    expect(s).toBe('proposing');
    s = transition(agentRunMachine, s, 'COMPLETE');
    expect(s).toBe('done');
  });

  it('supports error from any busy state, and busy/error introspection', () => {
    expect(transition(agentRunMachine, 'matching', 'ERROR')).toBe('error');
    expect(isBusy(agentRunMachine, 'drafting')).toBe(true);
    expect(isBusy(agentRunMachine, 'done')).toBe(false);
    expect(isError(agentRunMachine, 'error')).toBe(true);
  });

  it('routes a deliberate cancel to its own cancelled state, distinct from error', () => {
    const cancelled = transition(agentRunMachine, 'drafting', 'CANCEL');
    expect(cancelled).toBe('cancelled');
    expect(isBusy(agentRunMachine, 'cancelled')).toBe(false);
    expect(isError(agentRunMachine, 'cancelled')).toBe(false);
  });

  it('resets done/cancelled/error back to idle', () => {
    expect(transition(agentRunMachine, 'done', 'RESET')).toBe('idle');
    expect(transition(agentRunMachine, 'cancelled', 'RESET')).toBe('idle');
    expect(transition(agentRunMachine, 'error', 'RESET')).toBe('idle');
  });

  it('retry: START from every terminal state restarts the run (no stuck-at-terminal desync)', () => {
    expect(transition(agentRunMachine, 'done', 'START')).toBe('planning');
    expect(transition(agentRunMachine, 'cancelled', 'START')).toBe('planning');
    expect(transition(agentRunMachine, 'error', 'START')).toBe('planning');
  });

  it('retry-from-error-then-succeed: a fresh run after an error reaches done again', () => {
    let s = transition(agentRunMachine, 'error', 'START');
    expect(s).toBe('planning');
    s = transition(agentRunMachine, s, 'RESEARCH');
    s = transition(agentRunMachine, s, 'MATCH');
    s = transition(agentRunMachine, s, 'DRAFT');
    s = transition(agentRunMachine, s, 'PROPOSE');
    s = transition(agentRunMachine, s, 'COMPLETE');
    expect(s).toBe('done');
  });

  it('maps a turn step to its tool-keyed event', () => {
    expect(stepToEvent(turn(['research_company']))).toBe('RESEARCH');
    expect(stepToEvent(turn(['match_resume']))).toBe('MATCH');
    expect(stepToEvent(turn(['draft_cover_letter']))).toBe('DRAFT');
    expect(stepToEvent(turn(['suggest_interview_questions']))).toBe('DRAFT');
  });

  it('maps the terminal proposal step regardless of its (empty) tools', () => {
    expect(stepToEvent(proposal())).toBe('PROPOSE');
  });

  it('returns null for a plan-only turn with no recognized tool', () => {
    expect(stepToEvent(turn([]))).toBeNull();
    expect(stepToEvent(turn(['unknown_tool']))).toBeNull();
  });
});
