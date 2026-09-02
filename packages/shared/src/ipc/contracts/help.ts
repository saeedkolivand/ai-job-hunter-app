import type { HelpSearchRequest, HelpSearchResult } from '../../schemas/index.js';

export interface HelpContract {
  /**
   * Rank renderer-supplied help entries against a user question — the
   * retrieval half of the in-app help chat.
   *
   * The corpus is NOT duplicated into Rust: the shipped help entries live in
   * the translation bundles (`support.faq.<section>Questions.<id>.{q,a}`) and
   * are rendered by `features/support/support-data.ts`, so the caller sends
   * the ACTIVE locale's entries with every question and `commands::help` does
   * only the retrieval math. The reply carries entry ids and scores only —
   * never a copy of the text the caller already holds.
   *
   * Reuses the same L1 `retrieval` module as `scrape:hybridSearch`
   * (ADR-039): a lexical BM25 arm over `title`/`body`, an optional dense arm,
   * and reciprocal-rank fusion. The dense arm is gated on the SAME
   * `semanticScoring` preference that gates hybrid postings search, so a search
   * surface never spends against a paid embedding provider without an opt-in:
   * off → `arms.dense: 'skipped'`. An embedding failure is likewise never an
   * error to the user — the keyword results still come back with
   * `arms.dense: 'unavailable'`. Read `mode` (not the presence of results) to
   * decide whether the UI may call the ranking semantic.
   *
   * Every cap in `HelpSearchRequestSchema` is re-checked server-side: this is
   * an IPC boundary a non-UI caller (the agent CLI, a crafted extension
   * message) reaches directly, bypassing Zod entirely.
   *
   * Not abortable: v1 has no cancellation channel (unlike `hybridSearch`'s
   * `queryId`), so a superseded question is discarded by the caller rather
   * than cancelled in the backend.
   */
  search(req: HelpSearchRequest): Promise<HelpSearchResult>;
}

export const HELP_CHANNELS = {
  search: 'help:search',
} as const;
