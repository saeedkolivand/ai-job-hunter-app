import type { Extensions } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';

import { EditorDecorations } from './decorations';

/**
 * Allowed link protocols. The editor is export-safe by construction: the only
 * schemes a user (or a paste) can introduce are the three that survive to the
 * PDF/DOCX export and are safe to render in a webview. `javascript:`, `data:`,
 * `tel:`, `ftp:` etc. are all rejected — both on typed/dialog insert and on the
 * paste-coercion path (StarterKit's Link `isAllowedUri`).
 */
export const ALLOWED_LINK_PROTOCOLS = ['http', 'https', 'mailto'] as const;

/** Whitespace test used by the URL sanitiser. */
const WS = /\s/;

/**
 * Strip ASCII whitespace + C0 control characters from a URL before scheme
 * detection, so a smuggled newline-in-scheme cannot slip past the scheme check.
 * Done with a code-point filter (no control-char regex) so the intent is
 * explicit and lint-clean.
 */
function stripControlAndWhitespace(url: string): string {
  let out = '';
  for (const ch of url) {
    const code = ch.codePointAt(0) ?? 0;
    // Drop C0 controls (0x00-0x1F), DEL (0x7F), and any whitespace.
    if (code <= 0x1f || code === 0x7f || WS.test(ch)) continue;
    out += ch;
  }
  return out;
}

/**
 * Validate a URL against the locked protocol allow-list. Used by both the Link
 * dialog (reject before inserting) and the Tiptap Link `isAllowedUri` hook
 * (reject on paste). Bare/relative URLs are rejected — every resume/cover-letter
 * link the backend reads is absolute.
 */
export function isAllowedLinkUrl(url: string): boolean {
  const normalized = stripControlAndWhitespace(url);
  if (!normalized) return false;
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(normalized)?.[1]?.toLowerCase();
  if (!scheme) return false; // no scheme → relative/bare → reject (we only store absolute links)
  return (ALLOWED_LINK_PROTOCOLS as readonly string[]).includes(scheme);
}

/**
 * The LOCKED Tiptap schema for the resume/cover-letter editor.
 *
 * Only the formatting vocabulary that round-trips to the Rust `parse_line`
 * heuristics and survives the Typst export is enabled:
 *   heading (H2/H3), paragraph, text, bulletList, listItem, bold, italic,
 *   hardBreak, history (undo/redo), and link (http/https/mailto only).
 *
 * Everything else StarterKit ships is DISABLED so neither typing nor paste can
 * introduce a node the export would silently strip (orderedList, codeBlock,
 * blockquote, horizontalRule, inline code, strike, underline).
 *
 * Shared by `RichTextEditor` (live editor) and `markdown.ts` (headless schema
 * via `getSchema`), so the parse/serialize round-trip is validated against the
 * exact schema the user edits.
 */
export function buildEditorExtensions(): Extensions {
  return [
    StarterKit.configure({
      // Enabled, but constrained to the two visual heading levels.
      heading: { levels: [2, 3] },
      // Disabled — not part of the export-safe vocabulary.
      orderedList: false,
      codeBlock: false,
      blockquote: false,
      horizontalRule: false,
      code: false,
      strike: false,
      underline: false,
      // StarterKit v3 bundles Link; configure its allow-list rather than adding
      // a second copy of the extension (a duplicate would throw at schema build).
      link: {
        autolink: true,
        openOnClick: false,
        protocols: [...ALLOWED_LINK_PROTOCOLS],
        defaultProtocol: 'https',
        // Strict: only http/https/mailto. Overrides StarterKit's permissive
        // default that also allows ftp/tel/sms/etc.
        isAllowedUri: (url) => isAllowedLinkUrl(url),
        shouldAutoLink: (url) => isAllowedLinkUrl(url),
        HTMLAttributes: { rel: 'noopener noreferrer nofollow' },
      },
    }),
    // Display-only "document skin" decorations (header region). Adds no schema
    // nodes/marks — pure presentation — so the markdown round-trip is unaffected.
    EditorDecorations,
  ];
}
