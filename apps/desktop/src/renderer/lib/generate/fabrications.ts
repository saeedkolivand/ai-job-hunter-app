/**
 * The per-bullet fabrication review list — parsing it, and deciding how each
 * entry should be presented.
 *
 * `fabrications` rides inside a quality-report SLOT as opaque data: the
 * pipeline writes it, `parseSlot`/`mergeRecheckedReport` round-trip it without
 * looking inside (typing it there without validating it is the M-1 defect — an
 * unvalidated cast reaching a panel), and the surface that RENDERS it — this
 * module's callers — validates it here. Malformed entries are DROPPED, never
 * thrown on and never coerced: a review list is a trust surface, and half an
 * entry is worse than one entry fewer.
 */

/** One flagged bullet awaiting (or carrying) the user's verdict. */
export interface Fabrication {
  /** `<code>#<index>` — echoed back verbatim as `issueKey`. */
  issueKey: string;
  code: string;
  /** The offending span, verbatim from the generated document. */
  evidence: string;
  /**
   * The FULL line the evidence sat on when the report was built — the only
   * thing a removal may be anchored to.
   *
   * `evidence` is routinely a bare token (`"250"`, `"kubernetes"`, a single
   * letter), so locating a line by searching for it deletes whatever else
   * happens to contain those characters: `"250"` matches the phone number in
   * the contact header, `"kubernetes"` matches the whole skills line. The
   * validator picks the span; only the report knows which line it came from.
   *
   * OPTIONAL, and permanently so: reports persisted before this field existed
   * carry entries without it, and a re-checked report can preserve one. An
   * entry with no `line` is still decidable — it just cannot be applied
   * automatically (see {@link removeEvidenceLines}).
   */
  line?: string;
  /** `undefined` until the user decides — which is what keeps the run `needsReview`. */
  decision?: 'remove' | 'keep';
}

/**
 * How one entry should be PRESENTED, which is not the same as whether it is
 * resolved:
 *
 * - `pending` — undecided, and its evidence is still in the document. The
 *   ordinary Remove/Keep prompt.
 * - `orphaned` — undecided, but the evidence no longer occurs in the current
 *   text: the user hand-edited that line away, or a "Re-check" preserved the
 *   entry across a document the finding predates (deliberately preserved —
 *   wiping the list strands the run at `needsReview` forever, and reconciling
 *   it is the Rust side's call). Still DECIDABLE, because resolving it is what
 *   clears the review — but it must not be shown as a live "judge this bullet"
 *   prompt for text the user cannot find.
 * - `markedForRemoval` — the user chose Remove, the decision is recorded, and
 *   the flagged span is STILL in the document: the edit could not be applied
 *   (a read-only surface, or the write failed). The one state that must never
 *   read as finished — "Remove" that removes nothing while the chip turns green
 *   is the exact misreport this panel exists to prevent.
 * - `resolved` — carries a verdict AND the document agrees with it (Keep, or a
 *   Remove whose evidence is genuinely gone).
 */
export type FabricationPresentation = 'pending' | 'orphaned' | 'markedForRemoval' | 'resolved';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseDecision(value: unknown): Fabrication['decision'] {
  return value === 'remove' || value === 'keep' ? value : undefined;
}

/**
 * Validate ONE entry. Requires a non-empty `issueKey` (it is the write key for
 * `resolveFabrication` — an entry without one has no way to be resolved and
 * would sit in the list forever) plus string `code`/`evidence`. An unrecognized
 * `decision` reads as undecided rather than as a verdict the user never gave.
 *
 * `line` is the one field whose ABSENCE is a supported state — every report
 * persisted before it existed has none — so a missing or non-string one is read
 * as absent instead of dropping the entry. Dropping it would strand the run at
 * `needsReview` with nothing left to decide; reading it as absent only costs the
 * automatic apply, which then refuses honestly.
 */
function parseFabrication(value: unknown): Fabrication | null {
  if (!isRecord(value)) return null;
  const { issueKey, code, evidence, line } = value;
  if (typeof issueKey !== 'string' || issueKey.trim() === '') return null;
  if (typeof code !== 'string' || typeof evidence !== 'string') return null;
  const decision = parseDecision(value.decision);
  return {
    issueKey,
    code,
    evidence,
    ...(typeof line === 'string' ? { line } : {}),
    ...(decision ? { decision } : {}),
  };
}

