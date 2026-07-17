---
description: Run the current apps/landing phase gate via gate-auditor against the dev server
argument-hint: [phase - defaults to the phase the current diff targets]
---

Run the current **apps/landing WebGL phase gate** on rendered output.

1. Load the `webgl-gate-audit` skill; read the target phase in `docs/adr/0014-landing-gl-takeover.md`.
2. Ensure the dev server is up: `pnpm --filter @ajh/landing dev` (http://localhost:3000).
3. Spawn **only** the `gate-auditor` subagent (Task). It drives the browser via Chrome DevTools MCP:
   scrolls to exact t positions, screenshots, records traces, reads the console.
4. It checks the per-phase gates, scrub determinism, the fried-ramp flash budget (<=3 flashes/second),
   console cleanliness (zero THREE/WebGL errors), and the reduced-motion / capability-gate fallback.
5. It returns a **pass/fail table only** (one evidence note per row); raw screenshots never leave its
   context. Any fix routes to the owning author (`webgl-author` / `shader-engineer`).
