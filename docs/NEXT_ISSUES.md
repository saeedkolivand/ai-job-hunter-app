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

## 3. Indeed import from the Applications page → wrong status + missing docs

When the Applications page is open and a job is imported via the **Indeed** extension path,
the Application goes straight to **applied**. Expected per CONTEXT.md (extension import is a
**Save** origin): create/find a **saved** Application (dedup by normalized URL — reuse if it
exists, don't duplicate), and surface any existing Generation docs (generated CV / cover
letter) for that Application. Two bugs to confirm: (a) status set to `applied` instead of
`saved` on import; (b) existing generation docs not shown for the imported/matched job.
Likely area: extension import → Application origin/dedup logic + the Applications page view.

## 4. CLI providers gemini / antigravity / codex don't work

Tested the gemini CLI, antigravity CLI, and codex CLI as AI providers — none worked. The app
supports CLI-based providers (e.g. Claude Code CLI). Investigate the CLI provider adapter:
detection, invocation, arg/format differences per CLI, and error surfacing. Determine per-CLI
what fails (not found? wrong invocation? unsupported output mode?) and either fix the adapters
or clearly report which are supported.
