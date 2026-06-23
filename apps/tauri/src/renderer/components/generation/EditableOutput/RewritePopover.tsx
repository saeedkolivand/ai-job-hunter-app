import { Check, Loader2, Sparkles, X } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, Tag, transition, useFocusTrap } from '@ajh/ui';

import { type RewriteDocType, rewriteSelection } from '@/lib/generate';

/** The quick-action presets — id maps to an i18n label + a preset instruction. */
const PRESETS = ['shorten', 'expand', 'rephrase', 'impact', 'grammar'] as const;
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
  /** Document language (the generation's `meta.targetLanguage`) so the rewrite
   *  streams in the same language as the document. Defaults to 'en'. */
  locale?: string;
  /** Called with the accepted replacement text for the frozen range. */
  onAccept: (replacement: string) => void;
  /** Called to dismiss the popover (Cancel / Escape / backdrop). */
  onClose: () => void;
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
}: RewritePopoverProps) {
  const { t } = useTranslation();
  const [instruction, setInstruction] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [result, setResult] = useState('');
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  // Trap keyboard focus inside the popover while it is open so Tab cannot escape
  // into the page behind it. `useFocusTrap` also auto-focuses the first focusable
  // element; we still explicitly focus the instruction field below.
  const trapRef = useFocusTrap(true);

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
    setStreaming(true);

    rewriteSelection({
      selection: target.selection,
      instruction: trimmed,
      before: target.before,
      after: target.after,
      docType,
      model,
      locale,
      onToken: (tok) => setResult((prev) => prev + tok),
      signal: controller.signal,
    })
      .then((full) => {
        if (controller.signal.aborted) return;
        const cleaned = full.trim();
        setResult(cleaned);
        if (!cleaned) setError(t('aiGenerate.rewrite.empty'));
      })
      .catch(() => {
        if (controller.signal.aborted) return;
        setError(t('aiGenerate.rewrite.failed'));
      })
      .finally(() => {
        // Only clear `streaming` for the run that owns the current controller. A
        // newer run() has already set `streaming = true` and swapped `abortRef`;
        // clearing unconditionally here would re-enable the buttons mid-flight.
        // Guarding on the controller (rather than `aborted`) also re-enables the
        // buttons after an abort that wasn't followed by a new run — so a cancelled
        // rewrite never wedges the UI with permanently disabled buttons.
        if (abortRef.current === controller) setStreaming(false);
      });
  };

  const onPreset = (preset: Preset) => {
    const presetInstruction = t(`aiGenerate.rewrite.presetInstructions.${preset}`);
    setInstruction(presetInstruction);
    run(presetInstruction);
  };

  const canAccept = !streaming && !!result.trim() && !error;

  return (
    <motion.div
      ref={trapRef as React.RefObject<HTMLDivElement>}
      role="dialog"
      aria-modal="true"
      aria-label={t('aiGenerate.rewrite.title')}
      initial={{ opacity: 0, y: 6, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 6, scale: 0.98 }}
      transition={transition.fast}
      className="w-[22rem] max-w-[calc(100vw-2rem)] overflow-hidden rounded-xl border border-[var(--border-clear)] bg-secondary shadow-2xl"
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
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center justify-end gap-2 border-t border-[var(--border-clear)] px-3 py-2">
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
}
