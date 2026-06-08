#!/usr/bin/env node
// SessionStart hook — deterministically (re)activates the project's
// auto-invoked output policy at the start of every session, independent of
// whether CLAUDE.md is read in full or later summarized.
//
// It fires only at session start, so the mid-session off-switch still works:
// the user can say "stop caveman" / "normal mode" and it stays off until the
// next session. (A per-turn UserPromptSubmit hook would fight that off-switch,
// so SessionStart is the deliberate choice.)
//
// SessionStart stdout is injected into the session as additional context via
// the documented hookSpecificOutput.additionalContext field.

const policy = [
  '[style policy — active for this session]',
  '• caveman: respond ultra-terse — drop articles, filler, and pleasantries; keep ALL technical substance, code blocks, and exact error text. Auto-clarity exception: use normal prose for security warnings, irreversible-action confirmations, and multi-step sequences, then resume caveman. Off-switch: the user says "stop caveman" / "normal mode".',
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
