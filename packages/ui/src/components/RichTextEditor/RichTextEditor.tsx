import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react';
import { type Editor, EditorContent, useEditor } from '@tiptap/react';

import { cn } from '../../lib/cn';
import { buildEditorExtensions } from './extensions';
import { docToMarkdown, joinPreserved, markdownToDoc, splitPreserved } from './markdown';
import { Toolbar, type ToolbarLabels } from './Toolbar';

/** Imperative API exposed via `ref` — used by the app to wire AI rewrite. */
export interface RichTextEditorHandle {
  /** Plain text of the current selection (`''` when the selection is empty). */
  getSelectionText: () => string;
  /** Replace the current selection with `text` (plain text, inline-parsed). */
  replaceSelection: (text: string) => void;
  /** Move keyboard focus into the editor. */
  focus: () => void;
}

export interface RichTextEditorProps {
  /** Canonical markdown content (treated as initial content + external sync). */
  value: string;
  /** Emitted (debounced ~200ms) whenever the document changes. */
  onChange: (md: string) => void;
  disabled?: boolean;
  readOnly?: boolean;
  placeholder?: string;
  className?: string;
  /** Spellcheck on the prose surface — default `true`. */
  spellCheck?: boolean;
  /** Toolbar a11y strings supplied by the renderer (package stays i18n-free). */
  labels?: ToolbarLabels;
  /** Fired when the selection gains/loses a non-empty range. */
  onSelectionChange?: (hasSelection: boolean) => void;
}

const ONCHANGE_DEBOUNCE_MS = 200;

/**
 * Export-safe WYSIWYG editor for resume / cover-letter markdown. A thin Tiptap
 * wrapper over the LOCKED schema (see `extensions.ts`) whose document maps 1:1
 * to the markdown vocabulary the Rust export consumes. Drops into the existing
 * `<textarea>` contract (`value`/`onChange`).
 *
 * CONTROLLED-COMPONENT DISCIPLINE: the editor is internally UNCONTROLLED (Tiptap
 * holds doc state) and emits debounced `onChange(md)`. The `value` prop is
 * re-parsed into the document ONLY when it differs from the last markdown this
 * component emitted — i.e. an external document switch — never on every
 * keystroke, which would reset the cursor.
 *
 * The trailing `\n---\n` link-reference block (read by the backend) is held out
 * of the editable body and re-appended verbatim on every emit, so editing can
 * never corrupt the links.
 */
