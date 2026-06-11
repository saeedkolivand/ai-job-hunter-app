import {
  Bold,
  Heading2,
  Heading3,
  Italic,
  Link as LinkIcon,
  List,
  Redo2,
  Undo2,
} from 'lucide-react';
import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';
import type { Editor } from '@tiptap/react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { Input } from '../Input';
import { ModalShell } from '../ModalShell';
import { isAllowedLinkUrl } from './extensions';

/**
 * A known link the renderer feeds into the dialog as a pick-list option, so the
 * user can choose a URL they already have (LinkedIn, GitHub, a project page,
 * an email) instead of retyping it. `url` is the raw href (e.g. an `https://`
 * URL or a `mailto:` address); `label` is the human-readable name.
 */
export interface LinkSuggestion {
  label: string;
  url: string;
}

/**
 * a11y labels for the toolbar. Supplied by the renderer (via `RichTextEditor`'s
 * `labels` prop) so this package stays translation-free. All optional with
 * sensible English fallbacks for standalone/Storybook use.
 */
export interface ToolbarLabels {
  /** Accessible name for the toolbar container itself (the `role="toolbar"`). */
  toolbarLabel?: string;
  bold?: string;
  italic?: string;
  link?: string;
  bulletList?: string;
  heading2?: string;
  heading3?: string;
  undo?: string;
  redo?: string;
  /** Link dialog */
  linkDialogTitle?: string;
  linkLabelField?: string;
  linkUrlField?: string;
  linkUrlPlaceholder?: string;
  linkUrlError?: string;
  linkSave?: string;
  linkRemove?: string;
  linkCancel?: string;
  /** Title of the suggestions pick-list shown when `linkSuggestions` is non-empty. */
  linkSuggestionsTitle?: string;
}

const FALLBACK: Required<ToolbarLabels> = {
  toolbarLabel: 'Text formatting',
  bold: 'Bold',
  italic: 'Italic',
  link: 'Link',
  bulletList: 'Bullet list',
  heading2: 'Heading 2',
  heading3: 'Heading 3',
  undo: 'Undo',
  redo: 'Redo',
  linkDialogTitle: 'Add link',
  linkLabelField: 'Text',
  linkUrlField: 'URL',
  linkUrlPlaceholder: 'https://example.com',
  linkUrlError: 'Enter an http, https or mailto URL.',
  linkSave: 'Save',
  linkRemove: 'Remove',
  linkCancel: 'Cancel',
  linkSuggestionsTitle: 'Your links',
};

/**
 * Derive a compact, readable hint from a link href for the suggestions list:
 * `host` + a truncated path for http(s) URLs, the bare address for `mailto:`,
 * and the raw string otherwise. Display-only — never used for validation.
 */
function linkUrlHint(url: string): string {
  const mailto = /^mailto:/i.exec(url);
  if (mailto) return url.slice(mailto[0].length);
  try {
    const parsed = new URL(url);
    const path = parsed.pathname === '/' ? '' : parsed.pathname.replace(/\/$/, '');
    const tail = `${parsed.host}${path}${parsed.search}`;
    return tail.length > 44 ? `${tail.slice(0, 43)}…` : tail;
  } catch {
    return url;
  }
}

interface ToolbarProps {
  editor: Editor;
  disabled?: boolean;
  labels?: ToolbarLabels;
  /** External request to open the link dialog (e.g. Mod-k keyboard shortcut). */
  linkDialogOpen: boolean;
  onLinkDialogOpenChange: (open: boolean) => void;
  /** Known links offered as a pick-list under the URL field (optional). */
  linkSuggestions?: LinkSuggestion[];
}

interface ToolButtonProps {
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}

function ToolButton({ label, active, disabled, onClick, children }: ToolButtonProps) {
  return (
    <Button
      variant="unstyled"
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      // Prevent the editor from losing its selection when the button is pressed.
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
      className={cn(
        'inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors',
        'text-foreground/60 hover:bg-white/[0.08] hover:text-foreground/90',
        active && 'bg-brand/15 text-brand',
        disabled && 'pointer-events-none opacity-40'
      )}
    >
      {children}
    </Button>
  );
}

const ICON = 15;

/**
 * Fixed formatting toolbar rendered above the editor. Buttons in order:
 * Bold, Italic, Link, Bullet list, H2, H3, Undo, Redo. The Link button opens a
 * `ModalShell` dialog (label + URL) validated against the http/https/mailto
 * allow-list. Active-state styling reflects the current selection's marks/nodes.
 */
