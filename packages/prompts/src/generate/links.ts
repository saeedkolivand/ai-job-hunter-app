/**
 * Resume contact-link extraction + post-generation hyperlink injection.
 *
 * The Rust PDF/DOCX extractor appends a `\n---\n` markdown reference block of
 * `[anchor](url)` entries. We turn that into (a) a prompt instruction telling the
 * AI to write short labels (LinkedIn, GitHub), and (b) a post-generation injector
 * that replaces those labels with real markdown links.
 */

// Known social/portfolio domains that belong in a resume contact line.
const PROFILE_DOMAINS = [
  'linkedin.com',
  'github.com',
  'gitlab.com',
  'twitter.com',
  'x.com',
  'behance.net',
  'dribbble.com',
  'medium.com',
  'stackoverflow.com',
  'dev.to',
  'codepen.io',
  'youtube.com',
  'youtu.be',
  'notion.so',
  'figma.com',
  'npmjs.com',
  'crates.io',
  'solo.to',
  'bio.link',
  'linktr.ee',
  'bento.me',
];

function isProfileUrl(url: string): boolean {
  try {
    const host = new URL(url).hostname.replace(/^www\./, '').toLowerCase();
    return PROFILE_DOMAINS.some((d) => host === d || host.endsWith(`.${d}`));
  } catch {
    return false;
  }
}

/**
 * Derive a friendly label from a URL — mirrors the Rust url_label() in links.rs.
 * Used when a PDF annotation stores the raw URL as its anchor text instead of a label.
 * Exported for the cross-language parity test against Rust url_label().
 */
export function urlToFriendlyLabel(url: string): string {
  try {
    const host = new URL(url).hostname.replace(/^www\./, '').toLowerCase();
    if (host.startsWith('linkedin.com')) return 'LinkedIn';
    if (host.startsWith('github.com')) return 'GitHub';
    if (host.startsWith('gitlab.com')) return 'GitLab';
    if (host.startsWith('twitter.com') || host.startsWith('x.com')) return 'Twitter';
    if (host.startsWith('behance.net')) return 'Behance';
    if (host.startsWith('dribbble.com')) return 'Dribbble';
    if (host.startsWith('medium.com')) return 'Medium';
    if (host.startsWith('stackoverflow.com')) return 'Stack Overflow';
    if (host.startsWith('dev.to')) return 'Dev.to';
    if (host.startsWith('codepen.io')) return 'CodePen';
    if (host.startsWith('youtube.com') || host.startsWith('youtu.be')) return 'YouTube';
    if (host.startsWith('notion.so')) return 'Notion';
    if (host.startsWith('figma.com')) return 'Figma';
    if (host.startsWith('npmjs.com')) return 'npm';
    if (host.startsWith('crates.io')) return 'crates.io';
    // Unknown domain: the bare host (www-stripped, no path). Mirrors the Rust
    // url_label() fallback exactly so the two implementations cannot drift — see
    // the parity test (fixtures/url-labels.json, cargo test export::links).
    return host;
  } catch {
    return url;
  }
}

interface ParsedResumeLinks {
  /** Compact block to inject before <candidate_resume> */
  block: string;
  /** Clean email address extracted from mailto annotation, or empty string */
  cleanEmail: string;
}

/** Generic label for a single non-platform personal site / portfolio URL. */
const WEBSITE_LABEL = 'Website';

interface LinkBlockEntry {
  anchor: string;
  url: string;
}

/**
 * Parse the `\n---\n` markdown reference block (appended by the Rust extractor)
 * into raw `[anchor](url)` entries, in document order. Returns [] when absent.
 */
function parseLinkBlock(resume: string): LinkBlockEntry[] {
  const sep = resume.lastIndexOf('\n---\n');
  if (sep === -1) return [];
  const block = resume.slice(sep + 5);
  const entries: LinkBlockEntry[] = [];
  for (const l of block.split('\n')) {
    if (!l.startsWith('- [')) continue;
    const m = l.match(/^- \[([^\]]+)\]\(([^)]+)\)$/);
    if (!m) continue;
    const anchor = m[1] ?? '';
    const url = m[2] ?? '';
    if (anchor && url) entries.push({ anchor, url });
  }
  return entries;
}

