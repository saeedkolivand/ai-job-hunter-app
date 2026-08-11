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
 * - `resolved` — carries a verdict.
 */
export type FabricationPresentation = 'pending' | 'orphaned' | 'resolved';

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
 */
function parseFabrication(value: unknown): Fabrication | null {
  if (!isRecord(value)) return null;
  const { issueKey, code, evidence } = value;
  if (typeof issueKey !== 'string' || issueKey.trim() === '') return null;
  if (typeof code !== 'string' || typeof evidence !== 'string') return null;
  const decision = parseDecision(value.decision);
  return { issueKey, code, evidence, ...(decision ? { decision } : {}) };
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
 * How to present one entry against the document as it stands NOW.
 *
 * The substring check is exact because `evidence` is a verbatim span of the
 * generated document — the same property that lets the Rust repair loop locate
 * a `section: None` finding by its evidence. Cheap enough to run at render
 * time; an empty evidence string can't be located and reads as orphaned rather
 * than as a match against everything.
 */
export function presentFabrication(
  entry: Fabrication,
  documentText: string
): FabricationPresentation {
  if (entry.decision) return 'resolved';
  const span = entry.evidence.trim();
  if (!span || !documentText.includes(span)) return 'orphaned';
  return 'pending';
}

/** How many entries still need a verdict — what keeps a run `needsReview`. */
export function unresolvedCount(entries: Fabrication[]): number {
  return entries.filter((entry) => !entry.decision).length;
}
