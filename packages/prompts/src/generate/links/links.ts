/**
 * Resume contact-link extraction + post-generation hyperlink injection.
 *
 * The Rust PDF/DOCX extractor appends a `\n---\n` markdown reference block of
 * `[anchor](url)` entries. We turn that into (a) a prompt instruction telling the
 * AI to write short labels (LinkedIn, GitHub), and (b) a post-generation injector
 * that replaces those labels with real markdown links.
 */

import { detectSections, type ResumeSection } from '../../context-manager/sections.js';
import { isAllCapsSectionHeading, isKnownSectionName } from '../text/header-contact-line.js';

// Known social/portfolio domains that belong in a resume contact line.
// `about.me`/`carrd.co` added for parity with Rust WEBSITE_HOSTS (#L1,
// contact_profile/mod.rs:558-565) — link-in-bio hosts alongside the other
// four already here (solo.to, bio.link, linktr.ee, bento.me). `xing.com`
// added deliberately (#LOW, xing.com is also in JOB_BOARD_HOSTS below — a
// DACH candidate's personal Xing profile, gated to `/profile/` the same way
// LinkedIn is gated to `/in/`, must resolve to a proper contact entry, not
// silently drop just because the host is job-board-adjacent, and not a
// fabricated "Xing" project).
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
  'about.me',
  'carrd.co',
  'xing.com',
];

/**
 * Hosts that are job boards / aggregators / employer ATS — never a personal
 * contact link and never a project either. Mirrors Rust `JOB_BOARD_HOSTS`
 * (`contact_profile/mod.rs`). Used both to keep one off the "Website"
 * apex/first-seen pre-pass (#L1) and to drop it entirely in `classifyLinks`'s
 * main loop rather than letting it fall through to `body` (#HIGH-3 — a
 * fabrication risk of the same shape #M6 closed for non-personal LinkedIn).
 * `xing.com` is listed here AND in `PROFILE_DOMAINS` — see `isPersonalXing`.
 */
const JOB_BOARD_HOSTS = [
  'indeed.com',
  'glassdoor.com',
  'stepstone.de',
  'stepstone.com',
  'monster.com',
  'ziprecruiter.com',
  'lever.co',
  'greenhouse.io',
  'workday.com',
  'myworkdayjobs.com',
  'ashbyhq.com',
  'smartrecruiters.com',
  'recruitee.com',
  'personio.de',
  'arbeitnow.com',
  'xing.com',
];

function isJobBoard(url: string): boolean {
  const host = hostOf(url);
  return host !== null && JOB_BOARD_HOSTS.some((d) => host === d || host.endsWith(`.${d}`));
}

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
    // Exact-or-subdomain match. `host.startsWith('linkedin.com')` is unsafe —
    // `linkedin.com.evil.com` would match — so compare the host exactly or as a
    // subdomain of the brand domain (js/incomplete-url-substring-sanitization).
    const hostIs = (h: string, d: string) => h === d || h.endsWith('.' + d);
    if (hostIs(host, 'linkedin.com')) return 'LinkedIn';
    if (hostIs(host, 'github.com')) return 'GitHub';
    if (hostIs(host, 'gitlab.com')) return 'GitLab';
    if (hostIs(host, 'twitter.com') || hostIs(host, 'x.com')) return 'Twitter';
    if (hostIs(host, 'behance.net')) return 'Behance';
    if (hostIs(host, 'dribbble.com')) return 'Dribbble';
    if (hostIs(host, 'medium.com')) return 'Medium';
    if (hostIs(host, 'stackoverflow.com')) return 'Stack Overflow';
    if (hostIs(host, 'dev.to')) return 'Dev.to';
    if (hostIs(host, 'codepen.io')) return 'CodePen';
    if (hostIs(host, 'youtube.com') || hostIs(host, 'youtu.be')) return 'YouTube';
    if (hostIs(host, 'notion.so')) return 'Notion';
    if (hostIs(host, 'figma.com')) return 'Figma';
    if (hostIs(host, 'npmjs.com')) return 'npm';
    if (hostIs(host, 'crates.io')) return 'crates.io';
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

/** `www.`-stripped, lowercased hostname, or null on parse failure. */
function hostOf(url: string): string | null {
  try {
    return new URL(url).hostname.replace(/^www\./, '').toLowerCase();
  } catch {
    return null;
  }
}

/** Is this URL's host `linkedin.com` (or a subdomain of it)? */
function isLinkedinHost(url: string): boolean {
  const host = hostOf(url);
  return host === 'linkedin.com' || (host?.endsWith('.linkedin.com') ?? false);
}

/**
 * A personal LinkedIn profile is `/in/…`. A company (`/company/…`), school
 * (`/school/…`) or job (`/jobs/…`) page is shape-indistinguishable but must
 * never seed the contact line — or a fabricated body item (#M6, see the
 * exclusion in `classifyLinks`). Mirrors Rust `is_personal_linkedin`
 * (`contact_profile/mod.rs`).
 */