/** Decoded, empties-removed path segments of a URL; [] on parse failure. */
function pathSegments(url: string): string[] {
  try {
    return new URL(url).pathname
      .split('/')
      .map((s) => {
        try {
          return decodeURIComponent(s);
        } catch {
          return s;
        }
      })
      .filter(Boolean);
  } catch {
    return [];
  }
}

/** A bare-root URL — host only, no meaningful path. The shape of a homepage. */
function isBareRoot(url: string): boolean {
  return pathSegments(url).length === 0;
}

/**
 * Is this platform URL a *profile* (belongs on the contact line) rather than a
 * deep link to a specific repo/article (which belongs on its own body item)?
 * `github.com/<user>` is a profile; `github.com/<user>/<repo>` is a project →
 * body. Other platforms (LinkedIn, Twitter, Medium, …) are treated as profiles
 * since people rarely deep-link them as résumé project references.
 */
function isProfileShaped(url: string): boolean {
  let host: string;
  try {
    host = new URL(url).hostname.replace(/^www\./, '').toLowerCase();
  } catch {
    return false;
  }
  if (host === 'github.com' || host === 'gitlab.com') {
    return pathSegments(url).length <= 1;
  }
  return true;
}

/** A readable, visible label for a body link, preferring the human anchor. */
function bodyLabel(anchor: string, url: string): string {
  const a = anchor.trim();
  if (a && !/^https?:\/\//i.test(a) && !a.startsWith('mailto:')) return a;
  // Anchor is a raw URL (common in PDFs) — derive a name from the URL: a repo /
  // article slug (last meaningful path segment) reads better than the bare host.
  const segs = pathSegments(url);
  const last = segs[segs.length - 1];
  if (last && !/^\d+$/.test(last)) {
    const humanised = last
      .replace(/\.[a-z0-9]+$/i, '')
      .replace(/[-_]+/g, ' ')
      .trim();
    if (humanised) return humanised;
  }
  return urlToFriendlyLabel(url);
}

/** De-duplicate a body label so each injects to exactly one URL. */
function uniqueBodyLabel(label: string, used: Set<string>): string {
  let candidate = label;
  let n = 2;
  while (used.has(candidate.toLowerCase())) candidate = `${label} (${n++})`;
  used.add(candidate.toLowerCase());
  return candidate;
}

interface ClassifiedLinks {
  /** Profile / homepage links that belong on the header contact line. */
  contact: { label: string; url: string }[];
  /** Project / publication / portfolio links that belong on their own item (#18). */
  body: { label: string; url: string }[];
}

/**
 * Split the reference block into contact-line links vs body links (#18).
 *
 * - **Contact**: known platform *profiles* (LinkedIn, GitHub user, …) keep their
 *   brand label; the FIRST non-platform *bare-root* URL is admitted once under a
 *   generic "Website" label (the homepage/portfolio fix).
 * - **Body**: everything else — project repos (`github.com/u/repo`), article /
 *   DOI / publication links, and any additional personal sites. Previously these
 *   were dropped twice (by the PROFILE_DOMAINS allowlist + the "first non-platform
 *   only" Website rule, then stripped from the body), so academic project /
 *   publication URLs silently vanished. They are now preserved on their own items.
 *
 * `mailto:` is excluded here (handled separately as the clean email). Both the
 * prompt instructions and the post-generation injectors build on this, so the
 * label the AI is told to write and the label injection later looks for can never
 * drift.
 */
function classifyLinks(resume: string): ClassifiedLinks {
  const contact: { label: string; url: string }[] = [];
  const body: { label: string; url: string }[] = [];
  let websiteAdmitted = false;
  const usedBodyLabels = new Set<string>();

  for (const { anchor, url } of parseLinkBlock(resume)) {
    if (url.startsWith('mailto:')) continue;
    if (!/^https?:\/\//i.test(url)) continue;

    if (isProfileUrl(url) && isProfileShaped(url)) {
      // PDFs often store the raw URL as the anchor; derive the friendly label
      // (e.g. "LinkedIn") so injection matches what the AI writes.
      const label = /^https?:\/\//i.test(anchor) ? urlToFriendlyLabel(anchor) : anchor;
      contact.push({ label, url });
    } else if (!isProfileUrl(url) && isBareRoot(url) && !websiteAdmitted) {
      contact.push({ label: WEBSITE_LABEL, url });
      websiteAdmitted = true;
    } else {
      body.push({ label: uniqueBodyLabel(bodyLabel(anchor, url), usedBodyLabels), url });
    }
  }
  return { contact, body };
}

function resolveContactLinks(resume: string): { label: string; url: string }[] {
  return classifyLinks(resume).contact;
}

function resolveBodyLinks(resume: string): { label: string; url: string }[] {
  return classifyLinks(resume).body;
}

/**
 * Build a label→url map for the contact links in the extracted reference block.
 * Used for post-processing: replacing plain labels with [label](url) markdown.
 */
export function getLinkMap(resume: string): Record<string, string> {
  const map: Record<string, string> = {};
  for (const { label, url } of resolveContactLinks(resume)) {
    map[label] = url;
  }
  return map;
}

/**
 * Build a label→url map for the BODY links (projects, publications, portfolio)
 * extracted from the reference block (#18). Consumed only by the résumé injection
 * path — cover letters never carry body links.
 */
export function getBodyLinkMap(resume: string): Record<string, string> {
  const map: Record<string, string> = {};
  for (const { label, url } of resolveBodyLinks(resume)) {
    map[label] = url;
  }
  return map;
}

/** Escape a string for literal use inside a `RegExp`. */
function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** The candidate's email — the reliable signal for "this is the contact line". */
const CONTACT_EMAIL_RE = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/;
const SECTION_HEADER_RE = /^(PROFESSIONAL|WORK|EDUCATION|SKILLS|SUMMARY)/i;
/** An already-injected `[label](url)` span — protected so re-runs stay idempotent. */
const MD_LINK_SPAN_RE = /\[[^\]]+\]\([^)]+\)/g;

