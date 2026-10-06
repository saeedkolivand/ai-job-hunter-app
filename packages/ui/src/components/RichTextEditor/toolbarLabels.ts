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

export const FALLBACK: Required<ToolbarLabels> = {
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
export function linkUrlHint(url: string): string {
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
