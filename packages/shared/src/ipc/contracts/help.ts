import type { HelpSearchRequest, HelpSearchResult } from '../../schemas/index.js';

export interface HelpContract {
  /**
   * Rank renderer-supplied help entries against a user question — the
   * retrieval half of the in-app help chat (ADR-043).
   *
   * The corpus is NOT duplicated into Rust. The caller sends the ACTIVE
   * locale's entries with every question and the reply names entry ids and
   * scores only, never a copy of the text the caller already holds.
   *
   * Request shape, its per-field caps and their defaults live in
   * `HelpSearchRequestSchema`; the reply shape — including `mode` and the
   * per-arm `arms` outcomes a UI must read before calling a ranking semantic —
   * in `HelpSearchResultSchema`. The ranking itself, the preference that gates
   * the dense arm, and the server-side re-validation every non-UI caller
   * depends on are owned by `commands/help.rs`.
   */
  search(req: HelpSearchRequest): Promise<HelpSearchResult>;
}

export const HELP_CHANNELS = {
  search: 'help:search',
} as const;
