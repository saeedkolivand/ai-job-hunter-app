# ADR-041 — Searchable help page over compiled-in, localized entries: client-side substring matching, no model or IPC

**Status:** Accepted

**Date:** 2026-09-02

**Deciders:** owner (decision 2026-08-18, re-confirmed 2026-08-20), main session

## Context

[ADR-0006](0006-support-page-faq-only-diagnostics-removed.md) made the Support page intentionally FAQ-only when it removed the unreachable diagnostics/health/recovery/KnowledgeBase dashboard. The remaining page is an accordion of troubleshooting answers.

The real gap surfaced when the owner demonstrated the app to another person and had to explain it in person: users need to answer "how do I do X" at any moment, not only "why is X broken". A user cannot search the app for instructions; the FAQ covers only failures.

Two earlier proposals were considered and set aside in 2026-08:

1. An in-app help chatbot (mini-RAG over compiled-in help snippets): a `help_search` Rust command, hybrid keyword + embedding retrieval, a `help_vectors` table, grounded streaming answers. Fully planned; parked because its premise — that a small local model cannot hold the manual, so retrieval is required — assumes the answer must come from a model at all.

2. A ~550-line "job-hunt copilot RAG" prompt over the user's own data. Reviewed 2026-08-20 and rejected: it is a power-user analytics surface addressing a different use case; most of it already exists in the app; it does not address discoverability.

Both proposals assumed complexity was necessary; the gap is discoverability, which a direct search over compiled-in entries solves.

## Decision

**1. The Support page becomes a searchable help page**: a search box over compiled-in help entries, filtered client-side in the renderer, covering both troubleshooting (the existing FAQ) and how-to topics grouped by feature area.

**2. No model, no embeddings, no Rust command, no IPC, no new table, no new route**: the page reads its entries from the same place the FAQ already does — the `support-data` module in the support feature plus the `support` keys of the `en` and `de` translation resources. Entries are localized; a renderer test asserts that every key the page uses resolves in both locales (the CI extractor step is advisory and cannot fail the build).

**3. Matching is deliberate simple substring word-matching**: client-side, over the already-translated question and answer text. The corpus is a few dozen entries and a plain string scan is the correct tool.

**4. This partly supersedes ADR-0006**: the "Support is FAQ-only" statement no longer holds; the rest of ADR-0006 (the diagnostics/recovery dashboard is removed and stays removed; the export-diagnostics action lives in Settings) is unchanged. This is a new feature, not a revival of the deleted dashboard.

**5. The parked help-chatbot remains a possible later layer** over the same entries if usage shows that search alone leaves questions unanswered; it is not part of this decision.

**6. Content accuracy is the requirement, not the mechanism**: every entry is written against the current UI (labels, menu paths) and stale answers in the existing FAQ are corrected in the same change; an entry that cannot be verified against the code is left out rather than guessed.

## Considered options

1. **Keep FAQ-only** (rejected): it cannot answer how-to questions and the owner's demonstration showed that gap is real — users asking "how do I do X" cannot find answers in a troubleshooting-only page.

2. **The help chatbot mini-RAG** (deferred): answers the same questions with a model, embeddings, a new command and table, and a per-question spend, for a corpus a person can search directly. Parked; now a possible follow-up if usage shows search alone is insufficient.

3. **An external docs site** (rejected): the app is local-first and offline-capable; the answer must be available inside the app at the moment of the question, not on a website.

4. **A new onboarding flow** (rejected here): an onboarding wizard and spotlight tour already exist and are first-run-only; the "what should I do next" half is a separate, later decision about persistence on the Dashboard, not part of this record (now [ADR-042](adr-042-dashboard-next-step-tile-derived-not-stored.md)).

## Consequences

### Positive

- **Search surface for how-to questions**: users can find answers for "how do I export" or "what does the match score mean" without clicking through every section of an accordion.

- **Existing troubleshooting answers corrected**: stale phrases like "AI requires Ollama" (the app offers many providers) and "clear localStorage via DevTools" (use the real Settings action) are corrected in the same change.

- **No new infrastructure**: entries live in translation resources, with a test pinning en/de parity for every key the page uses; no new command, no new table, no new route, no bridge call.

- **Localization built in**: every entry has `en` and `de` translations paired by key, and a test fails when either locale lacks one.

### Tradeoffs and costs

- **Substring matching (case- and diacritic-folded, so `resume` finds `résumé`) has no synonym or typo tolerance**: a user asking "where do I upload my CV" will not find a page titled "import documents" unless they phrase it that way. Mitigation: write questions the way users phrase them (in user research language, not feature naming). The chatbot layer is the upgrade path if that proves insufficient.

- **The corpus must be kept current when UI labels change**: the cost of accuracy lives in the translation files and the indexed source paths. Every label must be re-verified when a UI text changes.

## References

- Support page component: `apps/desktop/src/renderer/features/support/components/SupportPage/index.tsx`
- Data source: `apps/desktop/src/renderer/features/support/support-data.ts`
- Localization: `packages/translations/src/locales/{en,de}/translation.json` under the `support` key
- [ADR-0006](0006-support-page-faq-only-diagnostics-removed.md) — the FAQ-only decision, partly superseded by this one
