import { Code, Eye, Pencil, Save, Sparkles } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useCallback, useMemo, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import {
  Button,
  MarkdownMessage,
  RichTextEditor,
  type RichTextEditorHandle,
  SegmentedControl,
  TextArea,
  type ToolbarLabels,
} from '@ajh/ui';

import { useSelectedModel } from '@/components/ui/ModelSelector';
import { buildLinkSuggestions, type GenerationMeta, type RewriteDocType } from '@/lib/generate';
import { useContactProfile } from '@/services/use-contact-profile';

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
  /**
   * The source résumé text (the uploaded/original), when available. Feeds the
   * link dialog's suggestion pick-list with the full extracted link map. Optional
   * — callers without a source (history cards) still get ContactProfile + in-doc
   * suggestions.
   */
  sourceResume?: string;
}

/**
 * The frozen rewrite session captured when a rewrite starts. Two splice modes:
 *
 * - **`'source'`** (raw `<TextArea>`): the result is spliced into the canonical
 *   string at `[start, end)` against `snapshot` (textarea offsets stay valid
 *   because the textarea is locked while streaming).
 * - **`'editor'`** (WYSIWYG `RichTextEditor`): the editor owns its own selection;
 *   on accept the result is handed to `editorRef.replaceSelection`, which splices
 *   in-document and emits `onChange`. No string offsets are involved, so they are
 *   absent for this mode.
 */
interface FrozenRange {
  mode: 'source' | 'editor';
  /** Source-mode splice range (absent in editor mode). */
  start?: number;
  end?: number;
  /** Source-mode value snapshot at freeze time — guards the splice against drift. */
  snapshot?: string;
  target: RewriteTarget;
}

