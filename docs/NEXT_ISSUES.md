# Next Issues — to plan after the audit-triage branch lands

Captured 2026-07-08. Work these in **plan mode** once `fix/audit-triage-p1` is
through review + PR. Each needs investigation before a fix.

## 1. Company web-search in tailoring — default ON when the model supports it

Keep the "search company" option (web-grounded company research used when tailoring
a résumé / cover letter) **enabled by default IF the selected AI model supports search
/ grounding**. Today it appears to default off (or unconditionally). Gate the default on
a provider/model _capability_ (search/grounding support), not a hardcode — respect the
"new model/provider works with zero changes" preference. Likely area: ai-provider model
capabilities + the tailoring/cover-letter flow's default for the research toggle.

## 2. Email subject not copyable

An email subject field somewhere in the app cannot be copied (no copy affordance / not
selectable). Find the email-subject UI (likely an outreach / referral / application email
surface) and make it copyable (a copy button or selectable text), consistent with other
copy affordances in the app.

## 3. [CLOSED] Indeed import from the Applications page → wrong status + missing docs

**Resolution (PR #630, 2026-07-11):** The extension import flow now correctly performs full
dedup-merge into pre-existing Applications by normalized URL, surfacing matched Application
metadata (title, company, status, appliedAt) via the new `applied.check` bridge verb. When an
already-saved job is imported again, the popup now shows its current status instead of
reporting "not found", and the Applications page honors the matched Application's existing
status without creating a duplicate. The dedup-merge logic in `handle_import` + the
`applied.check` read-only bridge verb together close this issue. Existing generation docs are
now discoverable for matched Applications via the standard Applications page UI.

## 4. CLI providers gemini / antigravity / codex don't work

Tested the gemini CLI, antigravity CLI, and codex CLI as AI providers — none worked. The app
supports CLI-based providers (e.g. Claude Code CLI). Investigate the CLI provider adapter:
detection, invocation, arg/format differences per CLI, and error surfacing. Determine per-CLI
what fails (not found? wrong invocation? unsupported output mode?) and either fix the adapters
or clearly report which are supported.

## 5. Flip `agent_run` off a request-supplied `base_url`

Task #16 moved the active AI provider config to a backend-owned store (`ai_config::AiConfigStore`)
so the generation commands resolve routing from _there_ instead of trusting the renderer: `ai_generate`,
`generate_pipeline`, research/salary, the extension bridge's `resolve_answer_assist`, and autopilot are
all flipped. The `agent_run` ("prep this application") agent-loop path (`commands/agent.rs` →
`run_agent_live`) is **not** flipped yet — it still resolves via `Completer::resolve(..., req.base_url)`
and threads the same renderer-supplied `req.base_url` into `agent::tools::ToolContext` for tool calls
(e.g. `research_company`). Flip this path onto `Completer::from_active` + a store-resolved
`ToolContext` so no generation command anywhere still accepts a request base_url.

## 6. [LOW] Drop the dead autopilot `assistant_provider/model/base_url` fields

Task #16 moved autopilot's assistant-notes provider resolution onto the backend
`AiConfigStore` (`Completer::from_active`); the renderer wizard now always sends
`assistantProvider`/`assistantModel`/`assistantBaseUrl` as `undefined` (left vestigial
per the "leave the rest intact" call at the time). Remove the now-dead fields from the
renderer `WizardState`/schema (`apps/desktop/src/renderer/features/autopilot/types.ts`,
`lib/schema.ts`, `lib/wizard-state.ts`) and the corresponding struct/deserialize/update
fields on the Rust `Autopilot` record, once nothing reads them.

## 7. [LOW] Write-path mutate-arg assertions for the new AI-provider setter hooks

`apps/desktop/src/renderer/services/use-ai-provider/use-ai-provider.test.ts` (task #16)
only has one generic smoke test (`exerciseServiceHooks` — renders every exported hook
without crashing). None of `useSetActiveProvider`/`useSetProviderSettings`/
`useConfigureActiveProvider` has a test asserting the exact mutate-argument shape sent
to `tauri-client` (provider/model/baseUrl) or that `keys.ai.activeConfig` is invalidated
on success. Pre-existing test gap — same class of assertion the extension-bridge boolean
mutation test (`use-extension-bridge.test.ts`) already has for its own setter.
