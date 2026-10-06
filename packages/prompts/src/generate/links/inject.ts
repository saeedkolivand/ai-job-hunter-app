import { detectSections } from '../../context-manager/sections.js';
import { isAllCapsSectionHeading, isKnownSectionName } from '../text/header-contact-line.js';
import {
  findInlineSeparator,
  matchLineTitle,
  sectionInsertionPoint,
  stripLeadingMarker,
  type TitleSpan,
  unlinkedItemLineIndices,
} from './line-match.js';

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
      // Bound the scan to BODY lines only — never the header block
      // (name/contact/tagline before the first section heading). Without
      // this bound, `matchLineTitle` accepting a match once the LINE (not
      // the label) is fully consumed — deliberate, for a short renamed item
      // like "orbit-sim" → "Orbital Simulator" — means a body label that
      // happens to literally START WITH the candidate's OWN NAME (a project
      // plausibly named after them, "Jane Doe Portfolio") can match the
      // header's own name line, and the injector wraps the candidate's own
      // name in a project hyperlink (#HIGH-3, security re-review). No
      // section heading found at all → scan nothing: with no identifiable
      // header/body boundary, PLACEMENT is not worth the risk of matching
      // into the header (the same "unplaced beats fabricated" trade-off
      // this whole net already makes elsewhere).
      const firstSectionIndex = lines.findIndex(
        (l) => isKnownSectionName(l) || isAllCapsSectionHeading(l)
      );
      const bodyStart = firstSectionIndex === -1 ? lines.length : firstSectionIndex;

      const candidates: { lineIndex: number; label: string; url: string; span: TitleSpan }[] = [];
      for (let i = bodyStart; i < lines.length; i++) {
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
            const rawStart = stripLeadingMarker(line);
            const rawPrefix = line.slice(0, rawStart);
            // `stripLeadingMarker`'s marker class includes `*` (for a
            // `* Item` bullet), so a run of PURELY `*` characters with
            // nothing else is virtually always a swallowed BOLD-OPEN
            // delimiter ("**Title**"), not a bullet — fold it back into the
            // title so the wrap below can never split a bold span across
            // the bracket boundary (leaving a bare, unlinked "**" before
            // the link and an orphaned closer inside it).
            const foldBackBold = rawPrefix !== '' && /^\*+$/.test(rawPrefix);
            const start = foldBackBold ? 0 : rawStart;
            const rest = line.slice(start);
            // MEDIUM (security re-review): an item-shaped line can still be a
            // TITLE plus an inline description on the same line ("Orbital
            // Simulator — A physics engine for Unity" is <= 8 words, so
            // isItemShapedLine admits it) — wrapping the WHOLE remainder in
            // `[…](url)` pulled the description into the clickable link
            // text. Cut at the first same-line separator (" — " / " – ")
            // instead; the title becomes the link, the separator + description
            // survive verbatim as plain trailing text on the same line —
            // preserved, never dropped.
            const sepIndex = findInlineSeparator(rest);
            const titleEnd = sepIndex === -1 ? rest.length : sepIndex;
            let title = rest.slice(0, titleEnd).trimEnd();
            const trailing = rest.slice(titleEnd);
            // A lone trailing `*` (not part of a `**` pair) is never valid
            // syntax on its own here — the markdown parser only recognizes
            // `**bold**`, not single-`*` italic — so it's always safe to
            // strip.
            if (/[^*]\*$/.test(title)) {
              title = title.slice(0, -1).trimEnd();
            }
            // A trailing `**` might be a stray marker (left dangling by the
            // separator cut above, or already stray in the model's own
            // output) OR the legitimate CLOSE of a real bold span whose
            // open half lives in the preserved prefix (the fold-back above
            // already handles the common case, but stays a set-once
            // constant — re-check here against whatever prefix survives).
            // Only strip when the total `**` count across prefix + title is
            // ODD (a genuine dangling marker); an EVEN total means it
            // legitimately closes something and must survive intact.
            if (title.endsWith('**')) {
              const prefix = line.slice(0, start);
              const pairCount = (s: string) => (s.match(/\*\*/g) ?? []).length;
              if ((pairCount(prefix) + pairCount(title)) % 2 !== 0) {
                title = title.slice(0, -2).trimEnd();
              }
            }
            if (title.trim()) {
              lines[soleSlot] = line.slice(0, start) + `[${title}](${url})` + trailing;
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
