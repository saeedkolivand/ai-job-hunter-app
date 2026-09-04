import { Check, Loader2, Sparkles, X } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Input, Tag, transition, useFocusTrap } from '@ajh/ui';

import { resolveRewriteTimeoutMs, type RewriteDocType, rewriteSelection } from '@/lib/generate';
// The pure limit/unchanged helpers come from their own module rather than the
// `@/lib/generate` barrel, so a test that stubs the barrel's IPC-backed exports
// still runs the real parser and the real unchanged predicate.
import {
  buildOvershootInstruction,
  exceedsRewriteLimit,
  isUnchangedRewrite,
  measureRewriteLength,
  parseRewriteLimit,
  type RewriteLimit,
  type RewriteLimitUnit,
} from '@/lib/generate/rewrite';

/** The quick-action presets — id maps to an i18n label + a preset instruction. */
const PRESETS = ['shorten', 'expand', 'rephrase', 'impact', 'grammar'] as const;

/** How long a rewrite may stream before the popover says it is still working.
 *  A default-effort rewrite of a long span measured 7-34 s typically, with
 *  individual streams of 91-152 s — long enough to look dead without a line. */
const STILL_WORKING_MS = 20_000;
/** Matches the panel's `w-[22rem]` — clamps left-edge overflow when the trigger
 *  sits near a viewport edge (anchored-portal mode only). */
const POPOVER_WIDTH_PX = 352;
type Preset = (typeof PRESETS)[number];

export interface RewriteTarget {
  /** The frozen selection text being rewritten. */
  selection: string;
  /** Frozen text before the selection (context, never rewritten). */
  before: string;
  /** Frozen text after the selection (context, never rewritten). */
  after: string;
}

interface RewritePopoverProps {
  target: RewriteTarget;
  docType: RewriteDocType;
  model: string;
  /** FALLBACK document language (the generation's `meta.targetLanguage`), used
   *  only when the SELECTION's own language cannot be detected — the rewrite
   *  streams in the span's language, which is not always the document's
   *  (`rewriteSelection` → `deriveRewriteLocale`). Defaults to 'en'. */
  locale?: string;
  /** Called with the accepted replacement text for the frozen range. */
  onAccept: (replacement: string) => void;
  /** Called to dismiss the popover (Cancel / Escape). This component renders no
   *  backdrop of its own in either mode; a caller that wants outside-click
   *  dismissal renders its own click-catcher and calls this (`EditableOutput`
   *  does — its catcher sits under the portaled panel's `z-toast`). */
  onClose: () => void;
  /**
   * When set, the popover portals to `document.body` and fixed-positions itself
   * below-right of this trigger element instead of rendering inline. Use when the
   * inline placement would be clipped by an `overflow-hidden`/`overflow-y-auto`
   * ancestor (e.g. inside a `ModalShell`) or needs to sit above a modal's z-index.
   */
  anchorEl?: HTMLElement | null;
}

/**
 * Floating rewrite popover (F4). Streams an AI rewrite of the frozen selection
 * into its own preview (never the textarea), and on Accept hands the result back
 * to the caller to splice into the raw text. A single rewrite is in flight at a
 * time — starting a new one (preset, submit, or regenerate) aborts the previous
 * via an AbortController. Modal + keyboard accessible: it traps focus while open
 * (reusing `useFocusTrap` from @ajh/ui — the same mechanism ModalShell uses),
 * autofocuses the instruction field, closes on Escape from anywhere, restores
 * focus to the trigger on close (handled by the caller), and the actions are
 * real buttons.
 */