export const RichTextEditor = forwardRef<RichTextEditorHandle, RichTextEditorProps>(
  function RichTextEditor(
    {
      value,
      onChange,
      disabled,
      readOnly,
      placeholder,
      className,
      spellCheck = true,
      labels,
      onSelectionChange,
    },
    ref
  ) {
    const editable = !disabled && !readOnly;

    // The last markdown THIS component emitted — guards external-sync re-parse
    // so we don't stomp the cursor on our own changes.
    const lastEmittedRef = useRef<string>(value);
    // The held-out trailing link-reference block for the current `value`.
    const tailRef = useRef<string>(splitPreserved(value).tail);
    const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
    const onChangeRef = useRef(onChange);
    onChangeRef.current = onChange;
    const onSelectionChangeRef = useRef(onSelectionChange);
    onSelectionChangeRef.current = onSelectionChange;

    const [linkDialogOpen, setLinkDialogOpen] = useState(false);
    // Drives the placeholder overlay; updated on every transaction.
    const [isEmpty, setIsEmpty] = useState(() => splitPreserved(value).body.trim() === '');

    const extensions = useMemo(() => buildEditorExtensions(), []);

    const emit = useCallback((editor: Editor) => {
      const body = docToMarkdown(editor.state.doc);
      const md = joinPreserved(body, tailRef.current);
      lastEmittedRef.current = md;
      onChangeRef.current(md);
    }, []);

    const editor = useEditor({
      extensions,
      editable,
      immediatelyRender: false,
      shouldRerenderOnTransaction: false,
      content: markdownToDoc(splitPreserved(value).body).toJSON(),
      editorProps: {
        attributes: {
          spellcheck: String(spellCheck),
          // role/aria so the editing surface is announced as a multiline textbox.
          role: 'textbox',
          'aria-multiline': 'true',
          ...(placeholder ? { 'aria-label': placeholder } : {}),
          class: 'ProseMirror-resume-editor',
        },
        handleKeyDown: (_view, event) => {
          // Mod-k opens the link dialog (StarterKit covers B/I/undo/redo).
          if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
            event.preventDefault();
            if (editable) setLinkDialogOpen(true);
            return true;
          }
          return false;
        },
      },
      onUpdate: ({ editor: ed }) => {
        setIsEmpty(ed.isEmpty);
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => emit(ed), ONCHANGE_DEBOUNCE_MS);
      },
      onSelectionUpdate: ({ editor: ed }) => {
        const { from, to } = ed.state.selection;
        onSelectionChangeRef.current?.(to > from);
      },
    });

    // External document switch: re-parse only when `value` is not what we emitted.
    useEffect(() => {
      if (!editor) return;
      if (value === lastEmittedRef.current) return;
      const { body, tail } = splitPreserved(value);
      tailRef.current = tail;
      lastEmittedRef.current = value;
      // `emitUpdate: false` so this programmatic sync doesn't echo back an onChange.
      editor.commands.setContent(markdownToDoc(body, editor.schema).toJSON(), {
        emitUpdate: false,
      });
      setIsEmpty(editor.isEmpty);
    }, [editor, value]);

    // Keep the editable flag in sync with disabled/readOnly.
    useEffect(() => {
      editor?.setEditable(editable);
    }, [editor, editable]);

    // Flush any pending debounce on unmount so the last edit is never lost.
    useEffect(() => {
      return () => {
        if (debounceRef.current) clearTimeout(debounceRef.current);
      };
    }, []);

    useImperativeHandle(
      ref,
      (): RichTextEditorHandle => ({
        getSelectionText: () => {
          if (!editor) return '';
          const { from, to, empty } = editor.state.selection;
          return empty ? '' : editor.state.doc.textBetween(from, to, '\n', ' ');
        },
        replaceSelection: (text: string) => {
          if (!editor) return;
          // The rewrite result is markdown — parse it through the same schema so
          // marks (**bold**, *italic*, [link](url)) render rather than leaking
          // their literal markers into the WYSIWYG. Insert the parsed doc's
          // block content over the current selection.
          const doc = markdownToDoc(splitPreserved(text).body, editor.schema);
          editor
            .chain()
            .focus()
            .insertContent(doc.toJSON().content ?? [])
            .run();
          // Emit immediately (rewrite is an explicit, atomic user action).
          emit(editor);
        },
        focus: () => editor?.commands.focus(),
      }),
      [editor, emit]
    );

    return (
      <div
        className={cn(
          'flex flex-col overflow-hidden rounded-lg border border-white/[0.08] bg-white/[0.02]',
          'focus-within:border-brand/40 focus-within:ring-1 focus-within:ring-brand/30',
          disabled && 'pointer-events-none opacity-60',
          className
        )}
      >
        {editor && editable && (
          <Toolbar
            editor={editor}
            disabled={disabled}
            labels={labels}
            linkDialogOpen={linkDialogOpen}
            onLinkDialogOpenChange={setLinkDialogOpen}
          />
        )}
        <div className="relative flex-1 overflow-y-auto">
          <EditorContent editor={editor} className="rich-text-editor min-h-[8rem] px-3 py-2" />
          {placeholder && isEmpty && (
            <div className="rich-text-editor-placeholder" aria-hidden>
              {placeholder}
            </div>
          )}
        </div>
      </div>
    );
  }
);
