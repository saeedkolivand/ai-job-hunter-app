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