function isPersonalLinkedin(url: string): boolean {
  return isLinkedinHost(url) && url.toLowerCase().includes('/in/');
}

/** Is this URL's host `xing.com` (or a subdomain of it)? */
function isXingHost(url: string): boolean {
  const host = hostOf(url);
  return host === 'xing.com' || (host?.endsWith('.xing.com') ?? false);
}

/**
 * A personal Xing profile is `/profile/…` — same gate shape as LinkedIn's
 * `/in/` (#LOW, deliberate): `xing.com` is also a `JOB_BOARD_HOSTS` entry
 * (Xing hosts job listings too), so without this gate a personal profile
 * link would either be dropped alongside real job postings or, pre-#HIGH-3,
 * fabricated into a "Xing" project — neither is right for a DACH candidate's
 * actual professional-network identity.
 */
function isPersonalXing(url: string): boolean {
  return isXingHost(url) && url.toLowerCase().includes('/profile/');
}

/**
 * Is this platform URL a *profile* (belongs on the contact line) rather than a
 * deep link to a specific repo/article (which belongs on its own body item)?
 * `github.com/<user>` is a profile; `github.com/<user>/<repo>` is a project →
 * body. Other platforms (LinkedIn, Twitter, Medium, …) are treated as profiles
 * since people rarely deep-link them as résumé project references — except
 * LinkedIn and Xing, which keep a stricter path gate instead (`/in/`,
 * `/profile/`): a company/school/job page is otherwise indistinguishable by
 * shape but must never seed the header (mirrors Rust `is_platform_profile_link`
 * for LinkedIn; the Xing gate has no Rust counterpart yet — see #LOW).
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
  if (host === 'linkedin.com' || host.endsWith('.linkedin.com')) {
    return isPersonalLinkedin(url);
  }
  if (host === 'xing.com' || host.endsWith('.xing.com')) {
    return isPersonalXing(url);
  }
  return true;
}

/**
 * Among bare-root, non-platform candidate URLs, decide which one is admitted
 * as the single "Website" contact link — order-independent (#A parity with
 * Rust `classify_contact_links`'s `apex_pick`/`first_pick`,
 * `contact_profile/mod.rs`): a host that is the apex of another candidate
 * host in this same document (e.g. `example.dev` beside `blog.example.dev`)
 * wins over every standalone candidate; among hosts tied on that signal,
 * first-seen decides. The subdomain check is dot-prefixed
 * (`host.endsWith('.' + other)`) — a naive substring `endsWith` would wrongly
 * treat `notexample.dev` as a subdomain of `example.dev`.
 */