/**
 * Shared Preview | Edit | Source surface for generated documents.
 *
 * - **Preview** renders prettified markdown (`MarkdownMessage`) or the caller's
 *   `previewSlot` (e.g. the real-PDF view).
 * - **Edit** is the WYSIWYG `RichTextEditor` — an export-safe visual editor over
 *   the same canonical markdown string. It holds its own doc state and emits
 *   debounced `onChange(md)`.
 * - **Source** is the raw monospace `TextArea` (power-user / inspection view); the
 *   raw string stays canonical, so copy/export read exactly what is typed. Both
 *   Edit and Source feed the identical `value`/`onChange`, so all callers keep
 *   working unchanged.
 * - **F4 inline rewrite** (when `enableRewrite`): selecting text in Edit or Source
 *   reveals a "Rewrite with AI" trigger. In **Source**, triggering freezes the
 *   selection range + a snapshot, locks the textarea while a single rewrite streams
 *   into the popover preview, and on Accept splices the result into the raw text at
 *   the frozen range via `onChange`. In **Edit**, the `RichTextEditor` owns the
 *   selection: triggering snapshots the selection text, locks the editor
 *   (`readOnly`) while streaming, and on Accept calls
 *   `editorRef.replaceSelection(result)` — which splices in-document and emits
 *   `onChange`, so the cursor is never lost to a value round-trip.
 *
 * The popover is anchored to a toolbar above the editor rather than the caret —
 * neither surface exposes reliable per-character pixel coordinates, so caret-pixel
 * math is deliberately avoided.
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
  sourceResume,
}: EditableOutputProps) {
  const { t } = useTranslation();
  const selectedModel = useSelectedModel();
  const effectiveModel = model ?? selectedModel;
  // Rewrite in the document's language (falls back to English) so the AI reply
  // matches the document instead of forcing English.
  const rewriteLocale = meta?.targetLanguage ?? 'en';

  const [view, setView] = useState<'preview' | 'edit' | 'source'>('preview');
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const editorRef = useRef<RichTextEditorHandle | null>(null);

  // Live selection (range only) used to show/hide the rewrite trigger.
  const [hasSelection, setHasSelection] = useState(false);
  // Frozen range while a rewrite popover is open — null when closed.
  const [frozen, setFrozen] = useState<FrozenRange | null>(null);

  const { data: contactProfile } = useContactProfile();
  // Known links offered in the editor's link dialog as a pick-list — built from
  // the authoritative ContactProfile, the source résumé's extracted link map, and
  // links already inline in the document; de-duplicated by URL.
  const linkSuggestions = useMemo(
    () => buildLinkSuggestions({ contactProfile, docValue: value, sourceResume }),
    [contactProfile, value, sourceResume]
  );

  // Toolbar a11y/labels for the WYSIWYG editor — all i18n lives in the renderer so
  // `@ajh/ui` stays translation-free (it provides English fallbacks).
  const editorLabels = useMemo<ToolbarLabels>(
    () => ({
      toolbarLabel: t('aiGenerate.editor.toolbar'),
      bold: t('aiGenerate.editor.bold'),
      italic: t('aiGenerate.editor.italic'),
      link: t('aiGenerate.editor.link'),
      bulletList: t('aiGenerate.editor.bulletList'),
      heading2: t('aiGenerate.editor.heading2'),
      heading3: t('aiGenerate.editor.heading3'),
      undo: t('aiGenerate.editor.undo'),
      redo: t('aiGenerate.editor.redo'),
      linkDialogTitle: t('aiGenerate.editor.linkDialogTitle'),
      linkLabelField: t('aiGenerate.editor.linkLabelField'),
      linkUrlField: t('aiGenerate.editor.linkUrlField'),
      linkUrlPlaceholder: t('aiGenerate.editor.linkUrlPlaceholder'),
      linkUrlError: t('aiGenerate.editor.linkUrlError'),
      linkSave: t('aiGenerate.editor.linkSave'),
      linkRemove: t('aiGenerate.editor.linkRemove'),
      linkCancel: t('aiGenerate.editor.linkCancel'),
      linkSuggestionsTitle: t('aiGenerate.editor.linkSuggestions'),
    }),
    [t]
  );

  // Source-view selection (textarea offsets) → toggles the rewrite trigger.
  const updateSelection = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    setHasSelection(el.selectionStart !== el.selectionEnd);
  }, []);

  // Open the rewrite popover for the Source (raw textarea) surface — captures
  // offsets + a snapshot so the result can be spliced into the canonical string.
  const openSourceRewrite = () => {
    const el = textareaRef.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    if (start === end) return;
    const selection = value.slice(start, end);
    setFrozen({
      mode: 'source',
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

  // Open the rewrite popover for the WYSIWYG surface. The editor owns the
  // selection, so we read its plain-text selection plus the surrounding plain
  // text (`before`/`after`) — pure style-grounding context for the prompt (fenced,
  // never rewritten). No string offsets are involved: on Accept the result is
  // spliced in-document via `editorRef.replaceSelection`.
  const openEditorRewrite = () => {
    const ctx = editorRef.current?.getSelectionContext();
    if (!ctx?.selection) return;
    const { selection, before, after } = ctx;
    setFrozen({
      mode: 'editor',
      target: { selection, before, after },
    });
  };

  const openRewrite = () => (view === 'edit' ? openEditorRewrite() : openSourceRewrite());

  const closeRewrite = () => {
    setFrozen(null);
    // Return focus to the active editing surface for keyboard continuity.
    if (view === 'edit') editorRef.current?.focus();
    else textareaRef.current?.focus();
  };

  const acceptRewrite = (replacement: string) => {
    if (!frozen) return;
    if (frozen.mode === 'editor') {
      // The WYSIWYG editor splices in-document and emits onChange itself, so the
      // cursor/selection survives (no value round-trip).
      editorRef.current?.replaceSelection(replacement);
      setFrozen(null);
      setHasSelection(false);
      requestAnimationFrame(() => editorRef.current?.focus());
      return;
    }
    // Source mode: splice into the snapshot at the frozen range — the textarea was
    // locked while streaming, so `value` cannot have drifted, but splice against
    // the snapshot for safety.
    const base = frozen.snapshot ?? value;
    const start = frozen.start ?? 0;
    const end = frozen.end ?? 0;
    const next = base.slice(0, start) + replacement + base.slice(end);
    onChange(next);
    setFrozen(null);
    // Restore focus + place the caret after the inserted text.
    const caret = start + replacement.length;
    requestAnimationFrame(() => {
      const el = textareaRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(caret, caret);
      setHasSelection(false);
    });
  };

  const isEditing = view === 'edit' || view === 'source';
  const rewriteVisible = enableRewrite && isEditing;

  // Shared rewrite popover — anchored to a toolbar position above the editor
  // (neither surface exposes per-char coords, so no caret-pixel math). Rendered in
  // both the WYSIWYG (Edit) and raw (Source) surfaces; the `frozen.mode` set when
  // it opened decides how Accept splices the result.
  const rewriteOverlay = (
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
  );

  return (
    <div className={className}>
      <div className="mb-2 flex shrink-0 items-center justify-between gap-2">
        {/* Rewrite trigger — left, only in Edit/Source with a live selection. */}
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
          {/* Save — in Edit/Source when a save handler is supplied; commits the
              current edits (e.g. so the preview recompiles on Save, not per keystroke). */}
          {onSave && isEditing && (
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

          <SegmentedControl<'preview' | 'edit' | 'source'>
            ariaLabel={t('aiGenerate.viewMode')}
            size="sm"
            tone="brand"
            value={view}
            onChange={setView}
            options={[
              { value: 'preview', label: t('aiGenerate.preview'), icon: Eye },
              { value: 'edit', label: t('aiGenerate.edit'), icon: Pencil },
              { value: 'source', label: t('aiGenerate.source'), icon: Code },
            ]}
            className="shrink-0"
          />
        </div>
      </div>

      {view === 'edit' ? (
        <div className="relative h-full w-full">
          {/* WYSIWYG edit surface — same canonical `value`/`onChange` as Source.
              Lock with readOnly while a rewrite streams (mirrors the Source path). */}
          <RichTextEditor
            ref={editorRef}
            value={value}
            onChange={onChange}
            disabled={disabled}
            readOnly={frozen !== null}
            spellCheck
            labels={editorLabels}
            linkSuggestions={linkSuggestions}
            onSelectionChange={setHasSelection}
            placeholder={placeholder ?? t('aiGenerate.placeholder')}
            className="h-full w-full"
          />

          {rewriteOverlay}
        </div>
      ) : view === 'source' ? (
        <div className="relative h-full w-full">
          {/* Source — raw markdown power-user / inspection view (hand-editable). */}
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

          {rewriteOverlay}
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
