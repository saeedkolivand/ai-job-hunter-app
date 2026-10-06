import { useCallback, useMemo, useState } from 'react';
import type { Editor } from '@tiptap/react';

import { isAllowedLinkUrl } from './extensions';
import { type LinkSuggestion, linkUrlHint } from './toolbarLabels';

/** Link-dialog state + actions for the formatting toolbar. */
export function useLinkDialog(
  editor: Editor,
  onLinkDialogOpenChange: (open: boolean) => void,
  linkSuggestions: LinkSuggestion[] | undefined
) {
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

  return {
    linkLabel,
    setLinkLabel,
    linkUrl,
    setLinkUrl,
    urlError,
    setUrlError,
    openLinkDialog,
    closeLinkDialog,
    applyLink,
    removeLink,
    pickSuggestion,
    visibleSuggestions,
  };
}

export type LinkDialogState = ReturnType<typeof useLinkDialog>;
