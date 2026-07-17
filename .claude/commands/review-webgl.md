---
description: apps/landing WebGL review with webgl-reviewer (+ gate-auditor on rendered-output changes)
argument-hint: [files or PR# - defaults to current git diff]
---

Run a **WebGL landing** review -- scrub-safety, resource disposal, per-frame allocation, post
pipeline correctness, budget, semantic-layer parity, and gate integrity.

1. Load the `token-efficiency` + `webgl-standards` skills; read `docs/adr/0014-landing-gl-takeover.md`.
2. Scope with graphify/codegraph; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `apps/landing/src/**`.
4. Spawn the `webgl-reviewer` subagent (Task) over the diff (both authors' work -- scenes/engine and
   GLSL/post). If the change alters rendered output, add `gate-auditor` as Secondary against the dev
   server -- **<=3 reviewers**.
5. Report severity-tagged findings (scrub-safety, disposal, allocation, uniform-vs-recompile,
   budget, semantic parity, gate, ASCII); **HIGH/CRITICAL block**.
