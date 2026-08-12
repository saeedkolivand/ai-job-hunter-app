/**
 * Per-section verdict chips for a finished quality run.
 *
 * The plan asks for "✓ no changes needed / N issues / needs review" per section
 * after the overall check. Two things constrain how honestly that can be built,
 * and both are respected here rather than papered over:
 *
 * 1. **A content issue's `section` is the document's own HEADING text**
 *    (`"Experience"`, `"BERUFSERFAHRUNG"`, …), not the closed
 *    `PipelineSectionKey` grammar `regenerateSection` takes. Mapping one to the
 *    other is an alias lookup, so a heading this build doesn't recognize yields
 *    a chip with NO Fix button rather than a wrong section key on the wire.
 * 2. **"auto-repaired" is not derivable per section.** The `repair` stage's
 *    artifact carries counts only (`rounds`, `reverted`, `truncatedAttempts`,
 *    …) — never which sections it regenerated (ADR-027 content-free rule). So a
 *    section is `clean` or it has `issues`; the repair story is told once, at
 *    run level, from `metrics.repairRounds`/`metrics.reverted`.
 *
 * A `clean` chip is only ever emitted for a section whose heading was actually
 * FOUND in the document: claiming "Summary — no issues" for a résumé with no
 * summary would be inventing a check that never ran.
 */
import type { PipelineSectionKey } from '@ajh/shared';
import type { ContentReportPayload } from '@ajh/shared/ipc';

type ContentIssue = ContentReportPayload['issues'][number];

/**
 * Heading aliases per canonical section key, lower-cased, en + de (the two
 * locales this app ships). Matched by "the heading CONTAINS the alias" so
 * "Professional Experience" and "Work Experience" both land on experience
 * without an entry each.
 *
 * `experience:0` rather than a bare `experience`: the wire grammar's indexed
 * half needs an index, and at quality depth the draft is one pass so an
 * individual entry is not separately regenerable — index 0 resolves to the
 * whole experience section (Phase 4's section-wise generator makes per-entry
 * addressing real).
 */
const SECTION_ALIASES: { key: PipelineSectionKey; aliases: readonly string[] }[] = [
  {
    key: 'summary',
    aliases: [
      'summary',
      'profile',
      'objective',
      'zusammenfassung',
      'profil',
      'kurzprofil',
      'über mich',
    ],
  },
  {
    key: 'skills',
    aliases: ['skills', 'competencies', 'technologies', 'kenntnisse', 'fähigkeiten', 'kompetenzen'],
  },
  {
    key: 'experience:0',
    aliases: ['experience', 'employment', 'berufserfahrung', 'erfahrung', 'werdegang'],
  },
  { key: 'projects', aliases: ['projects', 'portfolio', 'projekte'] },
  { key: 'education', aliases: ['education', 'academic', 'ausbildung', 'bildung', 'studium'] },
];

/** The canonical key a heading names, or `null` when this build can't tell. */
export function sectionKeyForHeading(heading: string): PipelineSectionKey | null {
  const needle = heading.trim().toLowerCase();
  if (!needle) return null;
  for (const { key, aliases } of SECTION_ALIASES) {
    if (aliases.some((alias) => needle.includes(alias))) return key;
  }
  return null;
}

export interface SectionVerdict {
  /** The heading as the DOCUMENT spells it — what the chip shows. Never the
   *  wire key: `experience:0` is an identifier, not a section name. */
  label: string;
  /**
   * The wire key `regenerateSection` takes, when the heading maps to one.
   * `null` = a section this build can name but cannot address, so no Fix button.
   */
  sectionKey: PipelineSectionKey | null;
  issues: number;
  criticals: number;
}

/**
 * The document's OWN heading line for `key`, or `null` when it has none.
 *
 * Returns the text rather than a boolean because that text is the chip's label:
 * a clean section is named the way the document names it ("BERUFSERFAHRUNG",
 * "Professional Experience"), exactly like a flagged one — never the internal
 * wire key (`experience:0`), which is a machine identifier and means nothing to
 * a reader.
 *
 * A heading line is short and carries no sentence punctuation — enough to
 * separate `## EXPERIENCE` / `Experience` from a bullet that merely mentions
 * the word. Deliberately conservative: a missed heading costs a `clean` chip
 * (the section simply isn't listed), while a false one would assert a check
 * that never happened.
 */
function findSectionHeading(documentText: string, aliases: readonly string[]): string | null {
  const HEADING_MAX_CHARS = 48;
  for (const line of documentText.split('\n')) {
    // Markdown/bullet furniture is stripped so the label reads as a heading,
    // not as `## Experience`.
    const trimmed = line.replace(/^[#*\s]+/, '').trim();
    if (!trimmed || trimmed.length > HEADING_MAX_CHARS) continue;
    if (/[.!?,;]/.test(trimmed)) continue;
    const lower = trimmed.toLowerCase();
    if (aliases.some((alias) => lower.includes(alias))) return trimmed;
  }
  return null;
}

/**
 * Build the chip row: every section the report has findings for, plus every
 * canonical section whose heading is present in the document and which the
 * report left alone. Issue-carrying sections come first (most actionable), each
 * group in the order the report/alias table lists them.
 */
export function buildSectionVerdicts(
  report: ContentReportPayload | null | undefined,
  documentText: string
): SectionVerdict[] {
  if (!report) return [];

  const flagged = new Map<string, SectionVerdict>();
  for (const issue of report.issues) {
    const heading = issue.section?.trim();
    // A document-wide finding (`section: null`) belongs to no section — the
    // panel already lists those under its own group.
    if (!heading) continue;
    const existing = flagged.get(heading);
    const bump = (verdict: SectionVerdict, next: ContentIssue) => {
      verdict.issues += 1;
      if (next.severity === 'critical') verdict.criticals += 1;
    };
    if (existing) {
      bump(existing, issue);
    } else {
      const verdict: SectionVerdict = {
        label: heading,
        sectionKey: sectionKeyForHeading(heading),
        issues: 0,
        criticals: 0,
      };
      bump(verdict, issue);
      flagged.set(heading, verdict);
    }
  }

  const flaggedKeys = new Set([...flagged.values()].map((v) => v.sectionKey).filter(Boolean));
  const clean: SectionVerdict[] = [];
  for (const { key, aliases } of SECTION_ALIASES) {
    if (flaggedKeys.has(key)) continue;
    // The heading is BOTH the existence proof and the label — a clean chip only
    // exists because the document names the section, so there is never a case
    // where the wire key has to stand in for a missing heading.
    const heading = findSectionHeading(documentText, aliases);
    if (!heading) continue;
    clean.push({ label: heading, sectionKey: key, issues: 0, criticals: 0 });
  }

  return [...flagged.values(), ...clean];
}
