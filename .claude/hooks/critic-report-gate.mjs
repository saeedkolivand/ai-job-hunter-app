#!/usr/bin/env node
// PreToolUse(SubagentHandback) + SubagentStop: a critic's APPROVE without the
// self-red-team section is invalid (critic-contract), so hand it back to the critic
// instead of letting it reach the orchestrator. Exit 2 sends stderr to the subagent.
import { isCriticAgent } from './guard-shell.mjs';

const RED_TEAM = /self-red-team|attacked and held/i;

/** The instruction to send back to the critic, or null to let the report through. */
export function gate(report, agentType, isCritic) {
  if (!agentType || !isCritic || !/\bAPPROVE\b/.test(report) || RED_TEAM.test(report)) return null;
  return (
    'Report not accepted: an APPROVE needs the self-red-team section (critic-contract). ' +
    'List the 3-5 riskiest spots, try to refute each, and report each as a finding or ' +
    '"attacked and held: <what you tried>". Then hand the report back.'
  );
}

if (process.argv[1]?.endsWith('critic-report-gate.mjs')) {
  let raw = '';
  for await (const chunk of process.stdin) raw += chunk;
  const input = JSON.parse(raw || '{}');
  if (input.stop_hook_active) process.exit(0); // already sent back once: don't loop
  const report = input.tool_input?.message ?? input.last_assistant_message ?? '';
  const type = input.agent_type;
  const reason = gate(report, type, Boolean(type) && isCriticAgent(type));
  if (reason) {
    console.error(reason);
    process.exit(2);
  }
}
