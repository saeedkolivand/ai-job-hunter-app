#!/usr/bin/env node
// SessionStart hook — deterministically (re)activates the project's
// auto-invoked skill policy at the start of every session, independent of
// whether CLAUDE.md is read in full or later summarized.
//
// It fires only at session start (not per-turn), so any mid-session opt-out a
// skill defines keeps working until the next session.
//
// SessionStart stdout is injected into the session as additional context via
// the documented hookSpecificOutput.additionalContext field.

const policy = [
  '[skill policy — active for this session]',
  '• ponytail: lazy-senior-dev mode — active every response. Reach for the simplest, shortest solution that actually works: question whether the task needs to exist (YAGNI), prefer the standard library / native platform features over dependencies, one line over fifty. Default intensity full; switch with /ponytail lite|full|ultra. Off-switch: the user says "stop ponytail" / "normal mode".',
  '• grill-with-docs: before presenting any non-trivial plan or design (including before ExitPlanMode), first run the grill-with-docs skill to stress-test it against the repo domain model + ADRs. Skip for trivial / one-line / docs changes.',
].join('\n');

process.stdout.write(
  JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'SessionStart',
      additionalContext: policy,
    },
  })
);
