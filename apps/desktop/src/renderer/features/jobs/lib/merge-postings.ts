import type { Posting } from '../types';
import { canonicalJobKey } from './canonical-job-key';

/** UTF-8 byte length, matching Rust's `str::len()` (bytes, not UTF-16 code
 * units) — required so the description tie-break can never flip vs the Rust
 * side's pick for a multi-byte string. */
function byteLength(s: string): number {
  return new TextEncoder().encode(s).length;
}

/**
 * Union two postings' `interactions`, deduping exact-duplicate entries.
 * `JobInteraction` is a flat record (no nested objects/arrays), so a
 * `JSON.stringify` identity is a trivial, correct dedupe key here.
 */
function mergeInteractions(
  incumbent: Posting['interactions'],
  challenger: Posting['interactions']
): Posting['interactions'] | undefined {
  const all = [...(incumbent ?? []), ...(challenger ?? [])];
  if (all.length === 0) return incumbent; // preserve undefined vs [] as-is
  const seen = new Set<string>();
  const deduped = all.filter((i) => {
    const key = JSON.stringify(i);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  return deduped;
}

/**
 * Collapse a later duplicate (`challenger`) into the first-seen `incumbent` for
 * the same canonical key. Mirrors the corrected trust-PR-E survivor policy
 * (`autopilot::merge_found_jobs` + the engine's `dedup_cross_source`):
 *
 * - **Incumbent identity is never swapped.** `id`/`url`/`source` (and every other
 *   field the incumbent already holds) are kept — user-state (viewed/saved/applied
 *   flags and the detail-pane selection) is keyed by `id`, so the survivor must
 *   keep the id the user may have already acted on.
 * - **`interactions` are UNIONED, not kept-incumbent-only** — two already-persisted
 *   legacy duplicates (pre-dating the engine's dedup pass) can each carry distinct
 *   saved/applied state; keeping only the incumbent's would silently drop the
 *   challenger's. Exact-duplicate entries collapse via `mergeInteractions`.
 * - **Longer description wins** (aggregator snippets are truncated), compared by
 *   UTF-8 byte length so the pick can never flip vs Rust's byte-`len()` pick for a
 *   multi-byte description; ties keep the incumbent.
 * - **Enrichment the incumbent LACKS is filled from the challenger**, never the
 *   reverse — a non-empty incumbent value always wins. This is the renderer analog
 *   of the Rust `extra`-map union: a whole-row replace would silently DROP the
 *   incumbent's salary (Adzuna's `salaryMin/Max/Currency`, consumed by
 *   usePostingActions / TailorFlow / ApplicationDetailPage) when a direct-board
 *   duplicate with no salary beat it on description length. `trust` (the ghost-job
 *   signal) is filled the same way so a collapse never regresses a row from "has a
 *   trust read" to "none". This fill-list must track every key any board writes
 *   into `JobPosting.extra` — lockstep with Rust `merge_extra`.
 */
function collapseDuplicate(incumbent: Posting, challenger: Posting): Posting {
  return {
    ...incumbent,
    description:
      byteLength(challenger.description) > byteLength(incumbent.description)
        ? challenger.description
        : incumbent.description,
    interactions: mergeInteractions(incumbent.interactions, challenger.interactions),
    // Fill only where the incumbent has nothing; `??` keeps a present incumbent value.
    remote: incumbent.remote ?? challenger.remote,
    salaryMin: incumbent.salaryMin ?? challenger.salaryMin,
    salaryMax: incumbent.salaryMax ?? challenger.salaryMax,
    salaryCurrency: incumbent.salaryCurrency ?? challenger.salaryCurrency,
    trust: incumbent.trust ?? challenger.trust,
  };
}

/**
 * Merge live (streamed) and persisted postings into a single deduplicated list.
 *
 * Two passes:
 *
 * 1. **By id** — backend `postings` win on duplicate ids: they carry interactions
 *    and the persisted full description. Streamed `livePostings` are added only
 *    when they have no backend counterpart yet (mid-scrape, not yet persisted).
 *
 * 2. **By canonical key** — the same job surfaced by two sources gets DIFFERENT
 *    board-prefixed ids, so pass 1 alone can't collapse it. The engine already
 *    dedupes the persisted vec, but the manual-scrape live stream is NOT deduped
 *    (it streams raw per-board items), so two boards emitting the same job leave
 *    two live rows until this pass reconciles them — closing the live-stream
 *    window and any cross-append duplicate. Keyed by `canonicalJobKey`, the exact
 *    TS mirror of the Rust `canonical_job_key` that the engine + autopilot use, so
 *    the displayed row count matches the deduped completion count. Because pass 1
 *    puts backend `postings` before `livePostings`, a persisted row (which carries
 *    the user's interactions) is the incumbent over a streamed duplicate. See
 *    {@link collapseDuplicate} for the survivor policy.
 *
 * @param absorbed Optional out-param: when a challenger id collapses into a
 *   survivor, `absorbed.set(challengerId, survivorId)` is recorded. Traceability
 *   for callers that hold external state keyed by posting id (concretely:
 *   `JobsResults`' `selectedId`) — a `board`-id row can be selected while it's
 *   the only live copy, then later absorbed into an `aggregator`-id incumbent
 *   once the persisted list refetches (boards stream at different speeds; the
 *   engine's incumbent-selection order need not match display arrival order).
 *   Without this, the id the caller has selected silently vanishes from the
 *   merged list and any "selection missing → fall back to top" reconciliation
 *   swaps the open detail pane to an unrelated job. An out-param (not a return-
 *   shape change) was chosen to keep every existing `mergePostings(...)` call
 *   site — which uses the return value as a plain `Posting[]` — unchanged; only
 *   `JobsPage` (which owns `selectedId`) needs to pass one.
 */
export function mergePostings(
  postings: Posting[],
  livePostings: Posting[],
  absorbed?: Map<string, string>
): Posting[] {
  // Pass 1 — merge by id (backend wins).
  const byId = new Map<string, Posting>();
  for (const p of postings) byId.set(p.id, p);
  for (const p of livePostings) if (!byId.has(p.id)) byId.set(p.id, p);

  // Pass 2 — collapse cross-source duplicates by canonical key. `survivors`
  // preserves first-seen order; `indexByKey` maps a canonical key to its slot.
  const survivors: Posting[] = [];
  const indexByKey = new Map<string, number>();
  for (const p of byId.values()) {
    const key = canonicalJobKey(p.url, p.title, p.company);
    const idx = indexByKey.get(key);
    if (idx === undefined) {
      indexByKey.set(key, survivors.length);
      survivors.push(p);
      continue;
    }
    const incumbent = survivors[idx];
    if (incumbent) {
      survivors[idx] = collapseDuplicate(incumbent, p);
      if (p.id !== incumbent.id) absorbed?.set(p.id, incumbent.id);
    }
  }
  return survivors;
}