function pickWebsiteUrl(candidates: { host: string; url: string }[]): string | null {
  const hosts = candidates.map((c) => c.host);
  const isSubdomainOfAnother = (host: string): boolean =>
    hosts.some((o) => o !== host && host.endsWith(`.${o}`));
  const isApexOfAnother = (host: string): boolean =>
    hosts.some((o) => o !== host && o.endsWith(`.${host}`));

  const apexPick = candidates.find((c) => !isSubdomainOfAnother(c.host) && isApexOfAnother(c.host));
  const firstPick = candidates.find((c) => !isSubdomainOfAnother(c.host));
  return (apexPick ?? firstPick)?.url ?? null;
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

/**
 * De-duplicate a body label so each injects to exactly one URL — de-dupes on
 * the NORMALIZED key (#M4), not the lowercased literal, so two differently
 * written anchors that key-collide ("CrossKit" / "Cross-Kit") don't end up as
 * separate entries competing for the same line by document-order accident
 * (the HIGH-1 URL-swap symptom surviving for this shape, since both would
 * title-match with an identical score). The disambiguator suffix is a plain
 * number, never parens (#M5) — `\b…\b` cannot match a `)`-terminated label
 * (`)` isn't a word character), so the old "(2)" suffix made a numbered
 * duplicate unlinkable by ANY phrasing, verbatim or renamed.
 */
function uniqueBodyLabel(label: string, used: Set<string>): string {
  let candidate = label;
  let n = 2;
  while (used.has(normalizeKey(candidate))) candidate = `${label} ${n++}`;
  used.add(normalizeKey(candidate));
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
 * - **Contact**: known platform *profiles* (LinkedIn `/in/`, GitHub user, …)
 *   keep their brand label; among bare-root, non-platform candidates, one is
 *   admitted once under a generic "Website" label (the homepage/portfolio
 *   fix) — the apex host wins over any candidate that is one of its own
 *   subdomains, order-independent, first-seen only breaking a genuine tie
 *   (`pickWebsiteUrl`, #A parity with Rust `classify_contact_links`).
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
  const usedBodyLabels = new Set<string>();
  const entries = parseLinkBlock(resume);

  // Pre-pass: which bare-root, non-platform, non-job-board URL (if any) wins
  // the "Website" slot — decided up front so admission below doesn't depend
  // on document order. `!isJobBoard` mirrors Rust's `!is_job_board` filter at
  // this exact point (#L1) — a job-board apex like indeed.com must never
  // become the "Website" contact link. The scheme check is CASE-SENSITIVE
  // (`startsWith`, not a case-insensitive regex) — Rust's mirrored
  // `classify_contact_links` pre-pass uses a plain `starts_with("http://") ||
  // starts_with("https://")`, with no lowercasing; a case-insensitive check
  // here would admit an "HTTPS://…" candidate Rust's own pre-pass never would
  // (LOW, security re-review — same-class divergence as everything else
  // fixed on this branch).
  const websiteCandidates: { host: string; url: string }[] = [];
  for (const { url } of entries) {
    if (url.startsWith('mailto:')) continue;
    if (!(url.startsWith('http://') || url.startsWith('https://'))) continue;
    if (isProfileUrl(url) || !isBareRoot(url) || isJobBoard(url)) continue;
    const host = hostOf(url);
    if (host) websiteCandidates.push({ host, url });
  }
  const websiteUrl = pickWebsiteUrl(websiteCandidates);
  let websiteAdmitted = false;

  for (const { anchor, url } of entries) {
    if (url.startsWith('mailto:')) continue;
    if (!/^https?:\/\//i.test(url)) continue;

    if (isProfileUrl(url) && isProfileShaped(url)) {
      // PDFs often store the raw URL as the anchor; derive the friendly label
      // (e.g. "LinkedIn") so injection matches what the AI writes.
      const label = /^https?:\/\//i.test(anchor) ? urlToFriendlyLabel(anchor) : anchor;
      contact.push({ label, url });
    } else if (!isProfileUrl(url) && isBareRoot(url) && url === websiteUrl && !websiteAdmitted) {
      contact.push({ label: WEBSITE_LABEL, url });
      websiteAdmitted = true;
    } else if (isLinkedinHost(url) && !isPersonalLinkedin(url)) {
      // An employer/school/job LinkedIn page — shape-indistinguishable from a
      // personal profile but must never seed a fabricated PROJECTS item
      // either (#M6): buildBodyLinksBlock tells the model to invent a
      // section for any body entry with no natural home, so letting this
      // through would turn an employer's LinkedIn page into a résumé
      // "project". Mirrors Rust classify_contact_links, which drops these
      // entirely.
      continue;
    } else if (isJobBoard(url)) {
      // A job board / aggregator / employer ATS link (Indeed, a Greenhouse
      // apply page, …) — the same fabrication risk as the LinkedIn case
      // right above (#HIGH-3): previously this fell through to `body`,
      // demoted from a *prevented* fake "Website" into a fake "Indeed"/
      // "Apply" project instead. Never a contact link, never a project.
      continue;
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

/**
 * Longest-first — so the literal-fallback injector (`inject`/`injectOne` in
 * `injectLinksIntoGeneratedText`) claims a more specific label's text before
 * a shorter label that happens to be its literal prefix gets a chance to
 * (e.g. the #M4/#M5 disambiguator shape "CrossKit" / "CrossKit 2").
 */
function byLengthDesc(labels: string[]): string[] {
  return [...labels].sort((a, b) => b.length - a.length);
}

/**
 * Bullets/plain whitespace (`•`, `-`, `*`, space) OR a genuine ordered-list
 * marker (digits immediately followed by `.`/`)`, then whitespace) stripped
 * before body-title matching. Deliberately does NOT treat a bare leading
 * digit run as a marker unless it is actually followed by `.`/`)` +
 * whitespace — otherwise a title that starts with a digit ("3D Printing
 * Pipeline", "2048 Game Engine", "500px Clone Gallery") loses its leading
 * digit(s) to the strip and can never title-match (#M1).
 */
function stripLeadingMarker(line: string): number {
  let i = 0;
  while (i < line.length && /[\s•*-]/.test(line[i] ?? '')) i++;
  const marker = /^\d+[.)]\s+/.exec(line.slice(i));
  if (marker) {
    i += marker[0].length;
    while (i < line.length && /[\s•*-]/.test(line[i] ?? '')) i++;
  }
  return i;
}
/**
 * Characters normalizeKey / title matching treat as insignificant separators
 * — anything that is not a Unicode letter or digit, so punctuation
 * (apostrophes, colons, parens, commas, …) doesn't break a match, not just
 * hyphen/underscore/whitespace (#M2 — "Jane's Portfolio"/"janes-portfolio",
 * "CrossKit (v2)"/"crosskit-v2", "CrossKit: The Toolkit"/"crosskit-the-toolkit").
 */
const SEPARATOR_CHAR_RE = /[^\p{L}\p{N}]/u;
/** A Unicode letter or digit — used to require a real word boundary. */
const WORD_CHAR_RE = /[\p{L}\p{N}]/u;
/** Anchored check: does `s` begin with an existing `[label](url)` markdown span? */
const LEADING_MD_LINK_RE = /^\[[^\]]{1,200}\]\([^)]{1,2000}\)/;
/**
 * Prefix-match floor for body-title matching (#C) — below this, short labels
 * collide too easily with unrelated text (e.g. "Goth" inside "Gotham City Guide").
 */
const MIN_TITLE_KEY_LEN = 6;

/**
 * Lowercase, accent-folded key with every non-letter/non-digit character
 * stripped (#M2, symmetric with `SEPARATOR_CHAR_RE`) — "ai-job-hunter-app",
 * "ai job hunter app", and "AI Job Hunter" all normalize to the same value, so
 * a body label survives whichever spelling the PDF extractor or the model
 * happened to produce. NFD + combining-mark strip folds accents ("Café" /
 * "cafe") so real-name titles in accented languages still match their slug.
 */
function normalizeKey(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '');
}

/**
 * Fold ONE character's accents (NFD decompose + strip its combining marks),
 * lowercased — "é" → "e", "ü" → "u" — so the leading-title WALK below (which
 * must stay one-original-character-per-step to keep slicing correct) matches
 * an accented real-name title against an ASCII slug label symmetrically with
 * `normalizeKey` above. Folding a single precomposed letter is 1:1 for every
 * realistic case; falls back to the plain lowercase on the rare empty result.
 */
function foldChar(ch: string): string {
  const folded = ch
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .toLowerCase();
  return folded.charAt(0) || ch.toLowerCase();
}

/**
 * English section-header words the prompt can produce even outside ALL CAPS
 * (only PROJECTS/PUBLICATIONS are guaranteed always-English per resume.ts;
 * the rest are extra safety) — checked case-insensitively as the WHOLE
 * trimmed line (optionally colon-terminated), never as part of a longer
 * title (#M3 — the ALL-CAPS check alone misses Title-Case "Projects").
 */
const KNOWN_SECTION_HEADER_WORDS = new Set([
  'projects',
  'publications',
  'summary',
  'professional summary',
  'work experience',
  'experience',
  'education',
  'skills',
  'certifications',
]);

/**
 * A bare section-header line ("PROJECTS", "SUMMARY", "ZUSAMMENFASSUNG", …) —
 * ALL CAPS (locale-agnostic), or a known English header word in any casing
 * (#M3) — so it is never itself a candidate item title.
 */
function isSectionHeaderLine(line: string): boolean {
  const t = line.trim();
  if (!t) return false;
  if (/\p{Lu}/u.test(t) && !/\p{Ll}/u.test(t)) return true;
  return KNOWN_SECTION_HEADER_WORDS.has(t.replace(/:$/, '').trim().toLowerCase());
}

/**
 * If the match stopped right before a closing bracket/paren whose opener was
 * already consumed inside the matched span (skipped as an insignificant
 * separator, #M2's widened class), extend `end` by one to include it — so
 * "CrossKit (v2)" wraps as a clean, bracket-balanced title instead of
 * leaving a dangling `)` outside the link (`[CrossKit (v2](url))`).
 */
function extendPastDanglingCloser(line: string, start: number, end: number): number {
  const closer = line[end];
  if (closer !== ')' && closer !== ']' && closer !== '}') return end;
  const opener = closer === ')' ? '(' : closer === ']' ? '[' : '{';
  const span = line.slice(start, end);
  const opens = span.split(opener).length - 1;
  const closes = span.split(closer).length - 1;
  return opens > closes ? end + 1 : end;
}

/**
 * If `end` lands between a UTF-16 surrogate pair's two halves, back it off by
 * one unit so a slice never emits an unpaired surrogate — invalid for
 * JSON/serde, and the plausible blast radius is an IPC/save failure.
 */
function backOffSurrogateSplit(line: string, end: number): number {
  if (end <= 0 || end >= line.length) return end;
  const hi = line.charCodeAt(end - 1);
  const lo = line.charCodeAt(end);
  const isHigh = hi >= 0xd800 && hi <= 0xdbff;
  const isLow = lo >= 0xdc00 && lo <= 0xdfff;
  return isHigh && isLow ? end - 1 : end;
}

interface TitleSpan {
  start: number;
  end: number;
  /** Higher wins the greedy assignment — see injectLinksIntoGeneratedText. */
  score: number;
}

/**
 * Test whether `label`'s normalized key matches the leading title of `line`
 * (#B/#C) — the model now writes the item's real name ("AI Job Hunter"), not
 * the machine label, which may be a URL-derived slug ("ai-job-hunter-app") or
 * its humanised PDF-extraction form ("ai job hunter app"). Walks `line`'s
 * significant characters (skipping a leading bullet/number marker and any
 * non-letter/non-digit separator) alongside the label's normalized key.
 *
 * A character MISMATCH is always rejected outright — never accepted just
 * because enough characters happened to match first. Only two endings count
 * as a real match: the full label key is consumed (and the line, if it
 * continues, does so at a genuine word boundary — "toolkit" must not match
 * inside "toolkits"), or the line's own title is fully consumed as a genuine
 * (>= MIN_TITLE_KEY_LEN) prefix of the label. This is what stops a
 * coincidental overlap ("gotham city guide" inside "Gothamburg Transit Map")
 * from cross-linking the wrong item.
 *
 * Returns null for a bare section-header line, a line already carrying a
 * markdown link at the title position (idempotency), a span that would
 * cross a `[`/`]` (the widened separator class, #M2, would otherwise let a
 * match skip straight across them — see the bracket check below), or no
 * match.
 */
function matchLineTitle(line: string, label: string): TitleSpan | null {
  if (isSectionHeaderLine(line)) return null;
  const start = stripLeadingMarker(line);
  if (LEADING_MD_LINK_RE.test(line.slice(start))) return null;

  const labelKey = normalizeKey(label);
  if (labelKey.length < MIN_TITLE_KEY_LEN) return null;

  let li = start;
  let matchedLen = 0;
  let end = start;
  while (li < line.length && matchedLen < labelKey.length) {
    const ch = line[li] ?? '';
    if (SEPARATOR_CHAR_RE.test(ch)) {
      li++;
      continue;
    }
    if (foldChar(ch) !== labelKey[matchedLen]) return null;
    matchedLen++;
    li++;
    end = li;
  }
  if (matchedLen < MIN_TITLE_KEY_LEN) return null;

  const lineExhausted = li >= line.length;
  const labelExhausted = matchedLen === labelKey.length;
  if (labelExhausted && !lineExhausted) {
    const next = line[end] ?? '';
    if (WORD_CHAR_RE.test(next)) return null; // mid-word — e.g. "toolkits"
  }

  end = extendPastDanglingCloser(line, start, end);
  end = backOffSurrogateSplit(line, end);
  // Never wrap a span containing `[`/`]` — MD_LINK_SPAN_RE (and the Rust
  // renderer's MD_LINK_RE, model/rich.rs:33-34) can't parse nested brackets,
  // and the widened separator class (#M2) would otherwise let a match skip
  // straight across them: "CrossKit [beta] Toolkit" → the broken
  // `[CrossKit [beta] Toolkit](url)` (#MEDIUM).
  const span = line.slice(start, end);
  if (span.includes('[') || span.includes(']')) return null;
  const exact = lineExhausted && labelExhausted;
  const score = matchedLen * 4 + (exact ? 2 : labelExhausted ? 1 : 0);
  return { start, end, score };
}

/**
 * The candidate's email — the reliable signal for "this is the contact line".
 * Length-capped to a linear form (js/polynomial-redos): the previous nested
 * `(?:\.[…]+)*\.[A-Za-z]{2,}` shape let the inner `+` overlap the trailing
 * literal-dot segment, re-partitioning on backtrack (still quadratic). The
 * segments are now bounded — local-part ≤64, domain ≤255, TLD ≤24 (the RFC-ish
 * upper bounds for real addresses) — so matching is linear-time. This guards the
 * `isContactCandidate` lines, which carry no length cap of their own.
 */
const CONTACT_EMAIL_RE = /[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]{1,255}\.[A-Za-z]{2,24}/;
const SECTION_HEADER_RE = /^(PROFESSIONAL|WORK|EDUCATION|SKILLS|SUMMARY)/i;
/**
 * An already-injected `[label](url)` span — protected so re-runs stay idempotent.
 * Quantifiers are bounded (js/polynomial-redos): a real markdown label/URL is far
 * shorter than these limits, so bounding cannot drop a genuine span, but it caps
 * the regex's worst-case work on adversarial input.
 */
const MD_LINK_SPAN_RE = /\[[^\]]{1,200}\]\([^)]{1,2000}\)/g;

