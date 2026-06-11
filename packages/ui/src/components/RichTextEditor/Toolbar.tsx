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
import { type ReactNode, useCallback, useEffect, useState } from 'react';
import type { Editor } from '@tiptap/react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { Input } from '../Input';
import { ModalShell } from '../ModalShell';
import { isAllowedLinkUrl } from './extensions';

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
};

interface ToolbarProps {
  editor: Editor;
  disabled?: boolean;
  labels?: ToolbarLabels;
  /** External request to open the link dialog (e.g. Mod-k keyboard shortcut). */
  linkDialogOpen: boolean;
  onLinkDialogOpenChange: (open: boolean) => void;
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