/**
 * Parse the opaque `fabrications` key off a quality-report slot.
 *
 * Total: any input — absent, a bare string, an object, an array of garbage —
 * yields an array (possibly empty), never a throw. Duplicate `issueKey`s are
 * collapsed to the FIRST occurrence: the key is the write identity, so two rows
 * claiming it would let the user "decide" one and watch the other stay pending.
 */
export function parseFabrications(value: unknown): Fabrication[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const out: Fabrication[] = [];
  for (const raw of value) {
    const parsed = parseFabrication(raw);
    if (!parsed || seen.has(parsed.issueKey)) continue;
    seen.add(parsed.issueKey);
    out.push(parsed);
  }
  return out;
}

/**
 * Is this entry's flagged span still findable in the document as it stands NOW?
 *
 * The substring check is exact because `evidence` is a verbatim span of the
 * generated document — the same property that lets the Rust repair loop locate
 * a `section: None` finding by its evidence. Cheap enough to run at render
 * time; an empty evidence string can't be located and reads as absent rather
 * than as a match against everything.
 */
function evidencePresent(entry: Fabrication, documentText: string): boolean {
  const span = entry.evidence.trim();
  return !!span && documentText.includes(span);
}

/**
 * How to present one entry against the document as it stands NOW.
 *
 * A recorded `remove` is NOT enough to call the entry finished: the verdict is
 * a record of intent, and the document is the record of fact. They agree only
 * once the span is gone.
 */
export function presentFabrication(
  entry: Fabrication,
  documentText: string
): FabricationPresentation {
  const present = evidencePresent(entry, documentText);
  if (entry.decision === 'keep') return 'resolved';
  if (entry.decision === 'remove') return present ? 'markedForRemoval' : 'resolved';
  return present ? 'pending' : 'orphaned';
}

/**
 * Is this entry genuinely settled?
 *
 * A decision alone is not enough. `keep` settles it outright (nothing has to
 * change). `remove` settles it only when the flagged span is actually ABSENT
 * from the current text — counting a recorded-but-unapplied removal as resolved
 * is what lets the badge turn green over text that is still, verbatim, in the
 * document.
 */
export function isFabricationResolved(entry: Fabrication, documentText: string): boolean {
  if (!entry.decision) return false;
  if (entry.decision === 'keep') return true;
  return !evidencePresent(entry, documentText);
}

/** How many entries still need a verdict OR an applied removal — what keeps a
 *  run `needsReview` and the integrity chip off green. */
export function unresolvedCount(entries: Fabrication[], documentText: string): number {
  return entries.filter((entry) => !isFabricationResolved(entry, documentText)).length;
}

/**
 * Delete the entry's own line from `documentText` — the edit a "Remove" verdict
 * promises, and the only deletion this module will perform.
 *
 * Line-granular rather than span-granular because the flagged unit IS a bullet:
 * excising the span alone would leave a mutilated half-sentence behind, which
 * is a worse document than either keeping or dropping the claim. EVERY line
 * equal to the anchor goes; leaving a second copy behind would strand the entry
 * at `markedForRemoval` with its buttons already spent.
 *
 * **Anchored on `entry.line`, by exact (whitespace-trimmed) line equality —
 * never by searching for `evidence`.** Locating the line by substring is how
 * this function deleted the wrong text: validator evidence is routinely a bare
 * token, so `"250"` took out the contact header (the phone number contains those
 * digits), `"kubernetes"` took out the whole skills line, and a one-letter span
 * took out half the document. A relocation heuristic cannot distinguish the line
 * the finding is about from a line that merely shares characters with it, so
 * there is none: the report says which line it meant, or nothing is deleted.
 *
 * Returns `null` — REFUSE, do not guess — when the entry carries no `line` (a
 * report persisted before the field existed) or when no line of the current text
 * matches it (the user already edited it away, or edited it into something else).
 * The caller's honest fallback is to record the verdict and let the entry render
 * as `markedForRemoval`: "still in the document, edit it out to finish".
 */
export function removeEvidenceLines(documentText: string, entry: Fabrication): string | null {
  const anchor = entry.line?.trim();
  if (!anchor) return null;
  const lines = documentText.split('\n');
  const kept = lines.filter((line) => line.trim() !== anchor);
  // Nothing matched: the anchor describes a line this text no longer has.
  if (kept.length === lines.length) return null;
  return kept.join('\n');
}
