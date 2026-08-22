import type { BoardHealth, BoardScrapeSummary } from '@ajh/shared';
import { type TFunction, useTranslation } from '@ajh/translations';
import { cn, Tag } from '@ajh/ui';

import { regionName } from '@/lib/region-name';
import { timeAgo } from '@/lib/time';

/**
 * Compact per-board diagnostics strip. Renders a `BoardScrapeSummary[]` as one
 * chip per board — `board · count | reason` — so a zero / partial / failed run is
 * always explainable instead of a silent "found 0". Shared by the Jobs page
 * (live manual scrape) and the Autopilot card (persisted last-run summaries), so
 * it lives outside both feature dirs (no cross-feature import).
 *
 * Variants: success (green count), error (red, sanitized reason), skipped
 * (neutral "config" tone, mapped reason), truncated (amber "partial"), note
 * (blue "informational" — a benign location policy the board applied, e.g. a
 * broadened / guessed market). When EVERY board succeeded, the whole strip
 * collapses to one compact "N boards · all ok" chip instead of one green chip
 * per board (noise reduction — a clean run doesn't need a per-board breakdown).
 */

/** Max length of a sanitized reason — a hint, not a full error dump. */
const MAX_REASON_LEN = 200;

/** Max length of a chip's DISPLAYED detail text — tighter than the sanitize
 *  ceiling above so a long (esp. German) reason never forces horizontal
 *  overflow on the 900×600 window floor; the chip itself also wraps. */
const CHIP_DETAIL_MAX = 60;

function capDetail(text: string): string {
  return text.length > CHIP_DETAIL_MAX ? `${text.slice(0, CHIP_DETAIL_MAX)}…` : text;
}