/** A markdown link span anywhere in a string (non-global, safe for `.test`). */
const HAS_MD_LINK_RE = /\[[^\]]{1,200}\]\([^)]{1,2000}\)/;

/**
 * Is `line` shaped like an item TITLE — not a nested/indented description
 * line, and not a full sentence (#HIGH-1)? The last-resort net's pairing
 * step must never treat a description bullet of an already-linked project,
 * or a prose sentence, as an "open slot" for a different, unrelated label.
 * Also refuses a line containing `[`/`]` (#MEDIUM) — pairing wraps the
 * line's own raw text, so a bracket inside it would produce the same broken
 * nested-bracket markdown the bracket check in `matchLineTitle` exists to
 * prevent.
 *
 * MEDIUM (security re-review): a single top-level bullet marker ("- Fleet
 * Tracker", "• Fleet Tracker") is stripped and the REMAINDER tested — many
 * résumés format project TITLES themselves as a flat bulleted list, not just
 * their descriptions, so flatly rejecting every marked line made this pool
 * unreachable for that (common) shape. Only genuine nesting — leading
 * whitespace/indentation before the marker, the actual textual signal of a
 * sub-point under a parent bullet — is still rejected as a description.
 * `unlinkedItemLineIndices`'s caller already slices on
 * `stripLeadingMarker`'s own index when splicing the link in, so a
 * top-level-bulleted title's marker is preserved untouched either way.
 */
