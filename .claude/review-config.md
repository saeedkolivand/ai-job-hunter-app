# review-config — path rules + learnings for every review surface

`pr-reviewer`, the Stop review-gate, `/review`, and `finding-verifier` all read this.
Two jobs: (1) tell reviewers which extra tools/risks apply per path, (2) record
**learnings** — repo-specific facts that stop known false positives from being
re-raised. Append the moment a finding turns out to be a false positive, into the
right section: **Hard exclusions** (never raise), **Signal-quality criteria** (how to
verify before raising), **Precedents** (severity/design calls already made).

## Path-specific rules

| Path glob                                                               | Extra checks the reviewer must run                                                                                                                                                                                                                                                                                          |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/desktop/src-tauri/src/commands/**` (esp. new `#[tauri::command]`) | IPC trust boundary: input validated in **Rust** (not the renderer); fs/path commands scope-checked (no traversal); registered in `lib.rs` invoke_handler; new contract method present in `packages/shared` + tauri-client + mock-client.                                                                                    |
| `apps/desktop/src-tauri/capabilities/**`, `tauri.conf.json`             | Capability scope not widened; CSP not loosened; updater/plugin perms unchanged. Flag any broadening.                                                                                                                                                                                                                        |
| `apps/desktop/src/renderer/**`                                          | React 19: effect deps, stale closures (functional `setState`/ref/`useEffectEvent`, never lint-suppress), cleanup/leaks, async-effect races (`AbortController`), keys, referential stability. Design-system: no raw `<button>/<select>/<textarea>`, no `[#hex]`, tokens only. i18n: `@ajh/translations`, en+de keys present. |
| `packages/shared/**` (IPC contracts/schemas)                            | After contract edits, `rtk pnpm gen:ipc:check` must be clean; mock-client must mirror the new method (tsc fails otherwise). No React/Node in `shared`.                                                                                                                                                                      |
| `packages/prompts/**`                                                   | Prompt-injection posture: untrusted inputs fenced; user instruction bounded; honesty/grounding rules intact. No UI/`window`.                                                                                                                                                                                                |
| `**/*.test.*`, `**/*.spec.*`                                            | Tests cover changed **edge/error/security** paths; no tautological assertions; fake-timer + async `act()` pitfalls.                                                                                                                                                                                                         |

## Hard exclusions (never re-raise — confirmed repo false positives)

- **`architecture.rs` / `docs/architecture-rules.md` L0–L3 lists** intentionally enumerate every module per layer; repeated module names / parallel headings are the house style, **not** duplication/drift.
- **Custom Tauri commands are NOT listed in `capabilities/default.json`** (e.g. `menu_take_pending`, all `autopilot_*`). Their absence is correct — do not raise "missing permission".
- **`import type` everywhere** is enforced by `@typescript-eslint/consistent-type-imports` (auto-fix). It is required, not a smell.
- **ESLint `noInlineConfig`** is on: never recommend `eslint-disable`/`@ts-ignore`. Refs read inside effects (`ref.current`) are exempt from `exhaustive-deps` **without** suppression — that's correct.
- **Dotted-key mock overrides are valid** in renderer tests. `createMockClient` in `apps/desktop/src/renderer/test-support.tsx` is a `Proxy` whose `get` trap resolves dotted paths, so `createMockClient({ 'autopilot.takePendingFocus': fn })` correctly overrides `client.autopilot.takePendingFocus`. Do NOT flag this as a "wrong override shape / shallow-merge miss" (CodeRabbit raised this on #477; it was a false positive). The concrete-object variant at `renderer/lib/mock-client/mock-client.ts` is different — it takes nested objects — so match the override shape to whichever factory the test imports.
- **`unwrap()`/`expect()` on a KNOWN-GOOD LITERAL inside a `LazyLock`/`OnceLock` static initializer** (e.g. `Regex::new(r"…").unwrap()` with a hardcoded pattern) is accepted house style (~70 instances) — flag static-init unwraps only when the input is fallible/externally influenced.
- **`json!(local)` over a contract-shaped local in `commands/**`** is pervasive house style — flag `json!` only when it wholesale-serializes a domain/storage STRUCT across the IPC boundary.
- **`void openExternal.mutateAsync(url)` with no catch** is established house style (10+ identical call sites: AggregatorKeysSettings, ResearchStep, OllamaNotInstalled, ExtensionStep, …) — do not flag unhandled-rejection on it. (False positive by a /review opus pass on PR A of task #23; verifier scored 20.)

## Signal-quality criteria (what makes a finding real here)

- **`noUncheckedIndexedAccess` is strict**: tests pass under esbuild (`pnpm test`) but `tsc`/CI is stricter. Always trust `rtk pnpm typecheck`, and guard array indices (no `!`).
- **jsdom drops hex-stop `linear-gradient`** in the full `pnpm test` run — assert gradients via `data-*` seams, not computed CSS. Not a renderer bug.
- **`rtk`-wrapped tool _output_ is lossy** (it substitutes tokens) — never trust `rtk rg`/`rtk git` stdout for exact symbol names, IDs, or GraphQL. Exit codes from `rtk pnpm typecheck`/`lint`/`cargo` ARE reliable.
- **New (untracked) files are invisible to plain `git diff`** — before raising "imported file missing from diff", run `git status` for `??` entries (the orchestrator should `git add -N` new files so review diffs include them; a Stop-gate review raised this falsely on PR C while typecheck + full vitest were green).
- **Helper fns defined near the top of long components** (e.g. `stopProp` at `AutopilotCard/index.tsx:272`) sit outside a diff hunk's context lines — do NOT flag "undefined identifier / ReferenceError" from diff context alone; grep the whole file first (a Stop-gate review raised this falsely on PR C while typecheck + full vitest were green).

## Precedents (severity + design calls already made)

- **Cold-start intent buffers** (`PendingMenu`, `PendingFocus`) are `app.manage`d **early** in setup (before the deep-link handler), deliberately NOT in `tray::build` — that ordering is the fix, not a bug.
- **ADRs may restate code-owned literals** (slot names, IPs, ports) alongside a pointer to the owning symbol — the "thin pointers, no copied literals" rule governs `docs/knowledge/` only, and ADR-0012 set the restate-with-pointer style. Do not flag literal restatement in `docs/adr/`. (False positive by a /review sonnet pass on PR A of task #23; verifier scored 0.)
- **A field appearing in the full-row `JobPreferences::set()` is not by itself the PR-#695 foot-gun** — check whether the field ALSO ships a dedicated single-column setter and whether the Settings component uses only that setter (the defended pattern). `extraAgencyCompanies` was flagged this way on the ADR-029 branch while its dedicated setter + component docstring guard existed; verifier scored 5. (False positive by a /review sonnet pass.)
