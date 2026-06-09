import { Eye, Pencil, Save, Sparkles } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useCallback, useRef, useState } from 'react';

import { Button, MarkdownMessage, SegmentedControl, TextArea } from '@ajh/ui';

import { useSelectedModel } from '@/components/ui/ModelSelector';
import type { GenerationMeta, RewriteDocType } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { RewritePopover, type RewriteTarget } from './RewritePopover';

interface EditableOutputProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  /** Document kind — drives the rewrite prompt's framing. */
  docType: RewriteDocType;
  /** Detected metadata — `targetLanguage` drives the rewrite locale (default 'en'). */
  meta?: GenerationMeta | null;
  /** Model override; defaults to the active provider's selected model. */
  model?: string;
  /** Enable the F4 inline-rewrite popover. Default true. */
  enableRewrite?: boolean;
  className?: string;
  textAreaClassName?: string;
  placeholder?: string;
  /**
   * Optional custom Preview surface (#24). When provided, the Preview tab renders
   * this instead of the prettified-markdown view — e.g. the real-PDF `PdfPreview`.
   * The caller owns its lifecycle (it reads the same canonical text). Edit mode is
   * unchanged. Consumers that omit it keep the markdown preview.
   */
  previewSlot?: React.ReactNode;
  /**
   * Optional explicit-save handler. When provided, a **Save** button appears in the
   * edit-view toolbar; clicking it commits the current edits (e.g. so a heavy
   * `previewSlot` recompiles only on Save instead of on every keystroke). When
   * omitted, behavior is unchanged — editing flows straight through `onChange`.
   */
  onSave?: () => void;
  /** Enables the Save button — true when there are unsaved edits. */
  canSave?: boolean;
}

/** The frozen selection range + text snapshot captured when a rewrite starts. */
interface FrozenRange {
  start: number;
  end: number;
  /** The exact text value at freeze time — guards the splice against drift. */
  snapshot: string;
  target: RewriteTarget;
}

/**
 * Shared Preview | Edit surface for generated documents.
 *
 * - **Preview** renders prettified markdown (`MarkdownMessage`).
 * - **Edit** is a raw `TextArea`; the raw string stays canonical, so copy/export
 *   read exactly what the user typed.
 * - **F4 inline rewrite** (when `enableRewrite`): selecting text in Edit reveals a
 *   "Rewrite with AI" trigger. Triggering freezes the selection range + a text
 *   snapshot, locks the textarea while a single rewrite streams into the popover
 *   preview (never the textarea), and on Accept splices the result into the raw
 *   text at the frozen range via `onChange`.
 *
 * The popover is anchored to a toolbar above the editor rather than the caret — a
 * `<textarea>` exposes no per-character pixel coordinates, so caret-pixel math is
 * deliberately avoided.
 */
