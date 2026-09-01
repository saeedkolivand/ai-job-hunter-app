import type { ScrapeProgressEvent } from '../../events/scrape.js';
import type {
  PostingsHybridSearchRequest,
  ScrapeBoardsRequest,
  ScrapeUrlRequest,
} from '../../schemas/index.js';
import type { JobPosting } from '../../types/index.js';

/** Whether one arm of a {@link PostingsHybridSearchResult} actually ran. */
export type HybridSearchArmStatus = 'ran' | 'skipped' | 'unavailable';

export interface HybridSearchArms {
  lexical: HybridSearchArmStatus;
  /** `skipped` when `semanticScoring` is off (the app-wide default); `unavailable` on an embedding failure. */
  dense: HybridSearchArmStatus;
  /** `skipped` when there were fewer than 2 fused candidates; `unavailable` on a rate limit or provider failure. */
  rerank: HybridSearchArmStatus;
}

/** Why a search stopped short of returning ranked results. */
export type HybridSearchOutcome = 'ok' | 'cancelled' | 'staleCorpus';

export interface HybridSearchResult {
  outcome: HybridSearchOutcome;
  /** Ranked posting ids, best first, already limited to the request's `limit`. Empty unless `outcome === 'ok'`. */
  hits: string[];
  arms: HybridSearchArms;
  /** How many postings this search actually ranked over. */
  corpusSize: number;
}

export interface ScrapeContract {
  boards(req: ScrapeBoardsRequest): Promise<{ jobId: string }>;

  url(req: ScrapeUrlRequest): Promise<{ jobId: string }>;

  /**
   * Subscribe to live scrape progress (`scrape:progress`), a coarse
   * boards-done/total fraction (0..1) emitted after each board finishes.
   * Returns a sync unsubscribe. Event-only surface, so it has no request
   * channel in `SCRAPE_CHANNELS` (same shape as `autopilot.onStep`).
   */
  onProgress(handler: (event: ScrapeProgressEvent) => void): () => void;

  /** Resolve a single posting (incl. full description) from its URL. */
  resolveUrl(req: { url: string }): Promise<JobPosting | null>;

  /**
   * Write a freshly-resolved full description back into the live postings cache
   * by posting id, so the match scorer reads the full text instead of the
   * truncated aggregator snippet. Returns `true` when an entry was updated,
   * `false` when the id is no longer in the live cache.
   */
  updateDescription(req: { id: string; description: string }): Promise<boolean>;

  listPostings(): Promise<JobPosting[]>;

  clearPostings(): Promise<void>;

  listInteractions(filter?: { interactionType?: string }): Promise<
    Array<{
      jobId: string;
      interactionType: string;
      timestamp: number;
      title: string;
      company: string;
      url: string;
      source: string;
      location: string;
    }>
  >;

  persistJob(req: { job: Record<string, unknown>; interactionType: string }): Promise<void>;

  /**
   * Delete a persisted interaction — the real "undo" for {@link persistJob},
   * e.g. reversing an accidental `dismissed` write. Keys on the SAME
   * `(jobId, interactionType)` pair `persistJob` writes under (the stored
   * `InteractionRecord.job_id`, not necessarily a cluster/UI key) — passing a
   * different `jobId` silently removes nothing. Returns `true` when a record
   * was removed, `false` when there was nothing to remove, so a caller can
   * tell "undone" apart from "there was nothing there".
   */
  removeInteraction(req: { jobId: string; interactionType: string }): Promise<boolean>;

  /**
   * Rank the live postings cache (or a caller-supplied eligible subset of
   * it) by lexical + optional dense relevance to a query — see
   * `commands::hybrid_search` (Rust) for the lexical/dense/fusion/rerank
   * pipeline. NOT abortable via the invoke promise itself: to supersede an
   * in-flight search, call `jobs.cancel(queryId)` with the SAME `queryId`
   * this request was sent with (hybrid search registers against the
   * app-wide `CancelRegistry` every job kind already cancels through —
   * there is no separate cancel channel here).
   */
  hybridSearch(req: PostingsHybridSearchRequest): Promise<HybridSearchResult>;
}

export const SCRAPE_CHANNELS = {
  boards: 'scrape:boards',
  url: 'scrape:url',
  resolveUrl: 'scrape:resolveUrl',
  updateDescription: 'scrape:updateDescription',
  listPostings: 'scrape:listPostings',
  persistJob: 'scrape:persistJob',
  removeInteraction: 'scrape:removeInteraction',
  clearPostings: 'scrape:clearPostings',
  listInteractions: 'scrape:listInteractions',
  hybridSearch: 'scrape:hybridSearch',
} as const;
