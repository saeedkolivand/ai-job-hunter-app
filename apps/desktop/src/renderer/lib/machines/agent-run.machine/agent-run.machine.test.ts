import { describe, expect, it } from 'vitest';

import type { AgentStepEvent } from '@ajh/shared';

import { isBusy, isError, transition } from '@/lib/machine';

import { agentRunMachine, type AgentRunState, stepToEvent } from './agent-run.machine';

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

const confirmRequest = (): AgentStepEvent => ({
  jobId: 'job-1',
  step: 3,
  text: '',
  tools: ['save_cover_letter'],
  denied: [],
  kind: 'confirm_request',
  confirm: { callId: '3-save_cover_letter', tool: 'save_cover_letter', args: {} },
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
    expect(stepToEvent(turn(['draft_resume']))).toBe('DRAFT');
    expect(stepToEvent(turn(['suggest_interview_questions']))).toBe('DRAFT');
  });

  it('maps the terminal proposal step regardless of its (empty) tools', () => {
    expect(stepToEvent(proposal())).toBe('PROPOSE');
  });

  it('returns null for a plan-only turn with no recognized tool', () => {
    expect(stepToEvent(turn([]))).toBeNull();
    expect(stepToEvent(turn(['unknown_tool']))).toBeNull();
  });

  it('maps a confirm_request step to CONFIRM_REQUEST', () => {
    expect(stepToEvent(confirmRequest())).toBe('CONFIRM_REQUEST');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// The `improve_resume` flow (Phase 7).
//
// Its tools are a different vocabulary from prep's, so a prep-keyed mapping
// answers `null` for every one of them and the machine sits in `planning` for a
// whole review run. Each test below fails against that mapping.
// ─────────────────────────────────────────────────────────────────────────────

describe('stepToEvent — the improve_resume flow', () => {
  it('maps every review tool to the reviewing state', () => {
    for (const tool of [
      'get_quality_report',
      'validate_resume',
      'search_candidate_evidence',
      'get_trim_suggestions',
      'run_quality_pipeline',
    ]) {
      expect(stepToEvent(turn([tool]), 'improve_resume')).toBe('REVIEW');
      expect(transition(agentRunMachine, 'planning', 'REVIEW')).toBe('reviewing');
    }
    expect(isBusy(agentRunMachine, 'reviewing')).toBe(true);
  });

  // `validate_resume` is called twice in one run (the post-fix re-check). A map
  // that transitioned only on first sight would mis-sequence the second one.
  it('is idempotent on the tool that legitimately appears twice', () => {
    const event = stepToEvent(turn(['validate_resume']), 'improve_resume');
    expect(event).toBe('REVIEW');
    const first = transition(agentRunMachine, 'planning', 'REVIEW');
    expect(first).toBe('reviewing');
    expect(transition(agentRunMachine, first, 'REVIEW')).toBe('reviewing');
  });

  // Flow-awareness, not a shared name list: prep's tools cannot move a review
  // run (and vice versa). Collapse the two maps into one and this fails.
  it('ignores tools the flow does not own, in both directions', () => {
    expect(stepToEvent(turn(['research_company']), 'improve_resume')).toBeNull();
    expect(stepToEvent(turn(['draft_resume']), 'improve_resume')).toBeNull();
    expect(stepToEvent(turn(['get_quality_report']), 'prep_application')).toBeNull();
    expect(stepToEvent(turn(['run_quality_pipeline']), 'prep_application')).toBeNull();
  });

  it('defaults to the prep flow when no kind is given, like the wire does', () => {
    expect(stepToEvent(turn(['research_company']))).toBe('RESEARCH');
    expect(stepToEvent(turn(['get_quality_report']))).toBeNull();
  });

  // The gated write is a step KIND, so it needs no per-flow tool entry — which
  // is what lets one confirm branch serve `save_cover_letter` and `save_resume`.
  it('suspends on the save_resume confirm request without a tool mapping', () => {
    const step: AgentStepEvent = {
      jobId: 'job-1',
      step: 6,
      text: '',
      tools: ['save_resume'],
      denied: [],
      kind: 'confirm_request',
      confirm: { callId: '6-0-save_resume', tool: 'save_resume', args: { resumeText: 'CV' } },
    };
    expect(stepToEvent(step, 'improve_resume')).toBe('CONFIRM_REQUEST');
    // As a plain turn it is NOT a review step — the suspend is the signal.
    expect(stepToEvent(turn(['save_resume']), 'improve_resume')).toBeNull();
  });

  it('runs the whole review lifecycle: report → check → suspend → summary → done', () => {
    /** Feed one review turn through the real mapping, never a hand-picked event. */
    const drive = (state: AgentRunState, tools: string[]): AgentRunState => {
      const event = stepToEvent(turn(tools), 'improve_resume');
      if (!event) throw new Error(`the improve flow does not recognize ${tools.join()}`);
      return transition(agentRunMachine, state, event);
    };
    let s = transition(agentRunMachine, 'idle', 'START');
    expect(s).toBe('planning');
    s = drive(s, ['get_quality_report']);
    s = drive(s, ['validate_resume']);
    s = drive(s, ['search_candidate_evidence']);
    s = drive(s, ['validate_resume']);
    expect(s).toBe('reviewing');
    s = transition(agentRunMachine, s, 'CONFIRM_REQUEST');
    expect(s).toBe('confirming');
    s = transition(agentRunMachine, s, 'APPROVE');
    s = transition(agentRunMachine, s, 'PROPOSE');
    s = transition(agentRunMachine, s, 'COMPLETE');
    expect(s).toBe('done');
  });
});

describe('agentRunMachine — confirm gate (Phase 3)', () => {
  it('suspends into `confirming` from any busy state on CONFIRM_REQUEST, and is itself busy', () => {
    for (const busy of ['planning', 'researching', 'matching', 'drafting', 'proposing'] as const) {
      expect(transition(agentRunMachine, busy, 'CONFIRM_REQUEST')).toBe('confirming');
    }
    expect(isBusy(agentRunMachine, 'confirming')).toBe(true);
  });

  it('resolving with APPROVE or DENY resumes the loop at `planning`', () => {
    expect(transition(agentRunMachine, 'confirming', 'APPROVE')).toBe('planning');
    expect(transition(agentRunMachine, 'confirming', 'DENY')).toBe('planning');
  });

  it('a full run that suspends once still reaches `done`', () => {
    let s = transition(agentRunMachine, 'idle', 'START');
    s = transition(agentRunMachine, s, 'DRAFT');
    s = transition(agentRunMachine, s, 'CONFIRM_REQUEST');
    expect(s).toBe('confirming');
    s = transition(agentRunMachine, s, 'APPROVE');
    expect(s).toBe('planning');
    s = transition(agentRunMachine, s, 'PROPOSE');
    s = transition(agentRunMachine, s, 'COMPLETE');
    expect(s).toBe('done');
  });

  it('Stop (CANCEL) still works while suspended', () => {
    expect(transition(agentRunMachine, 'confirming', 'CANCEL')).toBe('cancelled');
  });
});