function isItemShapedLine(line: string): boolean {
  if (/^\s/.test(line)) return false; // indented — nested under a parent bullet, a description
  const markerEnd = stripLeadingMarker(line);
  const trimmed = line.slice(markerEnd).trim();
  if (!trimmed || /[.!?]\s*$/.test(trimmed)) return false; // sentence-final punctuation
  if (trimmed.includes('[') || trimmed.includes(']')) return false;
  const words = trimmed.split(/\s+/).filter(Boolean);
  return words.length > 0 && words.length <= 8;
}

/**
 * Line indices, inside a detected PROJECTS/PUBLICATIONS `ResumeSection`, that
 * are item-shaped and carry no link yet — the pool the HIGH-part-2
 * last-resort net draws from when exactly one label is still unmatched after
 * both the title-match and literal-fallback passes (the renamed-item case,
 * e.g. "orbit-sim" written as "Orbital Simulator", which is only knowable
 * after generation — prompt partitioning can't fix it).
 */
function unlinkedItemLineIndices(lines: string[], sections: ResumeSection[]): number[] {
  const indices: number[] = [];
  for (const section of sections) {
    for (let i = section.startIndex + 1; i <= section.endIndex; i++) {
      const line = lines[i] ?? '';
      if (!line.trim() || HAS_MD_LINK_RE.test(line)) continue;
      if (isItemShapedLine(line)) indices.push(i);
    }
  }
  return indices;
}

