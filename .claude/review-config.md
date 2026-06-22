# review-config — path rules + learnings for `pr-reviewer`

`pr-reviewer` reads this every run. Two jobs: (1) tell it which extra tools/risks
apply per path, (2) record **learnings** — repo-specific facts that stop it from
re-raising known false positives. Append a learning the moment a finding turns out
to be a false positive.

## Path-specific rules

| Path glob                                                             | Extra checks the reviewer must run                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/tauri/src-tauri/src/commands/**` (esp. new `#[tauri::command]`) | IPC trust boundary: input validated in **Rust** (not the renderer); fs/path commands scope-checked (no traversal); registered in `lib.rs` invoke_handler; new contract method present in `packages/shared` + tauri-client + mock-client.                                                                                    |
| `apps/tauri/src-tauri/capabilities/**`, `tauri.conf.json`             | Capability scope not widened; CSP not loosened; updater/plugin perms unchanged. Flag any broadening.                                                                                                                                                                                                                        |
| `apps/tauri/src/renderer/**`                                          | React 19: effect deps, stale closures (functional `setState`/ref/`useEffectEvent`, never lint-suppress), cleanup/leaks, async-effect races (`AbortController`), keys, referential stability. Design-system: no raw `<button>/<select>/<textarea>`, no `[#hex]`, tokens only. i18n: `@ajh/translations`, en+de keys present. |
| `packages/shared/**` (IPC contracts/schemas)                          | After contract edits, `rtk pnpm gen:ipc:check` must be clean; mock-client must mirror the new method (tsc fails otherwise). No React/Node in `shared`.                                                                                                                                                                      |
| `packages/prompts/**`                                                 | Prompt-injection posture: untrusted inputs fenced; user instruction bounded; honesty/grounding rules intact. No UI/`window`.                                                                                                                                                                                                |
| `**/*.test.*`, `**/*.spec.*`                                          | Tests cover changed **edge/error/security** paths; no tautological assertions; fake-timer + async `act()` pitfalls.                                                                                                                                                                                                         |

## Learnings (repo exceptions — do NOT flag these)

- **`architecture.rs` / `docs/architecture-rules.md` L0–L3 lists** intentionally enumerate every module per layer; repeated module names / parallel headings are the house style, **not** duplication/drift.
- **Custom Tauri commands are NOT listed in `capabilities/default.json`** (e.g. `menu_take_pending`, all `autopilot_*`). Their absence is correct — do not raise "missing permission".
- **`import type` everywhere** is enforced by `@typescript-eslint/consistent-type-imports` (auto-fix). It is required, not a smell.
- **ESLint `noInlineConfig`** is on: never recommend `eslint-disable`/`@ts-ignore`. Refs read inside effects (`ref.current`) are exempt from `exhaustive-deps` **without** suppression — that's correct.
- **`noUncheckedIndexedAccess` is strict**: tests pass under esbuild (`pnpm test`) but `tsc`/CI is stricter. Always trust `rtk pnpm typecheck`, and guard array indices (no `!`).
- **jsdom drops hex-stop `linear-gradient`** in the full `pnpm test` run — assert gradients via `data-*` seams, not computed CSS. Not a renderer bug.
- **`rtk`-wrapped tool _output_ is lossy** (it substitutes tokens) — never trust `rtk rg`/`rtk git` stdout for exact symbol names, IDs, or GraphQL. Exit codes from `rtk pnpm typecheck`/`lint`/`cargo` ARE reliable.
- **Cold-start intent buffers** (`PendingMenu`, `PendingFocus`) are `app.manage`d **early** in setup (before the deep-link handler), deliberately NOT in `tray::build` — that ordering is the fix, not a bug.
- **Dotted-key mock overrides are valid** in renderer tests. `createMockClient` in `apps/tauri/src/renderer/test-support.tsx` is a `Proxy` whose `get` trap resolves dotted paths, so `createMockClient({ 'autopilot.takePendingFocus': fn })` correctly overrides `client.autopilot.takePendingFocus`. Do NOT flag this as a "wrong override shape / shallow-merge miss" (CodeRabbit raised this on #477; it was a false positive). The concrete-object variant at `renderer/lib/mock-client/mock-client.ts` is different — it takes nested objects — so match the override shape to whichever factory the test imports.
