/**
 * Build the link pick-list shown in the resume/cover-letter editor's link dialog.
 *
 * Pure + testable: unions the three places the app already knows the candidate's
 * links — the authoritative ContactProfile, the source résumé's extracted link
 * map (`getLinkMap` + `getBodyLinkMap`), and links already inline in the current
 * document — then de-duplicates by normalized URL, keeping the FIRST (most
 * authoritative) label. The dialog still validates the picked URL via
 * `isAllowedLinkUrl`, so this only needs to keep http/https/mailto candidates.
 */
import { getBodyLinkMap, getLinkMap } from '@ajh/prompts/generate';
import type { ContactProfile } from '@ajh/shared';
import type { LinkSuggestion } from '@ajh/ui';

/**
 * An inline markdown link span `[label](url)` in the live document. Mirrors the
 * `MD_LINK_SPAN_RE` shape in packages/prompts/src/generate/links.ts; the two
 * capture groups are the label and the raw URL. The URL group allows one level
 * of balanced parens so Wikipedia-style URLs (e.g. `Python_(programming_language)`)
 * are captured whole instead of being truncated at the first `)`. Bounded
 * quantifiers (200/2000) cap adversarial input to keep the regex linear.
 */
const MD_LINK_SPAN_RE = /\[([^\]]{1,200})\]\(((?:[^()]|\([^()]{0,200}\)){1,2000})\)/g;

/** URL schemes we surface — matches the dialog's `isAllowedLinkUrl` gate. */
const ALLOWED_SCHEMES = new Set(['http:', 'https:', 'mailto:']);

/**
 * Normalize a URL for de-duplication: lowercase host, drop a single trailing
 * slash; for `mailto:` compare the (lowercased) address. Returns null for URLs we
 * don't surface (non-http/https/mailto, or unparseable). The normalized form is a
 * de-dup KEY only — the original `url` is always what we keep.
 */
function normalizeUrl(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const lower = trimmed.toLowerCase();
  if (lower.startsWith('mailto:')) {
    const addr = trimmed.slice('mailto:'.length).trim().toLowerCase();
    return addr ? `mailto:${addr}` : null;
  }
  try {
    const u = new URL(trimmed);
    if (!ALLOWED_SCHEMES.has(u.protocol)) return null;
    const host = u.host.toLowerCase();
    const path = u.pathname.replace(/\/$/, '');
    return `${u.protocol}//${host}${path}${u.search}${u.hash}`;
  } catch {
    return null;
  }
}

/** A raw candidate before scheme-filtering / de-dup. */
interface RawSuggestion {
  label: string;
  url: string;
}

/**
 * Build + de-dup the link suggestions for the editor's pick-list.
 *
 * Precedence (first wins on URL collision): ContactProfile → source-résumé link
 * map → links already inline in the current document. So the most authoritative
 * label survives (e.g. the profile's "LinkedIn" beats an in-doc anchor text).
 */
export function buildLinkSuggestions(args: {
  contactProfile?: ContactProfile | null;
  docValue: string;
  sourceResume?: string;
}): LinkSuggestion[] {
  const { contactProfile, docValue, sourceResume } = args;
  const raw: RawSuggestion[] = [];

  // 1) ContactProfile — the authoritative header links.
  if (contactProfile) {
    if (contactProfile.linkedin) raw.push({ label: 'LinkedIn', url: contactProfile.linkedin });
    if (contactProfile.github) raw.push({ label: 'GitHub', url: contactProfile.github });
    if (contactProfile.website) raw.push({ label: 'Website', url: contactProfile.website });
    if (contactProfile.email) raw.push({ label: 'Email', url: `mailto:${contactProfile.email}` });
    for (const link of contactProfile.extraLinks ?? []) {
      raw.push({ label: link.label, url: link.url });
    }
  }

  // 2) Source résumé — the full extracted link map (contact + body links).
  if (sourceResume) {
    for (const [label, url] of Object.entries(getLinkMap(sourceResume))) {
      raw.push({ label, url });
    }
    for (const [label, url] of Object.entries(getBodyLinkMap(sourceResume))) {
      raw.push({ label, url });
    }
  }

  // 3) Links already inline in the current document.
  for (const m of docValue.matchAll(MD_LINK_SPAN_RE)) {
    raw.push({ label: m[1] ?? '', url: m[2] ?? '' });
  }

  // De-dup by normalized URL, first occurrence wins (precedence above).
  const seen = new Set<string>();
  const out: LinkSuggestion[] = [];
  for (const entry of raw) {
    const label = entry.label.trim();
    const url = entry.url.trim();
    if (!label || !url) continue;
    const key = normalizeUrl(url);
    if (!key) continue; // drops non-http/https/mailto + unparseable
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ label, url });
  }
  return out;
}