/**
 * The line index right after a section's last non-blank content line — or
 * right after its header if the section has no content — the splice point
 * for appending a new item (#HIGH-2, never a bare `lines.push()` at document
 * end with no section context).
 */
function sectionInsertionPoint(lines: string[], section: ResumeSection): number {
  for (let i = section.endIndex; i > section.startIndex; i--) {
    if ((lines[i] ?? '').trim()) return i + 1;
  }
  return section.startIndex + 1;
}

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
  const contactLabels = byLengthDesc(Object.keys(linkMap));
  // Any non-empty body label is a candidate for SOME step below (#MEDIUM —
  // a `>= 3` floor here used to drop a label like "Go" before it ever got a
  // chance, even though buildBodyLinksBlock's SHORT KEYS partition explicitly
  // asked the model to write it verbatim). The floor that actually matters
  // for the risky literal-regex fallback (over-matching a common short word
  // against arbitrary prose) is applied there instead, not at intake.
  const bodyLabels = Object.keys(bodyMap).filter((l) => l.trim().length > 0);
  if (!contactLabels.length && !bodyLabels.length) return text;

  // `preserveCase` wraps the text AS WRITTEN rather than substituting the
  // stored label's own spelling/casing — used only for the body-link fallback
  // (#C low-priority): the model now writes the item's real name (#B), which
  // may differ in case from the machine label even where it still matches
  // literally. Contact injection always uses the default (brand-cased label:
  // LinkedIn, GitHub) — unchanged.
  const injectPlain = (
    segment: string,
    labels: string[],
    map: Record<string, string>,
    preserveCase = false
  ): string => {
    let out = segment;
    for (const label of labels) {
      out = out.replace(
        new RegExp(`\\b${escapeRegExp(label)}\\b`, 'gi'),
        (m) => `[${preserveCase ? m : label}](${map[label]})`
      );
    }
    return out;
  };
  // Inject one label at a time, re-scanning for `[text](url)` spans FRESH
  // before each label — both pre-existing spans AND ones a previous label in
  // this same pass just inserted. Without the re-scan, a shorter label that
  // is a literal prefix of another ("CrossKit" vs the #M4/#M5 disambiguator
  // "CrossKit 2") could match INSIDE the sibling label's freshly-wrapped
  // span, nesting brackets; callers additionally sort labels longest-first
  // (see `byLengthDesc`) so the more specific label claims its text before a
  // shorter prefix gets a chance to consume part of it. Idempotent both
  // across calls and within one pass.
  const injectOne = (
    line: string,
    label: string,
    map: Record<string, string>,
    preserveCase: boolean
  ): string => {
    let out = '';
    let last = 0;
    for (const m of line.matchAll(MD_LINK_SPAN_RE)) {
      const idx = m.index ?? 0;
      out += injectPlain(line.slice(last, idx), [label], map, preserveCase) + m[0];
      last = idx + m[0].length;
    }
    return out + injectPlain(line.slice(last), [label], map, preserveCase);
  };
  const inject = (
    line: string,
    labels: string[],
    map: Record<string, string>,
    preserveCase = false
  ): string => {
    let out = line;
    for (const label of labels) out = injectOne(out, label, map, preserveCase);
    return out;
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

  // 2) Body links (#18) — kept on their own items. The model now writes the
  // item's real title (#B), not the machine label, so first try a
  // normalized-key match against each line's leading title (#C — handles the
  // dashed slug / humanised-PDF-label mismatch); anything a title match
  // misses falls to the literal-label fallback (only actually reachable for
  // the short keys buildBodyLinksBlock still asks the model to echo
  // verbatim, #HIGH part 1); anything STILL unmatched after that goes
  // through the last-resort net below. That net's guarantee is PLACEMENT,
  // not PRESENCE (#HIGH part 2) — a link with no legitimate PROJECTS/
  // PUBLICATIONS-section home is left unplaced, never fabricated into an
  // unrelated line or force-appended with no section context, because
  // visible fabricated content in an employer-facing document is worse than
  // a missing link.
  if (bodyLabels.length) {
    // A label whose URL is already linked somewhere in the text is done — caps
    // injection to once per label per document and keeps repeat invocations a
    // true no-op even after a line that used to match becomes already-linked
    // (idempotency: a naive per-line re-match would let that label attach to a
    // different, weaker-matching line on the second pass).
    const linkedUrls = new Set<string>();
    for (const m of text.matchAll(MD_LINK_SPAN_RE)) {
      // Sliced, not matched: `/\]\(([^)]*)\)$/` is unanchored at the start, so on
      // a span full of `](` it retries every one of them and degrades to O(n²)
      // (CodeQL js/polynomial-redos). The span always ends `](url)`, so the last
      // `](` is the only candidate — one scan, no backtracking.
      const open = m[0].lastIndexOf('](');
      if (open !== -1 && m[0].endsWith(')')) {
        const url = m[0].slice(open + 2, -1);
        if (url) linkedUrls.add(url);
      }
    }
    const remaining = new Map(
      bodyLabels
        .filter((l) => !linkedUrls.has(bodyMap[l] ?? ''))
        .map((l) => [l, bodyMap[l] ?? ''] as const)
    );

    if (remaining.size) {
      // Score every (line, label) pair, then greedily assign each label to its
      // single highest-scoring line — never "first match wins", which silently
      // swapped URLs between sibling items (a repo and its own live site both
      // named for the same project).
      const candidates: { lineIndex: number; label: string; url: string; span: TitleSpan }[] = [];
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i] ?? '';
        for (const [label, url] of remaining) {
          const span = matchLineTitle(line, label);
          if (span) candidates.push({ lineIndex: i, label, url, span });
        }
      }
      candidates.sort((a, b) => b.span.score - a.span.score);

      const usedLines = new Set<number>();
      for (const c of candidates) {
        if (usedLines.has(c.lineIndex) || !remaining.has(c.label)) continue;
        const line = lines[c.lineIndex] ?? '';
        const { start, end } = c.span;
        lines[c.lineIndex] =
          line.slice(0, start) + `[${line.slice(start, end)}](${c.url})` + line.slice(end);
        usedLines.add(c.lineIndex);
        remaining.delete(c.label);
      }
    }

    if (remaining.size) {
      // The literal `\b<label>\b` regex risks over-matching a short/common
      // word against arbitrary prose, so only attempt it for labels with
      // some real specificity (#MEDIUM) — a 1-2 char label like "Go" skips
      // straight to the last-resort net below instead, never risking a
      // false match on ordinary prose that happens to contain the word.
      const fallbackCandidates = [...remaining].filter(([label]) => label.trim().length >= 3);
      if (fallbackCandidates.length) {
        const fallbackLabels = byLengthDesc(fallbackCandidates.map(([label]) => label));
        const fallbackMap = Object.fromEntries(fallbackCandidates);
        for (let i = 0; i < lines.length; i++) {
          lines[i] = inject(lines[i] ?? '', fallbackLabels, fallbackMap, /* preserveCase */ true);
        }
        // Which of those attempts actually landed — the regex only fires if
        // the model echoed the label verbatim, which buildBodyLinksBlock now
        // only asks for on short (< MIN_TITLE_KEY_LEN) keys (#HIGH part 1).
        // A longer key the model renamed (e.g. "orbit-sim" written as
        // "Orbital Simulator") never will (#HIGH part 2).
        for (const [label, url] of fallbackCandidates) {
          if (lines.some((l) => l.includes(`](${url})`))) remaining.delete(label);
        }
      }
    }

    // Last-resort net (#HIGH part 2). The guarantee here is PLACEMENT, not
    // PRESENCE: a link with nowhere legitimate to go is left unplaced rather
    // than fabricated into the wrong spot or force-appended with no section
    // context — a missing link is a smaller defect than visible fabricated
    // content in an employer-facing document. Located via the same
    // locale-aware SECTION_LEXICON `detectSections()` uses elsewhere in this
    // package, never an English-only regex — otherwise this whole net is
    // unreachable for every non-English résumé (PROJEKTE, PROJETS,
    // PROYECTOS, …).
    if (remaining.size) {
      const sections = detectSections(lines.join('\n'))
        .filter((s) => s.name === 'Projects' || s.name === 'Publications')
        // HIGH-4 (security re-review): `detectSections`' own boundary
        // detection (`matchesHeaderTerm` in `context-manager/sections.ts`) is
        // a lexicon PREFIX match — `line.startsWith(term)` plus a boundary
        // character — not a standalone-heading check. A body line merely
        // STARTING with a lexicon term (a "Research …" job title, a
        // "Projects" bullet) is misclassified as the section's own heading,
        // corrupting the boundary this net writes a spliced-in link into —
        // the fabrication class this file closes twice already, reopened
        // through a different door. Re-verify the line `detectSections`
        // pointed at against this PR's own standalone-heading predicates
        // before trusting it as a boundary that may receive a write; a
        // section whose "heading" doesn't actually pass either shape check
        // is discarded here, same as a section detectSections never found at
        // all — the link is left unplaced (PLACEMENT, not PRESENCE), never
        // spliced into an unrelated body line.
        .filter((s) => {
          const headingLine = (lines[s.startIndex] ?? '').trim();
          return isKnownSectionName(headingLine) || isAllCapsSectionHeading(headingLine);
        });
      if (sections.length) {
        // If exactly one item-shaped, still-unlinked line and exactly one
        // label remain, pair them — by elimination it is almost certainly
        // the renamed item, and this is the only case prompt partitioning
        // cannot fix (only knowable after generation). The slot pool is
        // gated to lines shaped like an item TITLE (#HIGH-1) — no bullet
        // marker consumed, no sentence-final period, a handful of words —
        // so pairing can never land on an already-linked project's own
        // description bullet, or wrap a whole prose sentence.
        const openSlots = unlinkedItemLineIndices(lines, sections);
        if (remaining.size === 1 && openSlots.length === 1) {
          const soleEntry = [...remaining][0];
          const soleSlot = openSlots[0];
          if (soleEntry && soleSlot !== undefined) {
            const [label, url] = soleEntry;
            const line = lines[soleSlot] ?? '';
            const start = stripLeadingMarker(line);
            const title = line.slice(start);
            if (title.trim()) {
              lines[soleSlot] = line.slice(0, start) + `[${title}](${url})`;
              remaining.delete(label);
            }
          }
        }

        // Anything still remaining is appended as its own new item, spliced
        // right after the (first) section's own last content line — never
        // at document end with no heading, and never inventing a section
        // that doesn't exist.
        if (remaining.size) {
          const target = sections[0];
          if (target) {
            const insertAt = sectionInsertionPoint(lines, target);
            lines.splice(
              insertAt,
              0,
              ...[...remaining].map(([label, url]) => `[${label}](${url})`)
            );
            remaining.clear();
          }
        }
      }
      // No PROJECTS/PUBLICATIONS section detected at all: leave `remaining`
      // untouched. Do not invent a heading, and do not push to document end
      // — a link with no legitimate home is left unplaced, on purpose.
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
 * dropped (the original academic-link bug).
 *
 * Partitioned in two (#HIGH part 1): most entries tell the model to name the
 * item with its own real name (#B) — `injectLinksIntoGeneratedText()`
 * matches those by normalized key, not literal text (#C). But a key shorter
 * than the matcher's own floor (`MIN_TITLE_KEY_LEN`) can never title-match no
 * matter what the model writes, so for those SHORT keys only, the old
 * "write this exact label" instruction survives — that's the only way the
 * literal-fallback in `injectLinksIntoGeneratedText()` can still reach them.
 * Telling the model "never write the key" for every entry, unconditionally,
 * made the fallback unreachable and the safety-net claim false.
 *
 * Returns '' when there are no body links.
 */
export function buildBodyLinksBlock(resume: string): string {
  const body = resolveBodyLinks(resume);
  if (!body.length) return '';

  const reachable = body.filter((b) => normalizeKey(b.label).length >= MIN_TITLE_KEY_LEN);
  const unreachable = body.filter((b) => normalizeKey(b.label).length < MIN_TITLE_KEY_LEN);

  const parts: string[] = [];
  if (reachable.length) {
    parts.push(
      `CANDIDATE PROJECT / PUBLICATION LINKS — each entry below is a machine-derived reference key ` +
        `for a link that belongs to a specific item in the résumé (a project, publication, or ` +
        `portfolio piece), NOT the contact line. For each entry, write that item using the project's ` +
        `REAL name as it appears in the résumé — never the key itself, never a URL or slug, and never ` +
        `appended to the title (the hyperlink is attached automatically by matching the item name). ` +
        `Every entry must end up with exactly one matching item. If an item has no natural home in ` +
        `Experience or Skills, add a PROJECTS or PUBLICATIONS section and list it there — but never ` +
        `invent a title or context just to force one in:\n` +
        reachable.map((b) => `- ${b.label}`).join('\n')
    );
  }
  if (unreachable.length) {
    parts.push(
      `CANDIDATE PROJECT / PUBLICATION LINKS (SHORT KEYS) — these reference keys are too short to ` +
        `rename safely, so write EACH ONE exactly as shown below, verbatim, as the visible text on ` +
        `its matching item (the hyperlink is attached automatically by matching this exact text). If ` +
        `an item has no natural home in Experience or Skills, add a PROJECTS or PUBLICATIONS section ` +
        `and list it there — but never invent a title or context just to force one in:\n` +
        unreachable.map((b) => `- ${b.label}`).join('\n')
    );
  }
  return parts.join('\n\n');
}

/**
 * Strip the link reference block from resume text before sending to the AI
 * so the body text budget is not wasted on the reference list.
 */
export function stripLinkBlock(resume: string): string {
  const sep = resume.lastIndexOf('\n---\n');
  return sep === -1 ? resume : resume.slice(0, sep);
}