export function Toolbar({
  editor,
  disabled,
  labels,
  linkDialogOpen,
  onLinkDialogOpenChange,
  linkSuggestions,
}: ToolbarProps) {
  const l = { ...FALLBACK, ...labels };
  // Re-render on every editor transaction so active state / undo-redo
  // availability stay in sync. A counter is cheaper than reading editor state.
  const [, force] = useState(0);
  useEffect(() => {
    const update = () => force((n) => n + 1);
    editor.on('transaction', update);
    editor.on('selectionUpdate', update);
    return () => {
      editor.off('transaction', update);
      editor.off('selectionUpdate', update);
    };
  }, [editor]);

  const [linkLabel, setLinkLabel] = useState('');
  const [linkUrl, setLinkUrl] = useState('');
  const [urlError, setUrlError] = useState(false);

  const openLinkDialog = useCallback(() => {
    const { state } = editor;
    const { from, to, empty } = state.selection;
    const selected = empty ? '' : state.doc.textBetween(from, to, ' ');
    const existingHref = String(editor.getAttributes('link').href ?? '');
    setLinkLabel(selected);
    setLinkUrl(existingHref);
    setUrlError(false);
    onLinkDialogOpenChange(true);
  }, [editor, onLinkDialogOpenChange]);

  const closeLinkDialog = useCallback(
    () => onLinkDialogOpenChange(false),
    [onLinkDialogOpenChange]
  );

  const applyLink = useCallback(() => {
    if (!isAllowedLinkUrl(linkUrl)) {
      setUrlError(true);
      return;
    }
    const chain = editor.chain().focus().extendMarkRange('link');
    const { empty } = editor.state.selection;
    const text = linkLabel.trim();
    if (empty) {
      // No selection: insert the label text carrying the link mark.
      const content = text || linkUrl;
      chain
        .insertContent({
          type: 'text',
          text: content,
          marks: [{ type: 'link', attrs: { href: linkUrl } }],
        })
        .run();
    } else {
      // Selection present: set the link on it (optionally replace the visible text).
      if (text) {
        chain
          .insertContent({
            type: 'text',
            text,
            marks: [{ type: 'link', attrs: { href: linkUrl } }],
          })
          .run();
      } else {
        chain.setLink({ href: linkUrl }).run();
      }
    }
    closeLinkDialog();
  }, [editor, linkLabel, linkUrl, closeLinkDialog]);

  const removeLink = useCallback(() => {
    editor.chain().focus().extendMarkRange('link').unsetLink().run();
    closeLinkDialog();
  }, [editor, closeLinkDialog]);

  // Pick a suggestion: fill the URL field, and the label field too — but only
  // when it is empty, so a selected-text label the user already has is never
  // overwritten. Validation still runs on submit via `isAllowedLinkUrl`.
  const pickSuggestion = useCallback(
    (s: LinkSuggestion) => {
      setLinkUrl(s.url);
      setUrlError(false);
      setLinkLabel((current) => (current.trim() ? current : s.label));
    },
    [setLinkUrl, setUrlError, setLinkLabel]
  );

  // Filter the pick-list by a case-insensitive substring of whatever is typed
  // in either field, matched across the label, the raw URL, and the hint. Empty
  // query → show all. An empty result hides the section entirely (no rows).
  const visibleSuggestions = useMemo(() => {
    if (!linkSuggestions?.length) return [];
    const q = `${linkUrl} ${linkLabel}`.trim().toLowerCase();
    if (!q) return linkSuggestions;
    const terms = q.split(/\s+/);
    return linkSuggestions.filter((s) => {
      const haystack = `${s.label} ${s.url} ${linkUrlHint(s.url)}`.toLowerCase();
      return terms.every((t) => haystack.includes(t));
    });
  }, [linkSuggestions, linkUrl, linkLabel]);

  const linkActive = editor.isActive('link');

  return (
    <>
      <div
        role="toolbar"
        aria-label={l.toolbarLabel}
        className="flex items-center gap-0.5 border-b border-white/[0.06] px-2 py-1.5"
      >
        <ToolButton
          label={l.bold}
          active={editor.isActive('bold')}
          disabled={disabled}
          onClick={() => editor.chain().focus().toggleBold().run()}
        >
          <Bold size={ICON} />
        </ToolButton>
        <ToolButton
          label={l.italic}
          active={editor.isActive('italic')}
          disabled={disabled}
          onClick={() => editor.chain().focus().toggleItalic().run()}
        >
          <Italic size={ICON} />
        </ToolButton>
        <ToolButton label={l.link} active={linkActive} disabled={disabled} onClick={openLinkDialog}>
          <LinkIcon size={ICON} />
        </ToolButton>

        <span className="mx-1 h-4 w-px bg-white/10" aria-hidden />

        <ToolButton
          label={l.bulletList}
          active={editor.isActive('bulletList')}
          disabled={disabled}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
        >
          <List size={ICON} />
        </ToolButton>
        <ToolButton
          label={l.heading2}
          active={editor.isActive('heading', { level: 2 })}
          disabled={disabled}
          onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        >
          <Heading2 size={ICON} />
        </ToolButton>
        <ToolButton
          label={l.heading3}
          active={editor.isActive('heading', { level: 3 })}
          disabled={disabled}
          onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
        >
          <Heading3 size={ICON} />
        </ToolButton>

        <span className="mx-1 h-4 w-px bg-white/10" aria-hidden />

        <ToolButton
          label={l.undo}
          disabled={disabled || !editor.can().undo()}
          onClick={() => editor.chain().focus().undo().run()}
        >
          <Undo2 size={ICON} />
        </ToolButton>
        <ToolButton
          label={l.redo}
          disabled={disabled || !editor.can().redo()}
          onClick={() => editor.chain().focus().redo().run()}
        >
          <Redo2 size={ICON} />
        </ToolButton>
      </div>

      <ModalShell
        open={linkDialogOpen}
        onClose={closeLinkDialog}
        maxWidth="max-w-sm"
        ariaLabel={l.linkDialogTitle}
      >
        <form
          onSubmit={(e) => {
            e.preventDefault();
            applyLink();
          }}
          className="flex flex-col gap-3 p-4"
        >
          <h2 className="text-sm font-semibold text-foreground/90">{l.linkDialogTitle}</h2>
          <label className="flex flex-col gap-1 text-xs text-foreground/60">
            {l.linkLabelField}
            <Input value={linkLabel} onChange={(e) => setLinkLabel(e.target.value)} autoFocus />
          </label>
          <label className="flex flex-col gap-1 text-xs text-foreground/60">
            {l.linkUrlField}
            <Input
              type="url"
              value={linkUrl}
              placeholder={l.linkUrlPlaceholder}
              onChange={(e) => {
                setLinkUrl(e.target.value);
                if (urlError) setUrlError(false);
              }}
              aria-invalid={urlError}
              className={cn(urlError && 'border-red-500/50')}
            />
          </label>
          {urlError && <p className="text-xs text-red-300">{l.linkUrlError}</p>}
          {visibleSuggestions.length > 0 && (
            <div className="flex flex-col gap-1">
              <span className="text-xs font-medium text-foreground/50">
                {l.linkSuggestionsTitle}
              </span>
              <ul className="flex max-h-40 flex-col gap-0.5 overflow-y-auto rounded-md border border-white/[0.06] bg-white/[0.02] p-1">
                {visibleSuggestions.map((s) => (
                  <li key={`${s.label} ${s.url}`}>
                    <Button
                      type="button"
                      variant="unstyled"
                      // Filling the fields must not blur/close the dialog inputs.
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => pickSuggestion(s)}
                      aria-label={`${s.label} — ${s.url}`}
                      className={cn(
                        'flex w-full items-baseline justify-between gap-3 rounded px-2 py-1.5 text-left transition-colors',
                        'hover:bg-brand/10 focus-visible:bg-brand/10'
                      )}
                    >
                      <span className="truncate text-xs text-foreground/90">{s.label}</span>
                      <span className="shrink-0 truncate font-mono text-[0.7rem] text-foreground/45">
                        {linkUrlHint(s.url)}
                      </span>
                    </Button>
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className="flex items-center justify-between gap-2 pt-1">
            {linkActive ? (
              <Button type="button" variant="danger" size="sm" onClick={removeLink}>
                {l.linkRemove}
              </Button>
            ) : (
              <span />
            )}
            <div className="flex gap-2">
              <Button type="button" variant="ghost" size="sm" onClick={closeLinkDialog}>
                {l.linkCancel}
              </Button>
              <Button type="submit" variant="primary" size="sm">
                {l.linkSave}
              </Button>
            </div>
          </div>
        </form>
      </ModalShell>
    </>
  );
}