/**
 * Post-process AI-generated resume/cover-letter text: replace the short profile
 * labels the model wrote ("LinkedIn", "GitHub", "Website") in the contact line
 * with `[label](https://…)` markdown, so the Rust renderer can attach the
 * hyperlink without displaying the raw URL.
 *
 * The contact line is found by CONTENT, not position. Résumés keep it at the very
 * top, but cover letters place it below a marker / name / salutation — past any
 * fixed line window — which is why LinkedIn silently stayed unlinked in cover
 * letters (a résumé header and a cover-letter header share this same function).
 * We inject into every pipe-delimited line that carries the candidate's email
 * (the contact-line signal, wherever the model put it); the email guard keeps
 * body prose that merely mentions a platform untouched. Falls back to the first
 * pipe line bearing a known label when no email line is present. Idempotent: the
 * `(?<!\[)` lookbehind skips labels already inside a `[…]` link.
 *
 * `bodyMap` (#18) carries project / publication / portfolio links that belong to
 * specific résumé items, not the contact line — so they are injected ANYWHERE in
 * the body (every line), not gated to the contact line. Pass `{}` (the default)
 * for documents that carry no body links, e.g. cover letters.
 */
export function injectLinksIntoGeneratedText(
  text: string,
  linkMap: Record<string, string>,
  bodyMap: Record<string, string> = {}
): string {
  const contactLabels = Object.keys(linkMap);
  // Guard against over-matching short/common words against arbitrary prose.
  const bodyLabels = Object.keys(bodyMap).filter((l) => l.trim().length >= 3);
  if (!contactLabels.length && !bodyLabels.length) return text;

  const injectPlain = (segment: string, labels: string[], map: Record<string, string>): string => {
    let out = segment;
    for (const label of labels) {
      out = out.replace(
        new RegExp(`\\b${escapeRegExp(label)}\\b`, 'gi'),
        `[${label}](${map[label]})`
      );
    }
    return out;
  };
  // Inject into the plain text only, stepping over any pre-existing `[label](url)`
  // spans — so a label that recurs inside an already-injected URL (linkedin.com)
  // is never re-wrapped and the pass is idempotent.
  const inject = (line: string, labels: string[], map: Record<string, string>): string => {
    let out = '';
    let last = 0;
    for (const m of line.matchAll(MD_LINK_SPAN_RE)) {
      const idx = m.index ?? 0;
      out += injectPlain(line.slice(last, idx), labels, map) + m[0];
      last = idx + m[0].length;
    }
    return out + injectPlain(line.slice(last), labels, map);
  };
  const hasLabel = (line: string): boolean =>
    contactLabels.some((l) => new RegExp(`(?<!\\[)\\b${escapeRegExp(l)}\\b`, 'i').test(line));
  const isContactCandidate = (line: string): boolean =>
    line.includes('|') && !SECTION_HEADER_RE.test(line.trim());

  const lines = text.split('\n');

  // 1) Contact links — only the contact line (pipe-delimited, carries the email).
  if (contactLabels.length) {
    let injected = false;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i] ?? '';
      if (isContactCandidate(line) && CONTACT_EMAIL_RE.test(line)) {
        lines[i] = inject(line, contactLabels, linkMap);
        injected = true;
      }
    }
    if (!injected) {
      const i = lines.findIndex((l) => isContactCandidate(l) && hasLabel(l));
      if (i !== -1) lines[i] = inject(lines[i] ?? '', contactLabels, linkMap);
    }
  }

  // 2) Body links (#18) — kept on their own items, so inject them anywhere in the
  // body (idempotent; stepping over already-linked spans).
  if (bodyLabels.length) {
    for (let i = 0; i < lines.length; i++) {
      lines[i] = inject(lines[i] ?? '', bodyLabels, bodyMap);
    }
  }

  return lines.join('\n');
}

