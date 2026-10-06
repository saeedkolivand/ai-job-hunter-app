import { normalizeKey } from './line-match.js';

// Known social/portfolio domains that belong in a resume contact line.
// `about.me`/`carrd.co` added for parity with Rust WEBSITE_HOSTS (#L1,
// contact_profile/classify.rs) — link-in-bio hosts alongside the other
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
 * (`contact_profile/classify.rs`). Used both to keep one off the "Website"
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
 * Derive a friendly label from a URL — mirrors the Rust url_label() in model/rich.rs.
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
export function parseLinkBlock(resume: string): LinkBlockEntry[] {
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
 * (`contact_profile/classify.rs`).
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
 * `contact_profile/classify.rs`): a host that is the apex of another candidate
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
export function classifyLinks(resume: string): ClassifiedLinks {
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

/** label→url map, built by assignment exactly as before the split (labels come from untrusted anchors). */
function toLabelMap(links: { label: string; url: string }[]): Record<string, string> {
  const map: Record<string, string> = {};
  for (const { label, url } of links) {
    map[label] = url;
  }
  return map;
}

/**
 * Build a label→url map for the contact links in the extracted reference block.
 * Used for post-processing: replacing plain labels with [label](url) markdown.
 */
export function getLinkMap(resume: string): Record<string, string> {
  return toLabelMap(classifyLinks(resume).contact);
}

/**
 * Build a label→url map for the BODY links (projects, publications, portfolio)
 * extracted from the reference block (#18). Consumed only by the résumé injection
 * path — cover letters never carry body links.
 */
export function getBodyLinkMap(resume: string): Record<string, string> {
  return toLabelMap(classifyLinks(resume).body);
}
