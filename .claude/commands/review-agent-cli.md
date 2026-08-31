---
description: Agent-facing CLI review with agent-cli-reviewer (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run an **agent CLI** review (verb surface, argv parsing, allowlist projections incl. nested types, the machine-readable output contract, help/schema drift, throttle scope, destructive-command ergonomics).

1. Load the `token-efficiency` + `agent-cli-standards` skills.
2. Scope with graphify/codegraph; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `apps/desktop/src-tauri/src/extension_bridge/agent_cli.rs`, `agent_read.rs`, and any CLI verb table they reach.
4. Spawn **only** the `agent-cli-reviewer` subagent (Task) as Primary Owner. Add `tauri-security-reviewer` as Secondary for any destructive verb, new data exposure, or auth change — **≤3 reviewers**.
5. Report functional and stylistic findings in separate lists; **only functional HIGH/CRITICAL block** (irreversible action from a mis-parsed argument, PII a projection claims to exclude, unfenced untrusted text, a bypassed spend/rate limit, a hang, an output-contract break).
6. If the diff changes runtime behaviour, require an **end-to-end run against a live app** — this surface's worst defects were invisible to reading.