/**
 * Parse the markdown reference block appended by the Rust PDF/DOCX extractor.
 * Returns a prompt injection block telling the AI to write short labels
 * (LinkedIn, GitHub) — not full URLs. Actual hyperlinks are injected
 * post-generation by injectLinksIntoGeneratedText().
 */
export function parseLinksFromResume(resume: string): ParsedResumeLinks {
  const entries = parseLinkBlock(resume);
  if (!entries.length) return { block: '', cleanEmail: '' };

  const mailto = entries.find((e) => e.url.startsWith('mailto:'));
  const cleanEmail = mailto ? mailto.url.slice('mailto:'.length) : '';

  // Exactly the labels (platform brands + one "Website") getLinkMap() will inject,
  // so the AI is instructed to write the same short labels we later hyperlink.
  const labelEntries = resolveContactLinks(resume).map((e) => e.label);

  if (!labelEntries.length && !cleanEmail) return { block: '', cleanEmail: '' };

  const parts: string[] = [];
  if (cleanEmail) {
    parts.push(`CANDIDATE EMAIL (use this exact address, no spaces): ${cleanEmail}`);
  }
  if (labelEntries.length) {
    parts.push(
      `CANDIDATE PROFILE LINKS — write ONLY these short labels in the contact line (NOT the full URL):\n` +
        labelEntries.join(', ') +
        `\nExample: Haarlem, Netherlands | name@example.com | +31... | LinkedIn | GitHub | Website`
    );
  }

  return { block: parts.join('\n\n'), cleanEmail };
}

/**
 * Build a prompt instruction for the candidate's BODY links — project, article,
 * publication and portfolio URLs that belong to specific résumé items rather than
 * the contact line (#18). The block regime (PDF/RTF) strips these before the
 * model ever sees them, so without re-surfacing them here they are silently
 * dropped (the original academic-link bug). The model is told to keep each label
 * on its matching item — adding a PROJECTS / PUBLICATIONS section if an item has
 * no natural home — and to write ONLY the short label; the real hyperlink is
 * injected post-generation by injectLinksIntoGeneratedText() via getBodyLinkMap().
 * Returns '' when there are no body links.
 */
export function buildBodyLinksBlock(resume: string): string {
  const body = resolveBodyLinks(resume);
  if (!body.length) return '';
  const labels = body.map((b) => `- ${b.label}`).join('\n');
  return (
    `CANDIDATE PROJECT / PUBLICATION LINKS — each of these belongs to a specific item in the ` +
    `résumé (a project, publication, or portfolio piece), NOT the contact line. Keep every one ` +
    `of these labels as visible text on its matching item; write ONLY the short label, never the ` +
    `full URL (the link is attached automatically). If an item has no natural home in Experience ` +
    `or Skills, add a PROJECTS or PUBLICATIONS section and list it there. Drop none of them:\n` +
    labels
  );
}

/**
 * Strip the link reference block from resume text before sending to the AI
 * so the body text budget is not wasted on the reference list.
 */
export function stripLinkBlock(resume: string): string {
  const sep = resume.lastIndexOf('\n---\n');
  return sep === -1 ? resume : resume.slice(0, sep);
}