/** Leading/trailing wrapping punctuation to strip before classifying a token. */
const TRIM_PUNCT = /^["'`([\]{}<>|,;:]+|["'`()[\]{}<>|,;:]+$/g;

/**
 * Client-side mirror of the Rust `sanitize_reason` intent: an error string that
 * crossed IPC (or was persisted to disk) may carry absolute paths, full URLs,
 * host:port, emails, or credential fragments. We do NOT trust persisted strings
 * (PR B carry-over 4), so each whitespace token that looks like one of those is
 * collapsed to a neutral placeholder before display; the human message around it
 * (e.g. `"429 Too Many Requests"`) is kept intact. Over-redaction is always safe.
 */
export function sanitizeReason(raw: string): string {
  if (typeof raw !== 'string') return '';
  // Pre-cap the INPUT before tokenizing (independent of the MAX_REASON_LEN
  // truncation of the OUTPUT below) so a pathological multi-megabyte string
  // can't force an unbounded split/map over the full length.
  const capped = raw.length > 1000 ? raw.slice(0, 1000) : raw;
  const out = capped.split(/\s+/).filter(Boolean).map(redactToken).join(' ');
  return out.length > MAX_REASON_LEN ? `${out.slice(0, MAX_REASON_LEN)}…` : out;
}

/** Classify a single token and swap it for a placeholder when it leaks context. */
function redactToken(token: string): string {
  const trimmed = token.replace(TRIM_PUNCT, '');
  if (!trimmed) return token;
  const lower = trimmed.toLowerCase();

  const isUrl = trimmed.includes('://');
  const isCredential = [
    'key=',
    'app_id=',
    'secret=',
    'token=',
    'password=',
    'pwd=',
    'auth=',
    'key":',
    'secret":',
    'token":',
    'password":',
    'auth":',
  ].some((marker) => lower.includes(marker));
  const isWindowsPath = /^[a-z]:[\\/]/i.test(trimmed);
  const isUnixPath = trimmed.startsWith('/') && trimmed.slice(1).includes('/');
  const isHomeish =
    lower.includes('users\\') || lower.includes('users/') || lower.includes('home/');
  // UNC network path — `\\server\share\...` — leaks the user's network layout
  // same as a local absolute path.
  const isUncPath = trimmed.startsWith('\\\\');

  const segs = trimmed.split('.').filter(Boolean);
  const dottedIpv4 = segs.length === 4 && segs.every((seg) => /^\d+$/.test(seg));
  const colonIdx = trimmed.lastIndexOf(':');
  const preColon = colonIdx > 0 ? trimmed.slice(0, colonIdx) : '';
  const preColonSegs = preColon.split('.').filter(Boolean);
  // A bare "host:port" needs a hostname-LIKE part before the colon — either a
  // dotted-IPv4 (4 numeric octets) or something with at least one letter.
  // Without this, a ratio like "3.5:1" (2 numeric segments, no letters) was
  // misclassified as host:port and redacted.
  const preColonIsHostLike =
    preColon.includes('.') &&
    (preColonSegs.length === 4 && preColonSegs.every((s) => /^\d+$/.test(s))
      ? true
      : /[a-z]/i.test(preColon));
  const hostPort = colonIdx > 0 && preColonIsHostLike && /^\d+$/.test(trimmed.slice(colonIdx + 1));
  const isHostPort = trimmed.includes('.') && (dottedIpv4 || hostPort);

  const atIdx = trimmed.indexOf('@');
  const isEmail = atIdx > 0 && trimmed.slice(atIdx + 1).includes('.');

  let placeholder: string | null = null;
  if (isUrl) placeholder = '<url-redacted>';
  else if (isCredential) placeholder = '<credential-redacted>';
  else if (isWindowsPath || isUnixPath || isHomeish || isUncPath) placeholder = '<path-redacted>';
  else if (isHostPort) placeholder = '<host-redacted>';
  else if (isEmail) placeholder = '<email-redacted>';

  return placeholder ? token.replace(trimmed, placeholder) : token;
}

type ChipTone = 'success' | 'error' | 'skipped' | 'truncated' | 'note' | 'health';

/** Tone → `Tag` colour. Skipped is the neutral "needs configuration" tone;
 *  `note` uses the informational (blue) `processing` tone — distinct from the
 *  error/skip/partial tones — for a benign "how the search was adjusted" hint.
 *  `health` (Track B1's cross-run history) borrows the amber `warning` tone: it
 *  is not this run's failure — that already has its own red chip — but the
 *  standing "this source has been down for a while" context around it. */
const TONE_COLOR: Record<ChipTone, 'success' | 'error' | 'default' | 'warning' | 'processing'> = {
  success: 'success',
  error: 'error',
  skipped: 'default',
  truncated: 'warning',
  note: 'processing',
  health: 'warning',
};

interface Chip {
  key: string;
  tone: ChipTone;
  /** Empty for the collapsed "all ok" summary chip — rendered without the
   *  "board · detail" split. */
  board: string;
  detail: string;
  /** Native tooltip. Used by the Track B1 history chip to carry the remembered
   *  failure reason — the "why did this job source fail?" answer — without
   *  spending any of the chip's 60-char budget on it. Same self-describing
   *  `title` convention the badges use. */
  hint?: string;
}

/** Map a controlled `skipped` reason to a localized label (never a raw enum). */
function skipDetail(skipped: string, t: TFunction): string {
  switch (skipped) {
    case 'needs-login':
      return t('jobs.boardSummary.skip.needsLogin');
    case 'needs-company':
      return t('jobs.boardSummary.skip.needsCompany');
    case 'needs-keys':
      return t('jobs.boardSummary.skip.needsKeys');
    default:
      return t('jobs.boardSummary.skip.other');
  }
}

/**
 * Map a controlled location/work-type `note` token to a localized label — or
 * `null` for an unknown/future token so a legacy or newer backend can never
 * leak a raw machine token into the UI (tolerant, like the `skipped`
 * fallback). Two shapes:
 *   - `kind:<countryCode>` (lowercase alpha-2) — `broadened` / `guessed-market`;
 *     the country name is resolved natively.
 *   - `location-filtered:<n>` (PR F) / `work-type-filtered:<n>` (Phase 2b) —
 *     the engine emits these unconditionally for a board without server-side
 *     support for that filter (even n=0, "checked, nothing hidden"); n=0 gets
 *     a plain marker label, n>0 the pluralized hidden-count.
 *   - `slugs-invalid:<n>` / `rows-dropped:<n>` (PR H) / `companies-failed:<n>` —
 *     partial per-company visibility from the ATS boards; all emitted ONLY when
 *     n>0, so unlike `location-filtered`/`work-type-filtered` a zero/malformed
 *     n renders no chip.
 */
function noteDetail(note: string, t: TFunction, locale: string): string | null {
  const sep = note.indexOf(':');
  if (sep < 0) return null;
  const kind = note.slice(0, sep);
  const value = note.slice(sep + 1).trim();

  // `location-filtered:<n>` / `work-type-filtered:<n>` — count of results
  // dropped by the respective local post-filter (0 = the board was checked and
  // nothing was hidden). A malformed / negative / non-integer `n` is tolerated
  // (no chip), like an unknown token, so a legacy or garbled payload never
  // renders junk.
  if (kind === 'location-filtered' || kind === 'work-type-filtered') {
    // `Number('')` coerces to `0` in JS — reject empty explicitly so a bare
    // "location-filtered:" (no digits at all) stays malformed, not a false 0.
    if (!value) return null;
    const n = Number(value);
    if (!Number.isInteger(n) || n < 0) return null;
    if (kind === 'location-filtered') {
      return n === 0
        ? t('jobs.boardSummary.note.locationFilteredNone')
        : t('jobs.boardSummary.note.locationFiltered', { count: n });
    }
    return n === 0
      ? t('jobs.boardSummary.note.workTypeFilteredNone')
      : t('jobs.boardSummary.note.workTypeFiltered', { count: n });
  }

  // `slugs-invalid:<n>` / `rows-dropped:<n>` (PR H) / `companies-failed:<n>` —
  // partial per-company visibility from the ATS boards: `n` company slugs were
  // rejected by the pre-fetch SSRF/DNS-label validator, `n` rows failed to
  // deserialize on an otherwise-reached company, or `n` per-company fetches
  // failed (404/403/429/over-cap) while a sibling succeeded. All are emitted
  // ONLY when n>0 (an all-fail run surfaces as an error, never a note), so —
  // unlike location-filtered's n=0 marker — a zero / empty / non-integer /
  // negative n is malformed and renders no chip (tolerant, like an unknown
  // token).
  if (kind === 'slugs-invalid' || kind === 'rows-dropped' || kind === 'companies-failed') {
    if (!value) return null;
    const n = Number(value);
    if (!Number.isInteger(n) || n <= 0) return null;
    if (kind === 'slugs-invalid') return t('jobs.boardSummary.note.slugsInvalid', { count: n });
    if (kind === 'rows-dropped') return t('jobs.boardSummary.note.rowsDropped', { count: n });
    return t('jobs.boardSummary.note.companiesFailed', { count: n });
  }

  // `kind:<cc>` — alpha-2 only; a malformed multi-colon token (e.g.
  // "broadened:de:extra") must not render its trailing garbage as a "country".
  if (value.length !== 2) return null;
  const country = regionName(value, locale);
  switch (kind) {
    case 'broadened':
      return t('jobs.boardSummary.note.broadened', { country });
    case 'guessed-market':
      return t('jobs.boardSummary.note.guessed', { country });
    default:
      return null;
  }
}

/** A finite, positive epoch-ms timestamp, or `null` for anything else. */
function asTimestamp(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null;
}

/**
 * Track B1 — render a board's CROSS-RUN history as a second chip beside its
 * own result: "down for 4 runs · last worked 6 days ago". Rust only attaches
 * `health` to an unhealthy board, so a working board's strip is unchanged.
 *
 * Returns `null` for anything we should not claim: a healthy/unknown status, a
 * `failing` verdict with no actual failures behind it, or a malformed payload
 * (the object crosses IPC and may come from a persisted `lastRunSummaries`
 * record, so it is validated, not trusted).
 */
function healthDetail(
  health: BoardHealth,
  t: TFunction,
  locale: string,
  now: number
): string | null {
  if (!health || typeof health !== 'object') return null;
  const lastSuccess = asTimestamp(health.lastSuccessAt);

  if (health.status === 'stale') {
    return lastSuccess
      ? t('jobs.boardSummary.health.stale', { since: timeAgo(lastSuccess, now, locale) })
      : t('jobs.boardSummary.health.staleUnknown');
  }
  // Working right now, but failing a meaningful SHARE of the runs that reach it
  // — the state a consecutive-failure streak structurally cannot show, because
  // it resets on every good run in between.
  if (health.status === 'flaky') {
    const { failedRuns, verifiedRuns } = health;
    if (
      !Number.isInteger(failedRuns) ||
      !Number.isInteger(verifiedRuns) ||
      failedRuns < 1 ||
      verifiedRuns < failedRuns
    ) {
      return null;
    }
    return t('jobs.boardSummary.health.flaky', { count: failedRuns, total: verifiedRuns });
  }
  if (health.status !== 'failing') return null;

  const failures = health.consecutiveFailures;
  // A "failing" verdict with no counted failure is incoherent — say nothing
  // rather than render "down for 0 runs".
  if (!Number.isInteger(failures) || failures < 1) return null;
  if (lastSuccess) {
    return t('jobs.boardSummary.health.failingSince', {
      count: failures,
      since: timeAgo(lastSuccess, now, locale),
    });
  }
  // The distinct, load-bearing case: this source has NEVER worked on this
  // install — a different problem from one that broke last week, and the one a
  // "0 results" chip hides most completely.
  const opened = asTimestamp(health.failingSince);
  return opened
    ? t('jobs.boardSummary.health.neverWorkedSince', {
        since: timeAgo(opened, now, locale),
      })
    : t('jobs.boardSummary.health.neverWorked', { count: failures });
}

/**
 * Normalize + classify each summary defensively — the array crosses IPC and may
 * be a legacy/tampered persisted record, so unknown shapes are tolerated (a
 * non-object entry, a missing board id, a non-numeric count) rather than trusted.
 * Per-board precedence mirrors Rust `scrape_diagnostics`, with the informational
 * notes slotting below the failure/partial tones: error > skipped > truncated >
 * note > success. Up to three notes can legitimately coexist on one board (a
 * board-native note, `location-filtered`, `work-type-filtered`) — each renders
 * its OWN chip rather than one replacing another, mirroring the Rust
 * `BoardScrapeSummary.notes` array.
 */
function toChips(
  summaries: readonly BoardScrapeSummary[],
  t: TFunction,
  locale: string,
  now: number
): Chip[] {
  if (!Array.isArray(summaries)) return [];
  const chips: Chip[] = [];
  summaries.forEach((raw, i) => {
    if (!raw || typeof raw !== 'object') return;
    const s = raw as Partial<BoardScrapeSummary>;
    const boardId = typeof s.board === 'string' ? s.board.trim() : '';
    if (!boardId) return;
    const board = t(`jobs.boards.${boardId}`, { defaultValue: boardId });
    const error = typeof s.error === 'string' && s.error.trim() ? s.error : null;
    const skipped = typeof s.skipped === 'string' && s.skipped.trim() ? s.skipped : null;
    const truncated = typeof s.truncated === 'string' && s.truncated.trim() ? s.truncated : null;
    const notesRaw = Array.isArray(s.notes)
      ? s.notes.filter((n): n is string => typeof n === 'string' && n.trim().length > 0)
      : [];
    const noteDetails = notesRaw
      .map((n) => noteDetail(n, t, locale))
      .filter((d): d is string => d !== null);
    const count = typeof s.count === 'number' && Number.isFinite(s.count) ? s.count : 0;

    if (error) {
      chips.push({
        key: `${boardId}-${i}`,
        tone: 'error',
        board,
        detail: capDetail(sanitizeReason(error)),
      });
    } else if (skipped) {
      chips.push({
        key: `${boardId}-${i}`,
        tone: 'skipped',
        board,
        detail: capDetail(skipDetail(skipped, t)),
      });
    } else if (truncated) {
      chips.push({
        key: `${boardId}-${i}`,
        tone: 'truncated',
        board,
        detail: capDetail(t('jobs.boardSummary.partial')),
      });
    } else if (noteDetails.length > 0) {
      // One chip per applicable note — never collapse several independent
      // honesty signals into one, which is exactly the bug the Rust-side
      // `notes: Vec<String>` widening fixed.
      noteDetails.forEach((detail, ni) => {
        chips.push({
          key: `${boardId}-${i}-note-${ni}`,
          tone: 'note',
          board,
          detail: capDetail(detail),
        });
      });
    } else {
      chips.push({
        key: `${boardId}-${i}`,
        tone: 'success',
        board,
        detail: capDetail(t('jobs.boardSummary.count', { count })),
      });
    }

    // Track B1 — the cross-run history rides alongside, never replaces, this
    // run's own chip: "wwr · HTTP 500" then "wwr · down for 4 runs · last
    // worked 6 days ago". Rust attaches `health` only when the board is failing
    // or stale, so a working board's strip is byte-identical to before.
    const history = s.health ? healthDetail(s.health, t, locale, now) : null;
    if (history) {
      // The remembered reason answers "why did this source fail?" for a board
      // whose CURRENT chip can't (it was skipped, so this run has no error of
      // its own). Sanitized like every other persisted reason — it crossed IPC
      // out of a store and is not trusted.
      const remembered =
        typeof s.health?.lastError === 'string' && s.health.lastError.trim()
          ? sanitizeReason(s.health.lastError)
          : undefined;
      chips.push({
        key: `${boardId}-${i}-health`,
        tone: 'health',
        board,
        detail: capDetail(history),
        hint: remembered,
      });
    }
  });
  return chips;
}

export interface BoardSummaryChipsProps {
  summaries: readonly BoardScrapeSummary[];
  className?: string;
  /** Injectable clock for the "last worked N ago" history chips — tests pin it
   *  so the rendered string is an absolute expectation, not a moving one. */
  now?: number;
}

export function BoardSummaryChips({ summaries, className, now }: BoardSummaryChipsProps) {
  const { t, i18n } = useTranslation();
  const boardChips = toChips(summaries, t, i18n.language, now ?? Date.now());
  if (boardChips.length === 0) return null;

  // Noise reduction: a fully clean run (every board succeeded) collapses to one
  // chip instead of a green chip per board.
  const allOk = boardChips.length > 1 && boardChips.every((c) => c.tone === 'success');
  const chips: Chip[] = allOk
    ? [
        {
          key: 'all-ok',
          tone: 'success',
          board: '',
          detail: t('jobs.boardSummary.allOk', { count: boardChips.length }),
        },
      ]
    : boardChips;

  return (
    <div
      role="group"
      aria-label={t('jobs.boardSummary.label')}
      className={cn('flex flex-wrap items-center gap-1.5', className)}
    >
      {chips.map((c) => (
        <Tag
          key={c.key}
          color={TONE_COLOR[c.tone]}
          className="max-w-[220px] whitespace-normal break-words text-[10px] font-normal"
        >
          {c.board ? (
            <span title={c.hint}>
              <span className="font-semibold">{c.board}</span>
              {' · '}
              {c.detail}
            </span>
          ) : (
            c.detail
          )}
        </Tag>
      ))}
    </div>
  );
}