export function RewritePopover({
  target,
  docType,
  model,
  locale = 'en',
  onAccept,
  onClose,
  anchorEl,
}: RewritePopoverProps) {
  const { t } = useTranslation();
  const [instruction, setInstruction] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [result, setResult] = useState('');
  const [error, setError] = useState<string | null>(null);
  /** The finished result came back identical to the selection (F3): a no-op,
   *  not an error and not something to Accept. */
  const [unchanged, setUnchanged] = useState(false);
  /** Set when a parsed numeric limit is STILL exceeded after the one re-ask —
   *  the count is shown next to Accept and the user decides. */
  const [overLimit, setOverLimit] = useState<{
    n: number;
    limit: number;
    unit: RewriteLimitUnit;
  } | null>(null);
  /** A long reasoning pass has been streaming for {@link STILL_WORKING_MS}. */
  const [stillWorking, setStillWorking] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  // Trap keyboard focus inside the popover while it is open so Tab cannot escape
  // into the page behind it. `useFocusTrap` also auto-focuses the first focusable
  // element; we still explicitly focus the instruction field below.
  const trapRef = useFocusTrap(true);
  // A dedicated, locally-owned ref to the panel element (separate from
  // `trapRef`) so measuring its height doesn't depend on `useFocusTrap`'s
  // returned ref identity — merged onto the same node via the callback ref below.
  const panelRef = useRef<HTMLDivElement | null>(null);
  const [anchorRect, setAnchorRect] = useState<DOMRect | null>(null);
  // The popover's own rendered height — it varies with content (streaming
  // result, error text, …), so it can't be assumed static. Used to decide
  // whether the panel fits below the trigger or must flip above it.
  const [panelHeight, setPanelHeight] = useState<number | null>(null);

  // Anchored-portal mode only: measure the trigger on open, and re-measure on
  // scroll/resize — the caller's scrollable ancestor (e.g. a modal body) can move
  // the trigger while this fixed-positioned popover stays put otherwise. Also
  // track the panel's own height via ResizeObserver so a later content change
  // (e.g. the streaming result appearing) can re-trigger the fit check below.
  useLayoutEffect(() => {
    if (!anchorEl) return;
    const measureAnchor = () => setAnchorRect(anchorEl.getBoundingClientRect());
    measureAnchor();
    window.addEventListener('scroll', measureAnchor, true);
    window.addEventListener('resize', measureAnchor);

    const panel = panelRef.current;
    let observer: ResizeObserver | undefined;
    if (panel) {
      setPanelHeight(panel.getBoundingClientRect().height);
      observer = new ResizeObserver(() => setPanelHeight(panel.getBoundingClientRect().height));
      observer.observe(panel);
    }

    return () => {
      window.removeEventListener('scroll', measureAnchor, true);
      window.removeEventListener('resize', measureAnchor);
      observer?.disconnect();
    };
  }, [anchorEl]);

  // The instruction that produced the current result — lets Regenerate re-run the
  // same instruction without the user retyping it.
  const lastInstructionRef = useRef('');

  useEffect(() => {
    inputRef.current?.focus();
    // Abort any in-flight rewrite when the popover unmounts.
    return () => abortRef.current?.abort();
  }, []);

  // Escape closes from anywhere while the popover is open — not only when focus is
  // inside it (a bare onKeyDown on the dialog misses clicks/focus elsewhere).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  /**
   * Land a finished attempt: the result is shown either way, but Accept is only
   * offered for a rewrite that actually changed something. An over-limit count
   * is advisory (the user decides) — it never blocks Accept.
   */
  const settle = (cleaned: string, limit: RewriteLimit | null) => {
    setResult(cleaned);
    if (!cleaned) {
      setError(t('aiGenerate.rewrite.empty'));
      return;
    }
    if (isUnchangedRewrite(target.selection, cleaned)) {
      setUnchanged(true);
      return;
    }
    if (limit && exceedsRewriteLimit(cleaned, limit)) {
      setOverLimit({
        n: measureRewriteLength(cleaned, limit.unit),
        limit: limit.max,
        unit: limit.unit,
      });
    }
  };

  const run = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || streaming) return;
    // Single in-flight rewrite — abort the previous before starting a new one.
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    lastInstructionRef.current = trimmed;
    setError(null);
    setResult('');
    setUnchanged(false);
    setOverLimit(null);
    setStillWorking(false);
    setStreaming(true);

    // Client-side safety net: abort a stalled provider at the SAME effort-scaled
    // bound the shared stream helper uses for this request, so the renderer can
    // no longer kill a generation the backend is still legitimately streaming
    // (the previous fixed 60 s sat BELOW the backend deadline and did fire on a
    // long span). `timedOut` distinguishes this from a user-initiated abort
    // (close/unmount/new run) so the catch block surfaces the error instead of
    // silently swallowing it.
    let timedOut = false;
    // `let`, not `const`: the one re-ask below re-arms this with its OWN fresh
    // window (see there) rather than sharing what's left of the first attempt's.
    const armTimeout = () =>
      setTimeout(() => {
        timedOut = true;
        controller.abort();
      }, resolveRewriteTimeoutMs(model));
    let timeoutId = armTimeout();
    // A reasoning pass on a long span streams for a minute or more with nothing
    // on screen; say so rather than looking dead.
    const stillWorkingId = setTimeout(() => setStillWorking(true), STILL_WORKING_MS);

    // A numeric length limit in the instruction is verified by CODE, never
    // trusted from the model (measured: "under 200 characters" landed over the
    // line about half the time, "at most 40 words" missed every run).
    const limit = parseRewriteLimit(trimmed);

    const attempt = (instructionText: string) => {
      setResult('');
      return rewriteSelection({
        selection: target.selection,
        instruction: instructionText,
        before: target.before,
        after: target.after,
        docType,
        model,
        locale,
        onToken: (tok) => setResult((prev) => prev + tok),
        signal: controller.signal,
      }).then((full) => full.trim());
    };

    attempt(trimmed)
      .then(async (first) => {
        if (controller.signal.aborted) return;
        if (!limit || !exceedsRewriteLimit(first, limit)) {
          settle(first, limit);
          return;
        }
        // Exactly ONE re-ask, carrying the measured overshoot — never a loop.
        // Re-arm the timeout with its OWN full window: the first attempt may
        // have already burned most of the shared budget (a single stream
        // measured up to ~152 s), and without this the retry could be aborted
        // almost immediately — discarding the first attempt's perfectly
        // usable (if over-limit) draft still sitting in `first` for a hard
        // error instead of the honest over-limit-Accept path below.
        clearTimeout(timeoutId);
        timeoutId = armTimeout();
        const retry = await attempt(
          buildOvershootInstruction(trimmed, limit, measureRewriteLength(first, limit.unit))
        );
        if (controller.signal.aborted) return;
        settle(retry, limit);
      })
      .catch(() => {
        // Suppress error for user-initiated abort (close / unmount / new run).
        // Always surface error for a timeout-triggered abort.
        if (controller.signal.aborted && !timedOut) return;
        setError(t('aiGenerate.rewrite.failed'));
      })
      .finally(() => {
        clearTimeout(timeoutId);
        clearTimeout(stillWorkingId);
        // Only clear `streaming` for the run that owns the current controller. A
        // newer run() has already set `streaming = true` and swapped `abortRef`;
        // clearing unconditionally here would re-enable the buttons mid-flight.
        // Guarding on the controller (rather than `aborted`) also re-enables the
        // buttons after an abort that wasn't followed by a new run — so a cancelled
        // rewrite never wedges the UI with permanently disabled buttons.
        if (abortRef.current === controller) {
          setStreaming(false);
          setStillWorking(false);
        }
      });
  };

  const onPreset = (preset: Preset) => {
    const presetInstruction = t(`aiGenerate.rewrite.presetInstructions.${preset}`);
    setInstruction(presetInstruction);
    run(presetInstruction);
  };

  // A result identical to the selection is a no-op, so there is nothing to
  // accept — Regenerate stays live. An over-limit result IS acceptable (the
  // count next to Accept is the honest part; the user decides).
  const canAccept = !streaming && !!result.trim() && !error && !unchanged;

  // Anchored-portal mode: fixed-position below-right of the trigger, clamped so
  // the (fixed-width) panel never runs off the left edge, and flipped ABOVE the
  // trigger when it wouldn't fit below (e.g. Rewrite opened on a question near
  // the bottom of a scrollable, height-capped modal) — clamped so it also never
  // runs off the top edge. `visibility: hidden` (not `display: none`) until the
  // first measurement lands keeps the panel laid out (so its real height can be
  // read) without a visible flash — it's gone by paint since `useLayoutEffect`
  // flushes `setAnchorRect`/`setPanelHeight` before the browser paints.
  const anchoredStyle: React.CSSProperties | undefined = anchorEl
    ? anchorRect
      ? {
          position: 'fixed',
          top:
            panelHeight !== null && anchorRect.bottom + panelHeight + 4 > window.innerHeight - 8
              ? Math.max(8, anchorRect.top - panelHeight - 4)
              : anchorRect.bottom + 4,
          left: Math.min(
            Math.max(8, anchorRect.right - POPOVER_WIDTH_PX),
            window.innerWidth - POPOVER_WIDTH_PX - 8
          ),
        }
      : { position: 'fixed', top: 0, left: 0, visibility: 'hidden' }
    : undefined;

  const popover = (
    <motion.div
      ref={(node: HTMLDivElement | null) => {
        trapRef.current = node;
        panelRef.current = node;
      }}
      role="dialog"
      aria-modal="true"
      aria-label={t('aiGenerate.rewrite.title')}
      initial={{ opacity: 0, y: 6, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 6, scale: 0.98 }}
      transition={transition.fast}
      style={anchoredStyle}
      className={cn(
        'w-[22rem] max-w-[calc(100vw-2rem)] overflow-hidden rounded-xl border border-[var(--border-clear)] bg-secondary shadow-2xl',
        anchorEl && 'z-toast'
      )}
    >
      <div className="flex items-center justify-between border-b border-[var(--border-clear)] px-3 py-2">
        <span className="flex items-center gap-1.5 text-[11px] font-medium text-foreground/70">
          <Sparkles size={12} className="text-brand-soft" />
          {t('aiGenerate.rewrite.title')}
        </span>
        <Button
          variant="unstyled"
          type="button"
          onClick={onClose}
          aria-label={t('aiGenerate.rewrite.cancel')}
          className="rounded p-0.5 text-foreground/40 transition-colors hover:text-foreground/80"
        >
          <X size={13} />
        </Button>
      </div>

      <div className="space-y-2.5 px-3 py-2.5">
        {/* Selected text — read-only echo so the user knows what will change. */}
        <div>
          <p className="mb-1 text-[9px] font-semibold uppercase tracking-wider text-foreground/35">
            {t('aiGenerate.rewrite.selectionLabel')}
          </p>
          <p className="max-h-16 overflow-y-auto whitespace-pre-wrap rounded-md bg-muted px-2 py-1.5 text-[11px] leading-relaxed text-foreground/55">
            {target.selection}
          </p>
        </div>

        {/* Quick-action chips */}
        <div className="flex flex-wrap gap-1">
          {PRESETS.map((preset) => (
            <Tag.CheckableTag
              key={preset}
              checked={false}
              disabled={streaming}
              onChange={() => onPreset(preset)}
            >
              {t(`aiGenerate.rewrite.presets.${preset}`)}
            </Tag.CheckableTag>
          ))}
        </div>

        {/* Free instruction + submit */}
        <div className="flex items-center gap-1.5">
          <Input
            ref={inputRef}
            value={instruction}
            onChange={(e) => setInstruction(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                run(instruction);
              }
            }}
            disabled={streaming}
            placeholder={t('aiGenerate.rewrite.instructionPlaceholder')}
            aria-label={t('aiGenerate.rewrite.instructionLabel')}
            className="flex-1 text-[11px]"
          />
          <Button
            type="button"
            disabled={streaming || !instruction.trim()}
            onClick={() => run(instruction)}
            className="flex h-auto items-center gap-1 rounded-lg bg-brand/15 px-2.5 py-1.5 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/20 disabled:opacity-40 disabled:pointer-events-none"
          >
            {streaming ? <Loader2 size={11} className="animate-spin" /> : null}
            {t('aiGenerate.rewrite.submit')}
          </Button>
        </div>

        {/* Streaming preview / result */}
        {(streaming || result || error) && (
          <div>
            <p className="mb-1 flex items-center gap-1 text-[9px] font-semibold uppercase tracking-wider text-foreground/35">
              {streaming && <Loader2 size={9} className="animate-spin" />}
              {streaming ? t('aiGenerate.rewrite.streaming') : t('aiGenerate.rewrite.resultLabel')}
            </p>
            {error ? (
              <p className="rounded-md bg-red-400/10 px-2 py-1.5 text-[11px] text-red-300">
                {error}
              </p>
            ) : (
              <p className="max-h-32 overflow-y-auto whitespace-pre-wrap rounded-md border border-brand/15 bg-brand/[0.04] px-2 py-1.5 text-[11px] leading-relaxed text-foreground/80">
                {result || '…'}
              </p>
            )}
            {/* A long reasoning pass looks dead without this line. */}
            {streaming && stillWorking && (
              <p
                role="status"
                aria-live="polite"
                className="mt-1 text-[10px] italic text-foreground/45"
              >
                {t('aiGenerate.rewrite.stillWorking')}
              </p>
            )}
            {/* Neutral, NOT an error: the model handed the selection back. */}
            {unchanged && !streaming && (
              <p
                role="status"
                aria-live="polite"
                className="mt-1 rounded-md bg-muted px-2 py-1.5 text-[11px] text-foreground/60"
              >
                {t('aiGenerate.rewrite.unchanged')}
              </p>
            )}
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center justify-end gap-2 border-t border-[var(--border-clear)] px-3 py-2">
        {/* Still over the parsed limit after the one re-ask — the honest count
            sits next to Accept and the user decides. */}
        {overLimit && !streaming && (
          <span role="status" aria-live="polite" className="mr-auto text-[10px] text-foreground/50">
            {t(`aiGenerate.rewrite.overLimit.${overLimit.unit}`, {
              n: overLimit.n,
              limit: overLimit.limit,
            })}
          </span>
        )}
        <Button
          variant="unstyled"
          type="button"
          onClick={onClose}
          className="rounded-lg px-2 py-1 text-[11px] text-foreground/50 transition-colors hover:text-foreground/80"
        >
          {t('aiGenerate.rewrite.cancel')}
        </Button>
        {result && !streaming && (
          <Button
            type="button"
            onClick={() => run(lastInstructionRef.current)}
            className="rounded-lg border-transparent bg-muted px-2.5 py-1 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto"
          >
            {t('aiGenerate.rewrite.regenerate')}
          </Button>
        )}
        <Button
          type="button"
          disabled={!canAccept}
          onClick={() => onAccept(result.trim())}
          className="flex h-auto items-center gap-1 rounded-lg bg-brand/15 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/20 disabled:opacity-40 disabled:pointer-events-none"
        >
          <Check size={11} />
          {t('aiGenerate.rewrite.accept')}
        </Button>
      </div>
    </motion.div>
  );

  return anchorEl ? createPortal(popover, document.body) : popover;
}
