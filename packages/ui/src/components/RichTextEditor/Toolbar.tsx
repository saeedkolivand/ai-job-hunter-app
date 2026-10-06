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
import { type ReactNode, useEffect, useState } from 'react';
import type { Editor } from '@tiptap/react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { LinkDialog } from './LinkDialog';
import { FALLBACK, type LinkSuggestion, type ToolbarLabels } from './toolbarLabels';
import { useLinkDialog } from './useLinkDialog';

export type { LinkSuggestion, ToolbarLabels } from './toolbarLabels';

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

  const dialog = useLinkDialog(editor, onLinkDialogOpenChange, linkSuggestions);

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
        <ToolButton
          label={l.link}
          active={linkActive}
          disabled={disabled}
          onClick={dialog.openLinkDialog}
        >
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

      <LinkDialog open={linkDialogOpen} linkActive={linkActive} labels={l} dialog={dialog} />
    </>
  );
}
