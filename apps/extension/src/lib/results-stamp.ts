/**
 * Results-page stamps (PR3 §B.4) — a single-gesture pass over a
 * results-listing page's job-card links: collect candidate URLs (step one),
 * then stamp each card "saved"/"applied" once the background resolves them
 * via `applied.check.batch` (step two).
 *
 * Detection is GENERIC, in the style of `content.ts::markLikelyJobNode` — a
 * case-insensitive href pattern, never board-specific selectors. Under-claim
 * over mis-stamp: an ambiguous/duplicate link is skipped rather than
 * stamped against the wrong card.
 *
 * Injected via `results-stamp.ts` (compiled to `results-stamp.js`, a classic
 * script), mirroring `fill.ts`'s two-step register-then-invoke pattern — but
 * with TWO invoke calls sharing one registration: {@link collectResultsCards}
 * runs first and remembers the matched anchors (module-level state, alive
 * for the lifetime of this injected instance); {@link stampResultsCards}
 * reads that same state back by array INDEX, since the background's
 * `applied.check.batch` reply preserves the request's url order exactly
 * (the Rust contract — see the shared protocol doc).
 *
 * Pure DOM (plus the shared `field-signal.isHidden` import, same convention
 * as `content.ts`) — no extension APIs — so it is unit-testable against a
 * jsdom document.
 */

import { isHidden } from './field-signal';
import { currentNotebookPalette, type NotebookPalette } from './notebook-palette';

/** Isolated-world global key exposing {@link collectResultsCards}'s
 *  entry-point. MUST match the literal duplicated in `background.ts`. */
export const RESULTS_COLLECT_GLOBAL = '__ajhCollectResultsCards';
/** Isolated-world global key exposing {@link stampResultsCards}'s
 *  entry-point. MUST match the literal duplicated in `background.ts`. */
export const RESULTS_STAMP_GLOBAL = '__ajhStampResultsCards';

/** Mirrors the Rust `applied_check_batch::MAX_BATCH_URLS` / the shared
 *  `MAX_APPLIED_CHECK_BATCH_URLS` constant — capped client-side too so the
 *  collector never even BUILDS an over-cap request. */
export const MAX_STAMP_CARDS = 50;

/** A job-card link looks like a job-posting URL: a `/job(s)/`, `/career(s)/`,
 *  `/position(s)/`, or `/posting(s)/` path segment, or a `jk`/`jobid`/
 *  `job_id` query param — the same generic, path/param-shaped heuristic
 *  `markLikelyJobNode` uses for a job CONTAINER, applied here to a link. */
const JOB_HREF_HINT = /\/(jobs?|careers?|positions?|postings?)\/|[?&](jk|jobid|job_id)=/i;

/** One collected candidate, echoed to the caller so it can build the
 *  `applied.check.batch` request. */
export interface CollectedCard {
  url: string;
  index: number;
}

/** The anchors matched by the most recent {@link collectResultsCards} call,
 *  indexed the SAME way as the URLs handed back — read back by
 *  {@link stampResultsCards}. Module-level (not a class): this file is
 *  injected once per gesture and both entry-points share it. */
let candidateAnchors: HTMLAnchorElement[] = [];
/** The stamp HOST most recently placed for a given anchor (if any) — lets a
 *  re-run REPLACE rather than duplicate within THIS injected instance.
 *  Keyed by the anchor itself. Not the only dedup guard: a fresh injection
 *  starts with empty maps while the previous instance's hosts are still on
 *  the page, so {@link clearStamp} also does the DOM-based sibling scan —
 *  the map is the fast path, the scan is the correctness path (#1220). */
const stampNodes = new WeakMap<HTMLAnchorElement, HTMLElement>();
/** The closed shadow root placed inside a given anchor's stamp host — a
 *  page's own scripts can't reach this (`host.shadowRoot` is null, `open`
 *  mode not used), same isolation `fit-badge.ts`'s `renderFitBadge` uses.
 *  Test-only lookup via {@link peekStampShadow}. */
const stampShadows = new WeakMap<HTMLAnchorElement, ShadowRoot>();

/**
 * Scan `doc` for job-card links: dedupe by normalized absolute url (first
 * occurrence wins — a repeat is a nav/pagination echo, not a second card),
 * skip hidden anchors, cap at {@link MAX_STAMP_CARDS}. Resets the shared
 * {@link candidateAnchors} index so a later {@link stampResultsCards} call
 * maps back to THIS pass, not a stale one.
 */
