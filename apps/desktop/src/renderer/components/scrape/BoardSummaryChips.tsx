import type { BoardScrapeSummary } from '@ajh/shared';
import { type TFunction, useTranslation } from '@ajh/translations';
import { cn, Tag } from '@ajh/ui';

import { regionName } from '@/lib/region-name';

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

type ChipTone = 'success' | 'error' | 'skipped' | 'truncated' | 'note';

/** Tone → `Tag` colour. Skipped is the neutral "needs configuration" tone;
 *  `note` uses the informational (blue) `processing` tone — distinct from the
 *  error/skip/partial tones — for a benign "how the search was adjusted" hint. */
const TONE_COLOR: Record<ChipTone, 'success' | 'error' | 'default' | 'warning' | 'processing'> = {
  success: 'success',
  error: 'error',
  skipped: 'default',
  truncated: 'warning',
  note: 'processing',
};

interface Chip {
  key: string;
  tone: ChipTone;
  /** Empty for the collapsed "all ok" summary chip — rendered without the
   *  "board · detail" split. */
  board: string;
  detail: string;
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
 * Map a controlled location `note` token to a localized label — or `null` for an
 * unknown/future token so a legacy or newer backend can never leak a raw machine
 * token into the UI (tolerant, like the `skipped` fallback). Shape is
 * `kind:<countryCode>` (lowercase cc); the country name is resolved natively.
 */
function noteDetail(note: string, t: TFunction, locale: string): string | null {
  const sep = note.indexOf(':');
  if (sep < 0) return null;
  const kind = note.slice(0, sep);
  const cc = note.slice(sep + 1).trim();
  // Alpha-2 only — a malformed multi-colon token (e.g. "broadened:de:extra")
  // must not render its trailing garbage as a "country".
  if (cc.length !== 2) return null;
  const country = regionName(cc, locale);
  switch (kind) {
    case 'broadened':
      return t('jobs.boardSummary.note.broadened', { country });
    case 'guessed-market':
      return t('jobs.boardSummary.note.guessed', { country });
    default:
      return null;
  }
}

/**
 * Normalize + classify each summary defensively — the array crosses IPC and may
 * be a legacy/tampered persisted record, so unknown shapes are tolerated (a
 * non-object entry, a missing board id, a non-numeric count) rather than trusted.
 * Per-board precedence mirrors Rust `scrape_diagnostics`, with the informational
 * location note slotting below the failure/partial tones: error > skipped >
 * truncated > note > success.
 */
function toChips(summaries: readonly BoardScrapeSummary[], t: TFunction, locale: string): Chip[] {
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
    const noteRaw = typeof s.note === 'string' && s.note.trim() ? s.note : null;
    const note = noteRaw ? noteDetail(noteRaw, t, locale) : null;
    const count = typeof s.count === 'number' && Number.isFinite(s.count) ? s.count : 0;

    let tone: ChipTone;
    let detail: string;
    if (error) {
      tone = 'error';
      detail = sanitizeReason(error);
    } else if (skipped) {
      tone = 'skipped';
      detail = skipDetail(skipped, t);
    } else if (truncated) {
      tone = 'truncated';
      detail = t('jobs.boardSummary.partial');
    } else if (note) {
      tone = 'note';
      detail = note;
    } else {
      tone = 'success';
      detail = t('jobs.boardSummary.count', { count });
    }
    chips.push({ key: `${boardId}-${i}`, tone, board, detail: capDetail(detail) });
  });
  return chips;
}

export interface BoardSummaryChipsProps {
  summaries: readonly BoardScrapeSummary[];
  className?: string;
}

export function BoardSummaryChips({ summaries, className }: BoardSummaryChipsProps) {
  const { t, i18n } = useTranslation();
  const boardChips = toChips(summaries, t, i18n.language);
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
            <>
              <span className="font-semibold">{c.board}</span>
              <span>· {c.detail}</span>
            </>
          ) : (
            c.detail
          )}
        </Tag>
      ))}
    </div>
  );
}