export function EditableOutput({
  value,
  onChange,
  disabled = false,
  docType,
  meta,
  model,
  enableRewrite = true,
  className,
  textAreaClassName,
  placeholder,
  previewSlot,
  onSave,
  canSave = false,
}: EditableOutputProps) {
  const { t } = useTranslation();
  const selectedModel = useSelectedModel();
  const effectiveModel = model ?? selectedModel;
  // Rewrite in the document's language (falls back to English) so the AI reply
  // matches the document instead of forcing English.
  const rewriteLocale = meta?.targetLanguage ?? 'en';

  const [view, setView] = useState<'preview' | 'edit'>('preview');
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  // Live selection (range only) used to show/hide the rewrite trigger.
  const [hasSelection, setHasSelection] = useState(false);
  // Frozen range while a rewrite popover is open — null when closed.
  const [frozen, setFrozen] = useState<FrozenRange | null>(null);

  const updateSelection = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    setHasSelection(el.selectionStart !== el.selectionEnd);
  }, []);

  const openRewrite = () => {
    const el = textareaRef.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    if (start === end) return;
    const selection = value.slice(start, end);
    setFrozen({
      start,
      end,
      snapshot: value,
      target: {
        selection,
        before: value.slice(0, start),
        after: value.slice(end),
      },
    });
  };

  const closeRewrite = () => {
    setFrozen(null);
    // Return focus to the editor for keyboard continuity.
    textareaRef.current?.focus();
  };

  const acceptRewrite = (replacement: string) => {
    if (!frozen) return;
    // Splice into the snapshot at the frozen range — the textarea was locked while
    // streaming, so `value` cannot have drifted, but splice against the snapshot
    // for safety.
    const base = frozen.snapshot;
    const next = base.slice(0, frozen.start) + replacement + base.slice(frozen.end);
    onChange(next);
    setFrozen(null);
    // Restore focus + place the caret after the inserted text.
    const caret = frozen.start + replacement.length;
    requestAnimationFrame(() => {
      const el = textareaRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(caret, caret);
      setHasSelection(false);
    });
  };

  const rewriteVisible = enableRewrite && view === 'edit';

  return (
    <div className={className}>
      <div className="mb-2 flex shrink-0 items-center justify-between gap-2">
        {/* Rewrite trigger — left, only in edit mode with a live selection. */}
        <div className="flex items-center">
          {rewriteVisible && hasSelection && !frozen && (
            <Button
              type="button"
              onClick={openRewrite}
              className="flex h-auto items-center gap-1.5 rounded-lg bg-brand/15 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/20"
            >
              <Sparkles size={11} />
              {t('aiGenerate.rewrite.trigger')}
            </Button>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {/* Save — only in edit mode when a save handler is supplied; commits the
              current edits (e.g. so the preview recompiles on Save, not per keystroke). */}
          {onSave && view === 'edit' && (
            <Button
              type="button"
              onClick={onSave}
              disabled={disabled || !canSave}
              className="flex h-auto items-center gap-1.5 rounded-lg bg-brand/15 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/20 disabled:pointer-events-none disabled:opacity-30"
            >
              <Save size={11} />
              {t('aiGenerate.save')}
            </Button>
          )}

          <SegmentedControl<'preview' | 'edit'>
            ariaLabel={t('aiGenerate.viewMode')}
            size="sm"
            tone="brand"
            value={view}
            onChange={setView}
            options={[
              { value: 'preview', label: t('aiGenerate.preview'), icon: Eye },
              { value: 'edit', label: t('aiGenerate.edit'), icon: Pencil },
            ]}
            className="shrink-0"
          />
        </div>
      </div>

      {view === 'edit' ? (
        <div className="relative h-full w-full">
          <TextArea
            ref={textareaRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onSelect={updateSelection}
            onMouseUp={updateSelection}
            onKeyUp={updateSelection}
            // Lock with readOnly (not disabled) while a rewrite streams so the
            // textarea stays focusable/readable for screen readers; `disabled`
            // reflects only the outer prop.
            disabled={disabled}
            readOnly={frozen !== null}
            className={
              textAreaClassName ??
              'h-full w-full bg-transparent font-mono text-[12px] leading-relaxed text-foreground/80 placeholder:text-foreground/20'
            }
            spellCheck={false}
            placeholder={placeholder ?? t('aiGenerate.placeholder')}
          />

          {/* Rewrite popover — anchored to a toolbar position above the editor
              (textarea has no per-char coords, so no caret-pixel math). */}
          <AnimatePresence>
            {frozen && (
              <>
                <div className="fixed inset-0 z-[680]" onClick={closeRewrite} aria-hidden="true" />
                <div className="absolute right-0 top-0 z-[700]">
                  <RewritePopover
                    target={frozen.target}
                    docType={docType}
                    model={effectiveModel}
                    locale={rewriteLocale}
                    onAccept={acceptRewrite}
                    onClose={closeRewrite}
                  />
                </div>
              </>
            )}
          </AnimatePresence>
        </div>
      ) : previewSlot ? (
        // #24 — caller-supplied Preview (e.g. the real-PDF view) replaces markdown.
        <div className="h-full w-full overflow-hidden rounded-lg">{previewSlot}</div>
      ) : (
        <div className="h-full w-full overflow-y-auto rounded-lg">
          {value ? (
            <MarkdownMessage content={value} className="text-[12px] text-foreground/80" />
          ) : (
            <p className="text-[12px] text-foreground/20">
              {placeholder ?? t('aiGenerate.placeholder')}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