export function collectResultsCards(doc: Document): CollectedCard[] {
  candidateAnchors = [];
  const seen = new Set<string>();
  const out: CollectedCard[] = [];

  for (const a of Array.from(doc.querySelectorAll<HTMLAnchorElement>('a[href]'))) {
    if (out.length >= MAX_STAMP_CARDS) break;
    const href = a.getAttribute('href');
    if (!href || !JOB_HREF_HINT.test(href)) continue;
    if (isHidden(a)) continue;

    let resolved: URL;
    try {
      resolved = new URL(href, doc.location.href);
    } catch {
      continue; // an unparsable href is never a valid card link
    }
    // Same-origin only — a genuine results card never needs to link an
    // attacker-controlled third-party domain, and this page's own script
    // authored the href, so nothing here is a trust boundary we can rely on.
    if (resolved.origin !== doc.location.origin) continue;
    const url = resolved.toString();
    if (seen.has(url)) continue; // ambiguous repeat — skip rather than mis-stamp
    seen.add(url);

    const index = candidateAnchors.length;
    candidateAnchors.push(a);
    out.push({ url, index });
  }

  return out;
}

/** One url's outcome — mirrors `ExtensionAppliedBatchEntry` structurally
 *  (this module stays wire-type-free: it is a classic-script injection
 *  target and never imports `@ajh/shared`). */
export interface StampInput {
  url: string;
  found: boolean;
  status?: string;
}

/** Upper bound on forward siblings examined per card while hunting stale
 *  hosts — see {@link clearStamp}. */
const MAX_STAMP_DEDUP_SCAN = 8;

/**
 * Is `node` one of OUR stamp hosts? The page is attacker-controlled, so a
 * `data-ajh-stamp` attribute alone is not proof — a page-authored span may
 * carry it. The check requires the full placement signature: the marker
 * attribute AND the exact inline-style triplet `placeStamp` writes
 * (`display: inline-flex`, `vertical-align: middle`, `margin-left: 6px`).
 * `node instanceof HTMLSpanElement` narrows to the concrete element type
 * that actually has a `.style` map before any style property is read
 * (`Element` has none, and a `tagName` string check would not narrow for
 * the compiler); it also deliberately leaves a foreign-realm (XML/SVG)
 * `span` alone — such a node cannot be a host this module created.
 */
function isStampHost(node: Element): boolean {
  // `Element` has no style map — narrow to the concrete element type that
  // has one BEFORE reading style (a tagName check alone never narrows for
  // the compiler). The realm check also under-claims a little more than a
  // tagName-only read: a foreign-realm `span` (XML/SVG) is left alone
  // rather than probed for properties it may not carry.
  if (!(node instanceof HTMLSpanElement)) return false;
  if (node.getAttribute('data-ajh-stamp') !== 'true') return false;
  const style = node.style;
  return (
    style.display === 'inline-flex' &&
    style.verticalAlign === 'middle' &&
    style.marginLeft === '6px'
  );
}

/**
 * Remove any stamp previously placed for `anchor` — including stamps placed
 * by an EARLIER injected instance of this file.
 *
 * The in-memory {@link stampNodes} map only knows this instance's hosts; the
 * file is injected fresh on every Stamp click, so the map is re-created
 * empty while the previous instance's hosts are still in the page. The maps
 * therefore earn their place within a single injection (fast path, shadow
 * lookup) but cannot be the only guard, so after consulting the map this
 * does a DOM-based, bounded forward-sibling scan from the anchor: at most
 * {@link MAX_STAMP_DEDUP_SCAN} `nextElementSibling` steps, removing each
 * node that matches {@link isStampHost}'s exact signature and STOPPING at
 * the first node that does not.
 *
 * Attacker-controlled page: a page node is only removed if it sits in that
 * bounded run directly after this anchor AND carries our exact placement
 * signature — a lone page-owned marker elsewhere (or the first sibling,
 * which halts the scan) is never touched. Cost shape: at most
 * {@link MAX_STAMP_DEDUP_SCAN} signature checks per card, no
 * whole-document query.
 */
function clearStamp(anchor: HTMLAnchorElement): void {
  // This instance's own host, if any: drop the map entries first so the
  // sibling scan below can never re-find and double-handle it.
  const known = stampNodes.get(anchor);
  stampNodes.delete(anchor);
  stampShadows.delete(anchor);
  known?.remove();

  let node = anchor.nextElementSibling;
  for (let checked = 0; node && checked < MAX_STAMP_DEDUP_SCAN; checked += 1) {
    if (!isStampHost(node)) break; // first non-stamp sibling ends the run
    const stale = node;
    node = node.nextElementSibling; // advance before removal
    stale.remove();
  }
}

