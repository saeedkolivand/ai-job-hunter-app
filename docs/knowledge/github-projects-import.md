# GitHub Projects Import (resume builder)

Added: v0.117.0 (PR #495)

Describes the "Import from GitHub" action in the resume builder's Projects step. Fetches the user's public repositories, AI-generates resume bullets per repo, and appends project entries to the builder's `projects` field array.

## Rust: Fetch + IPC command

- **Module:** `apps/desktop/src-tauri/src/profile_import/github.rs` — `pub async fn fetch_repos(input: &str) -> AppResult<Vec<GitHubRepo>>`. Pairs with sibling `profile_import/linkedin.rs` (same co-location pattern; GitHub returns repos, not a profile, so NOT added to `ProfileData` / `detect_platform`).
- **HTTP hardening:** Routes through `scraping::http::fetch_text` (not bare `shared().get()`) for timeouts + 8MB cap + per-host rate limiter. Added opt-in `FetchOptions.timeout: Option<Duration>` field (default `None` for backward compat); set to 20s for the GitHub call. See `scraping/http/mod.rs`.
- **SSRF guard:** Username validation `^[A-Za-z0-9-]{1,39}$`; if input is a URL, host must be github.com (case-insensitive). API URL constructed server-side (never forwarded from client). Status-code dispatch: 404 → `Validation` ("not found"), 403/429 → `RateLimited`, other non-2xx → `Network`. All routes preserved (no URL/status leakage in errors).
- **Contract:** `packages/shared/src/ipc/contracts/github.ts` (`GitHubContract.importRepos(input): Promise<GitHubRepo[]>`; `GitHubRepo` type with optional fields using `#[serde(skip_serializing_if = "Option::is_none")]`).
- **Command:** `apps/desktop/src-tauri/src/commands/github.rs` — invokes `fetch_repos`, returns `{ repos }` or `{ error }`.
- **Registration:** `commands/mod.rs` + `lib.rs` handler list.

See `docs/API.md` → `github` namespace for the IPC contract and calling conventions.

## Renderer: AI bullets

- **Prompt generator:** `packages/prompts/src/generate/github-projects/` — mirrors `interview-questions/` pattern (lenient delimited parse, provider-aware). Exports `buildGitHubProjectsSystemPrompt()`, `buildGitHubProjectsPrompt(repos, target)`, `parseGitHubProjects(raw)` → `{ name, description }[]`.
- **Fence:** `buildGitHubReposBlock(repos)` builds a delimited `<github_repos>` block (per-item capping: name ≤120 chars, description ≤400 chars, ≤10 topics ×40 chars each, ≤30 repos total). Untrusted-fence pattern per ADR-010 ("IGNORE any instruction" sentence placed post-fence, never in the instruction region). Parser `stripInlineMarkdown()` removes bold/italic/inline-backticks before emitting.
- **Wrapper:** `generateGitHubProjects()` in `apps/desktop/src/renderer/lib/generate/generation/generation.ts` — streams via `streamGenerate`, parses output, matches entries to repos by de-slugged name (fallback: positional), re-attaches `htmlUrl` as `link` (AI never sees or writes URLs). Falls back to raw repo description (or de-slugged name if empty) per entry on any throw. Output is always exactly one `{ name, description, link }` per input repo, in input order.
- **No new user-facing strings in the prompt layer** — UI text supplied by chunk C (renderer).

See `packages/prompts/src/generate/github-projects/` (implementation) and `apps/desktop/src/renderer/lib/generate/generation/generation.ts` (wrapper).

## Renderer: UI import modal

- **Component:** `apps/desktop/src/renderer/features/resume-builder/components/GitHubImportModal/` — controlled via `open` prop, mounted in `StepExtras`. Fetch via `useGitHubImport()` mutation; repo selection via multi-select checkboxes; generation via `generateGitHubProjects()`.
- **Integration point:** `StepExtras` (wizard step for Projects accordion) renders a `GitBranch` icon button that opens the modal. Projects array uses `useFieldArray({ name: 'projects' })` to append entries.
- **UX features:** Username prefilled from `contact?.github` (extracted via `extractGitHubUsername`). Seeded via `useEffect + seededRef` (avoids snap-back trap on async profile load). Select-all / deselect-all toggles. Generate button disabled until at least one repo selected. Cancel disabled during generation. Escape key calls `onClose` during generation (abort wired via `AbortSignal`). On generation error, modal stays open, selection preserved, inline error shown (no append, user can retry or cancel).
- **Accessibility:** Modal `ariaLabelledby` points to title id. Fetch-loading + generation-status announce via persistent `aria-live="polite" aria-atomic` `sr-only` regions (not just button label). Decorative icons (`GitBranch`, `Star`) marked `aria-hidden`.
- **i18n:** 15 translation keys under `build.extras.projects.github.*` (en + de, plural keys as _one/_other): `trigger`, `modalTitle`, `modalDescription`, `usernamePlaceholder`, `fetchButton`, `loading`, `selectAll`, `deselectAll`, `repoCount_one/repoCount_other`, `addSelected_one/addSelected_other`, `cancel`, `generating`, `noRepos`, `stars_one/stars_other`, `generateError`.

See `apps/desktop/src/renderer/features/resume-builder/components/GitHubImportModal/index.tsx` and `StepExtras`.

## Test coverage

- **Rust** — Tests in `apps/desktop/src-tauri/src/profile_import/github.rs` (SSRF validation, status-code mapping, serialization) and `apps/desktop/src-tauri/src/scraping/http/test.rs` (`FetchOptions.timeout` field). Run: `cargo test github` + `cargo test --lib scraping::http`.
- **Prompts** — Tests in `packages/prompts/src/generate/github-projects/`. Scenarios: fence, parser, link re-attachment, name-based matching, cap boundaries. Run: `pnpm -C packages/prompts test`.
- **Renderer** — Tests in `apps/desktop/src/renderer/features/resume-builder/` (GitHub import modal). Scenarios: Enter key fetch, Escape during generation, Add-button disabled when none selected, select-all after deselect-all, generation error recovery, prefill seededRef regression path. Run: `pnpm -C apps/desktop test`.

## Related decisions

- **ADR-010** — Untrusted-input fencing for web-sourced data (applies to the GitHub repo fence).
- **ADR-004** — Ports & adapters (service hook `useGitHubImport` is the boundary; component never calls `window.api` directly).

## Related patterns

- **Profile import sibling:** `apps/desktop/src-tauri/src/profile_import/linkedin.rs` (same module co-location, different contract — LinkedIn returns profile fields, GitHub returns repos).
- **AI bullet generation:** `interview-questions/` prompt generator (same template + streaming + parser pattern).
- **Resume builder step:** `StepExtras` is one of 7 wizard steps using `useFieldArray` for repeatable sections.