/** Place (or replace) a small inline stamp right after `anchor`. Only the
 *  stamp's CONTENT — which label it shows, "Saved" vs "Applied" — renders
 *  inside a CLOSED shadow root on the host, so the page's own scripts (the
 *  same ones that authored the card anchors) can't read that text back off
 *  the shared DOM, same isolation `fit-badge.ts`'s `renderFitBadge` uses and
 *  for the same reason. The HOST element itself (`span[data-ajh-stamp]`) is
 *  visible in the light DOM by design — the feature IS a marker the user can
 *  see next to a matched card — so a page's own script can still observe
 *  that a lookup matched this anchor (its presence + position), just never
 *  which status it matched. */
function placeStamp(
  doc: Document,
  palette: NotebookPalette,
  anchor: HTMLAnchorElement,
  status: string | undefined
): void {
  clearStamp(anchor);

  const host = doc.createElement('span');
  host.setAttribute('data-ajh-stamp', 'true');
  host.style.cssText = 'display:inline-flex;vertical-align:middle;margin-left:6px';
  const shadow = host.attachShadow({ mode: 'closed' });

  const wrap = doc.createElement('span');
  const applied = status === 'applied';
  wrap.textContent = applied ? 'Applied' : 'Saved';
  wrap.style.cssText = [
    'display:inline-flex',
    'align-items:center',
    'gap:4px',
    'padding:1px 7px',
    'border-radius:999px',
    'font:11px/1.6 system-ui,-apple-system,sans-serif',
    `border:1px solid ${applied ? palette.ok : palette.inkSoft}`,
    `color:${applied ? palette.ok : palette.inkSoft}`,
    `background:${palette.card}`,
  ].join(';');

  const dismiss = doc.createElement('button');
  dismiss.type = 'button';
  dismiss.textContent = '×';
  dismiss.setAttribute('aria-label', 'Dismiss this stamp');
  dismiss.style.cssText =
    'border:0;background:transparent;color:inherit;cursor:pointer;font:inherit;padding:0;line-height:1';
  dismiss.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    clearStamp(anchor);
  });
  wrap.append(dismiss);
  shadow.append(wrap);

  anchor.insertAdjacentElement('afterend', host);
  stampNodes.set(anchor, host);
  stampShadows.set(anchor, shadow);
}

/** Test-only escape hatch: the closed shadow root placed for `anchor`, if
 *  any. `stampResultsCards` itself only ever returns a COUNT (to the
 *  background, never to the page) — the per-anchor closed-root CONTENT it
 *  built is otherwise unobservable outside this module, so a test needs this
 *  side door (module-internal, never sent anywhere) to assert the isolated
 *  content actually rendered. This says nothing about the visible HOST
 *  element itself, which is deliberately observable — see {@link
 *  placeStamp}'s doc. */
export function peekStampShadow(anchor: HTMLAnchorElement): ShadowRoot | undefined {
  return stampShadows.get(anchor);
}

/**
 * Stamp every FOUND result against the anchor {@link collectResultsCards}
 * matched at the same index. `results` is expected in the SAME order the
 * collected urls were sent in (the desktop's `applied.check.batch` reply
 * preserves input order) — an out-of-range index or an unresolved anchor
 * (the page changed between collect and stamp) is silently skipped, never
 * guessed. Returns the number of cards actually stamped.
 */
export function stampResultsCards(
  doc: Document,
  palette: NotebookPalette,
  results: StampInput[]
): number {
  let stamped = 0;
  results.forEach((r, index) => {
    if (!r.found) return;
    const anchor = candidateAnchors[index];
    if (!anchor || !doc.contains(anchor)) return;
    placeStamp(doc, palette, anchor, r.status);
    stamped += 1;
  });
  return stamped;
}

/** The `collect` injected entry-point — a plain array of plain
 *  `{url, index}` objects (JSON-safe), read via `executeScript`'s `func`
 *  return value (not a `files` completion value — see the entry file). */
export function runCollectResultsCards(): CollectedCard[] {
  return collectResultsCards(document);
}

/** The `stamp` injected entry-point. */
export function runStampResultsCards(results: StampInput[]): number {
  const palette = currentNotebookPalette(window);
  return stampResultsCards(document, palette, results);
}
