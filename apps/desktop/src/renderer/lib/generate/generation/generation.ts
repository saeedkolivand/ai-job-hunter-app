/**
 * LLM generation for Resume + Cover Letter.
 *
 * 1. Extract metadata (JSON — name, role, company, languages, keywords)
 * 2. Generate resume      (streamed text with **keyword** bold markers)
 * 3. Generate cover letter (streamed text with **keyword** bold markers)
 *
 * Generation runs through the backend orchestration pipeline (`ai.generatePipeline`),
 * which streams `ai:stream` deltas under the returned jobId. Export lives in `./export`.
 */

import {
  buildBuilderSystemPrompt,
  buildInterviewResumePrompt,
  type InterviewAnswers,
} from '@ajh/prompts/builder';
import {
  buildApplicationAnswerPrompt,
  buildApplicationAnswerSystemPrompt,
  buildApplicationEmailPrompt,
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildGitHubProjectsPrompt,
  buildGitHubProjectsSystemPrompt,
  buildInterviewQuestionsPrompt,
  buildInterviewQuestionsSystemPrompt,
  buildJobAdSummaryPrompt,
  buildJobAdSummarySystemPrompt,
  buildLikelyQuestionsPrompt,
  buildLikelyQuestionsSystemPrompt,
  buildMetadataPrompt,
  buildReferralImprovePrompt,
  buildReferralPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  buildRewritePrompt,
  buildStarFeedbackPrompt,
  buildStarFeedbackSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  type GenerationMode,
  getBodyLinkMap,
  getLinkMap,
  injectLinksIntoGeneratedText,
  isAllCapsSectionHeading,
  isFirstLineContactShaped,
  isHeaderContactLine,
  isKnownSectionName,
  parseGitHubProjects,
  type ReferralFormat,
  resolveMarket,
  type RewriteDocType,
  type SalaryRange,
  sanitizeCompanyName,
  sanitizeJobTitle,
  validateMetadata,
} from '@ajh/prompts/generate';
import type { ContactProfile, GitHubRepo } from '@ajh/shared';
import { detectLanguages, getLanguageName, toLanguageCode } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from '../../app-client';
import { OUTPUT_LANGUAGES, safeLocale } from '../locales';
import {
  buildProviderProfile,
  type GenerationIntent,
  resolveActiveProvider,
  resolveTemperatureOverride,
  type TemperatureStep,
} from '../provider-context';
import { awaitAiStream } from '../stream-promise';

export type { GenerationMeta, GenerationMode };
export { MODES } from '@ajh/prompts/generate';

// ─── LLM helpers ─────────────────────────────────────────────────────────────

/** Effective sampling inputs for one generation step: the user's per-model
 *  temperature OVERRIDE (Ollama-only, see {@link resolveTemperatureOverride}),
 *  plus the declared {@link GenerationIntent}. NEITHER field carries a raw
 *  default number — the backend adapter picks its own numbers (or none) per
 *  `(model, intent)`; the renderer states intent only. */
interface SamplingParams {
  temperature?: number;
  intent: GenerationIntent;
}

/** Resolve one step's {@link SamplingParams}: the temperature override lookup
 *  is Ollama-only and per-step; `intent` is the caller's declared, fixed
 *  classification for that surface (never computed from tier/model). */
function resolveSampling(step: TemperatureStep, intent: GenerationIntent): SamplingParams {
  return { temperature: resolveTemperatureOverride(step), intent };
}

async function streamGenerate(
  model: string,
  system: string,
  user: string,
  onToken: (tok: string) => void,
  temperature?: number,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void,
  intent: GenerationIntent = 'default'
): Promise<string> {
  const api = getClient();
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider(model);
  // Per-model generation limits are local (Ollama) only — cloud/CLI providers
  // ignore them, and the backend only applies num_predict/num_ctx for Ollama.
  const localLimits =
    activeProvider === 'ollama' ? providerSettings?.modelLimits?.[activeModel] : undefined;
  // Resume + cover-letter generation runs through the backend orchestration
  // pipeline (a composable Pipeline of stages), not the raw generate command.
  // Same streaming contract: emits `ai:stream` deltas under the returned jobId.
  const res = await api.ai.generatePipeline({
    model: activeModel,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    locale: safeLocale(locale),
    // `temperature` is the Ollama per-model override ONLY — an explicit user
    // choice that wins on every adapter. Every other sampling knob
    // (topP/frequencyPenalty/presencePenalty/repeatPenalty) is no longer sent
    // from here at all: each provider adapter picks its own numbers for
    // `intent`, or none, via `AiProvider::sampling_profile`
    // (`commands/ai_provider/mod.rs`) — see that doc comment for why sending
    // the same hardcoded numbers to every vendor was the actual bug.
    temperature,
    intent,
    // provider + baseUrl are NO LONGER sent (task #16): the backend resolves the
    // active provider/base_url from its own store and overwrites `model` before
    // streaming, so an XSS'd renderer can no longer point generation at an
    // arbitrary endpoint. `effort` (a generation tuning knob every
    // reasoning-capable provider now reads, not routing) stays.
    effort: providerSettings?.effort,
    // Per-model local limits (Ollama) — context window (num_ctx) + max output
    // (num_predict). Omitted (undefined) for cloud/CLI or when unset.
    maxTokens: localLimits?.maxTokens,
    contextWindow: localLimits?.contextWindow,
  });

  return awaitAiStream(api, res.jobId, {
    onToken,
    onThinking,
    signal,
    provider: activeProvider,
    model: activeModel,
    // Same value just sent to the backend above — sizes the renderer-side
    // timeout so a high-effort generation isn't killed client-side while the
    // backend is still legitimately streaming (`computeStreamTimeoutMs`).
    effort: providerSettings?.effort,
  });
}

// ─── Generation steps ─────────────────────────────────────────────────────────

export async function extractMetadata(
  resume: string,
  jobAd: string,
  model: string,
  locale = 'en'
): Promise<GenerationMeta> {
  // Detect languages client-side
  const clientSideDetection = detectLanguages(resume, jobAd);

  const profile = buildProviderProfile(model);

  const { system, user } = buildMetadataPrompt(resume, jobAd, profile);
  try {
    // Analysis carries its own per-model temperature override (user's chosen
    // design) plus the `deterministic` intent — exact, non-creative output.
    const sampling = resolveSampling('analysis', 'deterministic');
    const raw = await streamGenerate(
      model,
      system,
      user,
      () => {},
      sampling.temperature,
      locale,
      undefined,
      undefined,
      sampling.intent
    );
    const meta = validateMetadata(raw);
    if (meta) {
      // Override with client-side detection
      return {
        ...meta,
        resumeLanguage: clientSideDetection.resumeName,
        jobAdLanguage: clientSideDetection.jobAdName,
        mismatch: clientSideDetection.mismatch,
      };
    }
    console.warn('[extractMetadata] model returned unparseable JSON — using heuristics', {
      model,
      rawLength: raw.length,
    });
  } catch (err) {
    // Never silent: the heuristics below are materially worse than the model,
    // and a run that quietly took this path produced cover letters addressed to
    // a job-ad heading. Which path ran has to be visible in the log.
    console.warn('[extractMetadata] extraction failed — using heuristics', {
      model,
      error: err instanceof Error ? err.message : String(err),
    });
  }

  const nameMatch = resume.match(/^([A-Z][a-z]+ [A-Z][a-z]+(?:\s[A-Z][a-z]+)?)/m);
  // `\b` on every word alternative. Without it `at` matched inside "Wh(at) You'll
  // Do" / "gre(at) experience" / Dutch "d(at) je zelf", and `job` inside "jobs",
  // so the capture ran from mid-word to the next comma or newline.
  const titleMatch = jobAd.match(/\b(?:position|role|title|job)[:\s]+([^\n]+)/i);
  const companyMatch = jobAd.match(/(?:\b(?:at|company|employer|firm)|@)[:\s]+([^\n,]+)/i);
  return {
    candidateName: nameMatch?.[1] ?? '',
    // Same gate as the model's own output — this regex is the worse source.
    jobTitle: sanitizeJobTitle(titleMatch?.[1]),
    // Same gate the model's own output goes through — this regex is the WORSE
    // of the two sources, so it certainly does not get to skip validation.
    companyName: sanitizeCompanyName(companyMatch?.[1]),
    resumeLanguage: clientSideDetection.resumeName,
    jobAdLanguage: clientSideDetection.jobAdName,
    mismatch: clientSideDetection.mismatch,
    targetLanguage: clientSideDetection.resumeName,
    topRequirements: [],
  };
}

// ─── Header seeding (H — the editor is the source of truth) ────────────────────
//
// PDF/DOCX export used to rebuild the header (name + contact line) from the
// Contact Profile every time, discarding whatever the generated/edited text
// said. That silently dropped edits to those two lines from PDF/DOCX while
// they still shipped in TXT export and clipboard copy. The fix seeds the
// profile's own header into the canonical text right after generation, so the
// string the editor shows IS what exports — the Rust-side overrides
// (`ContactProfile::apply_to_header`, the `candidate_name` overrides) are now
// fallbacks for a header that has none, not unconditional rewrites.

// `isHeaderContactLine` (a mirror of the Rust parser's `is_contact_shaped`)
// lives in `@ajh/prompts/generate` — imported above — not here, so its
// cross-language parity fixture test can live alongside it in that package
// (packages/prompts/src/generate/text/header-contact-line.{ts,test.ts}),
// consistent with `urlToFriendlyLabel`'s existing Rust-parity pattern.

/**
 * True once we're past the header block. Mirrors the Rust parser's
 * `seen_section` flag: only an actual section heading ends the header zone —
 * a BLANK line does not on its own (Rust's parser leaves `seen_section` false
 * across one), so the scan below must keep looking past it, not stop there.
 * A markdown ATX heading (`#…`) is always a boundary (Rust's
 * `strip_atx_heading` check runs unconditionally, before any contact
 * classification); otherwise a line is a boundary when it's a known section
 * name (`isKnownSectionName` — covers every locale
 * `../../locale/index.ts`'s `CONVENTIONS` ships résumé headers for) OR has
 * the shape of an ALL-CAPS section title (`isAllCapsSectionHeading` — the
 * résumé prompt mandates ALL-CAPS headers, and this is what catches one not
 * literally in the known-name list: a locale's own wording, or an English
 * heading like "PROFESSIONAL EXPERIENCE" that isn't a verbatim match).
 *
 * This predicate is a best-effort recognizer, not a safety mechanism — see
 * `seedHeaderFromProfile`'s separate STRUCTURAL bound (the first
 * blank-line-delimited block) for what actually prevents an unrecognized
 * heading from turning the seeding scan destructive.
 */
function looksLikeHeaderBoundary(line: string): boolean {
  const t = line.trim();
  if (!t) return false;
  if (/^#{1,6}\s/.test(t)) return true;
  return isKnownSectionName(t) || isAllCapsSectionHeading(t);
}

/**
 * Strip control characters (a `\n` above all) and cap length — mirrors the
 * Rust `sanitize_header_part` treatment `contactLine` already went through
 * (it's built by `ContactProfile::header_markdown`). `fullName` reaches this
 * function as a separate, un-sanitized string (the profile's `fullName`
 * field, not part of `header_markdown`'s output), so without this it would
 * be the one field spliced into the seeded text unsanitized: a raw `\n`
 * would inject an arbitrary extra physical line, including — if it happened
 * to read as a known section name — a fabricated section Rust's parser
 * would treat as real. Iterates Unicode code points (`[...name]`), not
 * UTF-16 units (`.slice`) — a surrogate pair straddling the 200 cap would
 * otherwise split into a lone, invalid surrogate. Strips `\p{Cc}` (control
 * characters) AND `\p{Cf}` (Format characters, e.g. the bidi override
 * U+202E) — a bidi override can visually REVERSE the surrounding rendered
 * name, mirrors Rust's `is_format_char` in `contact_profile/mod.rs`.
 */
function sanitizeHeaderName(name: string): string {
  return [...name.replace(/[\p{Cc}\p{Cf}]/gu, '')].slice(0, 200).join('');
}

/**
 * Casing/diacritic/punctuation-insensitive key so "SAEED KOLIVAND" and "Saeed
 * Kolivand" — or "François Müller" and "FRANCOIS MULLER" (a document without
 * accents, a profile with them, or vice versa) — compare equal. NFKD
 * decomposes an accented character into base letter + combining mark (`ç` →
 * `c` + U+0327); the mark's Unicode category is `Mn` (Nonspacing_Mark), NOT
 * `\p{L}`/`\p{N}`, so it must be STRIPPED explicitly (`\p{M}` → `''`) before
 * the punctuation collapse below — collapsing it to a space instead (an
 * earlier version of this function did) inserts a spurious word break
 * ("François" → "franc ois"), which fails to match the same name written
 * without diacritics. Only after marks are gone does collapsing every
 * remaining non-letter/non-number run to a single space fold in whitespace
 * and punctuation differences (extra spaces, a stray period) too.
 */
function nameKey(s: string): string {
  return s
    .normalize('NFKD')
    .replace(/\p{M}/gu, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim()
    .toLowerCase();
}

/**
 * The index of the first non-blank line, or `lines.length` if every line is
 * blank (including an empty array). Shared by {@link headerBlockEnd} (the
 * header block starts here, not unconditionally at index 0 — Rust's parser
 * accepts the first line with content after ANY number of leading blanks,
 * `export/parser/mod.rs`, and this side must agree with it) and
 * `seedHeaderFromProfile`'s no-match insert branch (so a spliced-in contact
 * line lands right at/after the real content, not sandwiched inside a run of
 * several leading blanks).
 */
function firstContentLine(lines: string[]): number {
  let i = 0;
  while (i < lines.length && (lines[i] ?? '').trim() === '') i++;
  return i;
}

/**
 * The header block: the first run of content lines — starting at
 * {@link firstContentLine}, not unconditionally at index 0 — up to (not
 * including) the next blank line after that, or the whole array if there is
 * none. Shared ceiling for the name reconciliation search in
 * {@link findNameLine} and the contact-line scan in `seedHeaderFromProfile`
 * below — see that function's STRUCTURAL bound note for why neither scan may
 * look past it. Recomputed fresh wherever it's needed rather than cached
 * across a mutation: an `unshift` earlier in the same line array shifts every
 * later index by one, so a stale block-end value would silently drift out of
 * alignment with the array it's bounding.
 *
 * CodeRabbit (round 7): starting the termination scan at a hardcoded `i = 1`
 * (rather than one past {@link firstContentLine}) made TWO OR MORE leading
 * blank lines — a shape PDF extraction can and does produce, this codebase
 * has already special-cased ONE — return 1 immediately (the second blank),
 * so every real header line beyond it (the name at index 2+) fell outside
 * the block and became unreachable to both scans above. One leading blank
 * happened to still work by accident: index 1 there IS the real content
 * line, so the old hardcoded start coincided with the correct one.
 */
function headerBlockEnd(lines: string[]): number {
  for (let i = firstContentLine(lines) + 1; i < lines.length; i++) {
    if ((lines[i] ?? '').trim() === '') return i;
  }
  return lines.length;
}

/**
 * The index of the header-block line that's already the profile's own name
 * (compared via {@link nameKey}, so casing/diacritics/punctuation don't
 * matter), or -1 if none is. `key` is expected pre-computed by the caller so
 * an empty-after-normalizing `fullName` (punctuation-only) short-circuits
 * without scanning at all — an empty key would otherwise spuriously match
 * any other blank-after-normalizing line in the block.
 *
 * Also stops at the first `looksLikeHeaderBoundary` line, same STRUCTURAL
 * bound as the contact scan below it — WITHOUT this, `headerBlockEnd`
 * degrades to `lines.length` whenever the text has no blank line anywhere
 * (a wholly unstructured document), and an unbounded scan can then match the
 * candidate's name recurring later in the BODY (a "References available
 * upon request — Jordan Lee" sign-off, a certifications line repeating the
 * name) and reconcile THAT line instead of ever seeding one at the top —
 * strictly worse than the pre-reconciliation fallback, which at least always
 * unshifts. The match check runs BEFORE the boundary check, not after: an
 * ALL-CAPS name is itself boundary-shaped (`isAllCapsSectionHeading`), so it
 * must be allowed to match on the very line that would otherwise break the
 * scan — checking boundary first would make an ALL-CAPS name at the top of
 * the block unreachable.
 */
function findNameLine(lines: string[], key: string): number {
  if (!key) return -1;
  const blockEnd = headerBlockEnd(lines);
  for (let i = 0; i < blockEnd; i++) {
    const line = lines[i] ?? '';
    if (nameKey(line) === key) return i;
    if (looksLikeHeaderBoundary(line)) break;
  }
  return -1;
}

/** A real email shape (local-part `@` domain `.` tld), not just a bare `@` —
 *  "Software Engineer @ Acme" contains an `@` but no email; only a genuine
 *  email should outrank other candidates in {@link pickReplacementIndex}. */
const EMAIL_SHAPE_RE = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+[.][A-Za-z]{2,}/;

/**
 * Choose which of possibly several contact-shaped `matches` (excluding index
 * 0 — see the caller) to overwrite with `contactLine`, by a positive signal
 * rather than by position. The real contact line nearly always carries a
 * genuine email, so a match containing one wins; failing that, a match with
 * no `@` at all but a phone shape; failing that, the FIRST match in the
 * block — not the last: with no email/phone signal anywhere, the last match
 * is the one closest to the body, i.e. most likely to actually BE body
 * content (a skills line with 2+ separators) that a boundary-recognition
 * miss let through, and overwriting that is real content loss even though
 * this function never deletes a line. Precondition: `matches` is non-empty.
 */
function pickReplacementIndex(lines: string[], matches: number[]): number {
  const withEmail = matches.find((i) => EMAIL_SHAPE_RE.test(lines[i] ?? ''));
  if (withEmail !== undefined) return withEmail;
  const withPhone = matches.find((i) => {
    const line = lines[i] ?? '';
    return !line.includes('@') && isFirstLineContactShaped(line);
  });
  if (withPhone !== undefined) return withPhone;
  return matches[0] ?? 0;
}

/**
 * Seed the generated text's header with the Contact Profile's own values, so
 * the canonical string already carries the exact header PDF/DOCX export
 * renders. Line 1 (the name) is replaced when the profile has a `fullName`
 * (sanitized locally by {@link sanitizeHeaderName} — see there for why).
 * `contactLine` itself carries NO sanitizer on this side — unlike
 * `fullName`, it is never a raw profile field; it is always the return value
 * of the `contact_profile_header_line` IPC call, i.e. Rust's
 * `ContactProfile::header_markdown()`, which already strips control
 * characters and caps length before this function ever sees it. Adding a
 * second, redundant TS-side pass here would risk drifting from Rust's
 * sanitizer rather than adding any real safety — this asymmetry is
 * deliberate, not an oversight.
 *
 * Exactly ONE pre-section contact-shaped line — chosen by
 * {@link pickReplacementIndex}, not by position — is overwritten with
 * `contactLine`, never re-implemented here. No-op when the profile has
 * nothing to contribute (`contactLine` is then `''` too) — the model's own
 * header stands. (No early-return on an "effectively empty" profile: a
 * `fullName`-only profile has a name to seed but nothing else, and the two
 * guards below are already individually correct for that case on their
 * own — an extra gate here previously made the `fullName` branch
 * unreachable whenever the rest of the profile was blank.)
 *
 * This function NEVER removes a line — only ever replaces one, or (when
 * nothing in the block qualifies) inserts a new one. A second contact-shaped
 * line in the block (a duplicated/stale email, a job title that happens to
 * have 2+ separators) therefore SURVIVES untouched rather than being
 * deleted OR silently overwritten. That duplicate does reach export — Rust's
 * `model_from_resume_text` joins every pre-section Contact line with `" · "`
 * — so it is a real (visible, user-correctable) quality defect, not a no-op;
 * it is the deliberate trade for the property that actually matters: this
 * function can no longer DESTROY real résumé content (a job title, a body
 * line it mis-scans as contact-shaped) — whether by deleting it outright or
 * by silently overwriting it in place, which is just as much a loss.
 *
 * The first content line ({@link firstContentLine} — NOT unconditionally
 * index 0; with N ≥ 2 leading blank lines it can sit deeper in the block) is
 * never a candidate for the replacement scan below, including when there's
 * no `fullName` and that line is already contact-shaped by Rust's narrower
 * line-0-ONLY rule (`isFirstLineContactShaped`) — a combined
 * "Jane Doe | jane@example.com" with no separate name line. Were it eligible
 * and the ONLY candidate, overwriting it would erase "Jane Doe" entirely
 * (there is no separate name line to fall back to), which is worse than
 * leaving it untouched and inserting the profile's line right after — so it
 * always falls to the "insert" branch instead. (CodeRabbit, round 8: this
 * exclusion originally hardcoded index 0, which silently stopped covering
 * the line it protects once the header block itself started at
 * `firstContentLine` instead of index 0 — the exact line a combined
 * name+contact first line moves to behind ≥2 leading blanks.)
 *
 * Before the unshift/replace fallback above runs, the `fullName` branch
 * first searches the header block ({@link findNameLine}, bounded by the same
 * {@link headerBlockEnd} the contact scan below uses) for a line that's
 * ALREADY the profile's own name — compared casing/punctuation-insensitively
 * via {@link nameKey}, not by position — and overwrites just that line in
 * place instead of falling through to unshift/replace. This closes two
 * duplicate-header repros the position-0-only logic above didn't cover: (1)
 * an ALL-CAPS name line — `looksLikeHeaderBoundary` reads it as a section
 * heading via `isAllCapsSectionHeading`, so the unshift/replace fallback
 * would insert the profile's name ABOVE it, leaving the ALL-CAPS original
 * sitting right below as a fake body section (with the model's title and
 * original contact line under it — a fully duplicated header); (2) a leading
 * blank line (PDF extraction routinely emits one) — line 0 gets blindly
 * replaced, leaving the model's real, still-unrecognized name line one row
 * down, where the contact scan then also mis-splices a second contact line.
 * Reclassifying that one line is the ONLY safe re-scoping of a line
 * `looksLikeHeaderBoundary` flagged: `nameKey` can match nothing but the
 * profile's own name, so it's the single line in the block guaranteed to be
 * redundant with what we're about to write there — the
 * never-remove-or-blind-overwrite invariant above still holds for every
 * OTHER line in the block.
 *
 * `sanitizeHeaderName` never changes case — the profile's own casing is what
 * the user typed, and this function must not invent a different rendering of
 * their name — so an ALL-CAPS `fullName` reconciled onto a line at `i > 0`
 * (case (2) above) is STILL ALL-CAPS afterward, and would re-trip
 * `looksLikeHeaderBoundary` right where the contact scan below starts
 * looking. That scan is told which index (if any) was just reconciled and
 * skips it exactly like index 0 — see the comment at that scan.
 *
 * `findNameLine` shares the contact scan's STRUCTURAL bound (below) for the
 * same reason: without it, a document with no blank line anywhere turns
 * `headerBlockEnd` into `lines.length`, and the name search could match the
 * profile's name recurring later in the BODY (a sign-off, a repeated byline)
 * instead of ever finding a header line to reconcile — worse than not
 * searching at all, since it would suppress the unshift fallback that
 * otherwise guarantees a name line lands at the top.
 *
 * ponytail: known ceiling, not a bug — an ALL-CAPS (or otherwise
 * boundary-shaped) name line that does NOT match the profile's `fullName`
 * still falls through to unshift and stacks as a visible duplicate. The
 * profile is the only ground truth available here; guessing that some
 * OTHER boundary-shaped line is "probably the name anyway" would risk
 * destroying a real section heading instead, which is strictly worse.
 *
 * STRUCTURAL bound (the actual safety mechanism — `looksLikeHeaderBoundary`
 * is best-effort recognition, not this): the scan below never looks past the
 * first blank-line-delimited block from the top of the text — the header, by
 * definition. `looksLikeHeaderBoundary` firing on a real heading stops the
 * scan earlier, inside that block, which is what makes ordinary multi-line
 * headers (name / title / contact, still one block) work; but a
 * heading-recognition MISS (an unfixtured locale, a creative heading) can now
 * only degrade to "didn't seed" or "left a duplicate line" — combined with
 * "never remove or blind-overwrite," it can never again scan into the
 * document body and destroy real résumé content (a job entry, a skills line,
 * a project) the way an unbounded, removal-capable scan once did.
 */
export function seedHeaderFromProfile(
  text: string,
  profile: ContactProfile,
  contactLine: string
): string {
  const lines = text.split('\n');
  if (!lines.length) return text;

  const fullName = profile.fullName?.trim();
  // Set below when the fullName branch reconciles an EXISTING header-block
  // line in place (rather than unshifting/replacing index 0) — the contact
  // scan needs this index so it can treat that line like index 0 (never a
  // boundary check, never a match candidate). See the note at that scan.
  let reconciledNameIndex = -1;
  if (fullName) {
    // First: is one of the header block's own lines already this name, just
    // differently cased/punctuated (e.g. an ALL-CAPS name, or the model's
    // real name line pushed to index 1 by a leading blank)? If so, fix it in
    // place — see the doc comment above ("Before the unshift/replace
    // fallback above runs…") for why this is safe.
    const nameLineIndex = findNameLine(lines, nameKey(fullName));
    if (nameLineIndex !== -1) {
      lines[nameLineIndex] = sanitizeHeaderName(fullName);
      reconciledNameIndex = nameLineIndex;
    } else {
      // Guarded like every other write below: line 0 is only overwritten when
      // it's actually name-shaped — not a section heading (`looksLikeHeaderBoundary`)
      // and not already contact-shaped (`isFirstLineContactShaped`). A model
      // that omits the name line (starts straight with "SUMMARY" or a
      // "Jane Doe | jane@example.com" combined line) must never have that line
      // clobbered — the name is INSERTED ahead of it instead, same
      // never-remove-or-blind-overwrite invariant the rest of this function
      // holds. See the doc comment above for the destructive repro this closes.
      const line0 = lines[0] ?? '';
      if (looksLikeHeaderBoundary(line0) || isFirstLineContactShaped(line0)) {
        lines.unshift(sanitizeHeaderName(fullName));
      } else {
        lines[0] = sanitizeHeaderName(fullName);
      }
    }
  }

  if (contactLine.trim()) {
    // The header block: line 0 up to (not including) the first blank line,
    // or the whole text if there is none. Hard ceiling for the scan below —
    // see the STRUCTURAL bound note above. Recomputed here (not reused from
    // the fullName branch above) because an `unshift` there would have
    // shifted every index by one.
    const blockEnd = headerBlockEnd(lines);

    // The first content line is never a candidate here (loop starts at
    // i = 1, and this second check excludes it again when it sits deeper
    // than 1 behind ≥2 leading blanks) — see the doc comment above. Computed
    // once, not per-iteration: it doesn't change across the loop (nothing
    // above this point mutates `lines`' length once we're inside the
    // contact branch).
    const contentStart = firstContentLine(lines);
    const matches: number[] = [];
    for (let i = 1; i < blockEnd; i++) {
      // Two independent exclusions, checked every iteration since either can
      // land anywhere in the block (not just near the top):
      // - `contentStart`: the first content line — see the doc comment above
      //   for why it can never be a candidate, regardless of `fullName`.
      // - `reconciledNameIndex`: the name step above can reconcile fullName
      //   IN PLACE onto a DIFFERENT line at i > 0 (a leading-blank shifted
      //   the real name line down one, or a non-name-shaped line sits ahead
      //   of it) without changing its casing — `sanitizeHeaderName` never
      //   Title-Cases, since the profile's own casing is what the user
      //   typed. An ALL-CAPS profile name therefore stays ALL-CAPS after
      //   reconciliation and would re-trip `looksLikeHeaderBoundary` right
      //   here, breaking the scan before it ever reaches the real contact
      //   line below and stacking a second one via the insert branch.
      // These usually coincide (the name IS the first content line) but not
      // always (a non-name, non-boundary line — an unrecognized subtitle —
      // can sit ahead of the name inside the block), so both are checked
      // independently rather than assuming equality.
      if (i === contentStart || i === reconciledNameIndex) continue;
      const line = lines[i] ?? '';
      if (looksLikeHeaderBoundary(line)) break;
      if (isHeaderContactLine(line)) matches.push(i);
    }

    if (matches.length > 0) {
      lines[pickReplacementIndex(lines, matches)] = contactLine;
    } else {
      // CodeRabbit (security re-review): assuming line 0 is always the name
      // was already wrong once — true after the fullName-driven
      // unshift/replace above, or when the model already wrote a name/
      // contact line on its own, but NOT when there's no fullName to seed
      // AND line 0 is itself a section heading (the model omitted the name
      // line entirely); splicing right after it would put the contact line
      // INSIDE that section, under its heading, not in the header block
      // above it. Insert ahead of the heading in that case instead.
      //
      // CodeRabbit (round 7): the same reasoning generalizes past index 0 —
      // with N ≥ 2 leading blank lines the real first content line is at
      // `firstContentLine(lines)`, not unconditionally index 0/1, and a
      // hardcoded `1` would splice the contact line into the MIDDLE of that
      // blank run, ahead of the actual content. Reduces to the exact
      // original 0-or-1 ternary above for zero leading blanks (where
      // `firstContentLine` is 0).
      const contentStart = firstContentLine(lines);
      const insertAt = looksLikeHeaderBoundary(lines[contentStart] ?? '')
        ? contentStart
        : contentStart + 1;
      lines.splice(insertAt, 0, contactLine);
    }
  }

  return lines.join('\n');
}

/**
 * Fetch the Contact Profile + its localized header line and seed them into
 * `text` (H — the editor is the source of truth over whatever header the
 * model wrote). Shared by every résumé-producing generation path —
 * `generateResume` AND `synthesizeResume` (the Resume Builder), which has no
 * base résumé to derive a header from — its prompt
 * (`packages/prompts/src/builder/builder-prompt.ts`) has the model write an
 * ordinary name + contact line, same as any other résumé prompt, and this
 * call overwrites it with the profile's own values regardless of what the
 * model wrote, exactly like the base-résumé path.
 *
 * Both IPC calls are guarded (`.catch(() => undefined)`): header seeding is
 * cosmetic post-processing on an already-finished, already-paid-for AI
 * generation — a transient IPC failure here must degrade to "seed nothing"
 * (the model's own header stands, same as before H shipped), never throw and
 * discard the whole result the caller is about to persist.
 *
 * `signal` (LOW, security re-review): the generation `AbortSignal` — this
 * step runs AFTER the model stream finishes, so it can't cancel an in-flight
 * generation, but it must still honor a cancellation the user issued WHILE
 * these two post-stream IPC calls were in flight (or already true when this
 * runs). Tauri's `invoke()` has no native abort wiring, so this can't cancel
 * the underlying calls themselves — it short-circuits to "seed nothing"
 * before starting AND after they resolve, so a cancelled generation's
 * discarded result is never silently patched by a seeding call the caller
 * no longer wants applied.
 */
async function seedHeaderFromContactProfile(
  text: string,
  meta: GenerationMeta,
  locale: string,
  signal?: AbortSignal
): Promise<string> {
  if (signal?.aborted) return text;
  const api = getClient();
  const headerLang = toLanguageCode(meta.targetLanguage || locale);
  // Fired concurrently, not sequentially — headerLine's input (headerLang)
  // doesn't depend on the fetched profile, so there is nothing to gain by
  // awaiting `get` first; Promise.all saves a round-trip on the common
  // (both-succeed) path. Each call keeps its own independent guard: a
  // rejection on either one still degrades to "seed nothing," never throws.
  const [contact, contactLine] = await Promise.all([
    api.contactProfile.get().catch((err: unknown) => {
      console.warn(
        'seedHeaderFromContactProfile: contactProfile.get failed, header not seeded',
        err
      );
      return undefined;
    }),
    api.contactProfile.headerLine(headerLang).catch((err: unknown) => {
      console.warn(
        'seedHeaderFromContactProfile: contactProfile.headerLine failed, header not seeded',
        err
      );
      return undefined;
    }),
  ]);
  if (signal?.aborted) return text;
  if (!contact || contactLine === undefined) return text;
  return seedHeaderFromProfile(text, contact, contactLine);
}

export async function generateResume(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const profile = buildProviderProfile(model);
  const tone = usePreferencesStore.getState().outputTone;

  const system = buildResumeSystemPrompt(mode, profile, tone, meta.targetLanguage);
  const user = buildResumePrompt(resume, jobAd, meta, mode, profile);
  // Résumé generation is `deterministic`: exact, non-creative output — the
  // exact job-ad keyword repetition ATS matching needs must survive verbatim.
  const sampling = resolveSampling('resume', 'deterministic');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    sampling.temperature,
    locale,
    signal,
    onThinking,
    sampling.intent
  );
  // Contact links go on the header line; body links (projects/publications, #18)
  // are re-attached to their own items anywhere in the body.
  const injected = injectLinksIntoGeneratedText(
    extractPlainText(raw),
    getLinkMap(resume),
    getBodyLinkMap(resume)
  );

  // H: seed the profile's own header (name + contact line) into the text now,
  // AFTER link injection, so it wins over whatever contact-line links that step
  // wrote — leaving that step's contact-line pass in place is harmless, its
  // output is simply overwritten here.
  return seedHeaderFromContactProfile(injected, meta, locale, signal);
}

/**
 * Resume Builder synthesis (#1 / B9): build a from-scratch résumé from structured
 * interview answers in a SINGLE streamed pass. Mirrors {@link generateResume} —
 * same provider config, effective tier, and streaming pipeline (so it works for
 * every provider with zero per-provider code and adds NO new IPC) — but uses the
 * builder prompts grounded on `<interview_answers>` instead of a base résumé + job
 * ad. Provided links are kept inline by the prompt, so no link-map injection is
 * needed (there is no source résumé to parse). Header-seeded exactly like
 * {@link generateResume} (H) — the builder prompt has the model write an
 * ordinary name + contact line, and {@link seedHeaderFromContactProfile}
 * overwrites it with the profile's own values regardless.
 */
export async function synthesizeResume(
  answers: InterviewAnswers,
  meta: GenerationMeta,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const profile = buildProviderProfile(model);

  const system = buildBuilderSystemPrompt(profile);
  const user = buildInterviewResumePrompt(answers, meta);
  const sampling = resolveSampling('resume', 'deterministic');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    sampling.temperature,
    locale,
    signal,
    onThinking,
    sampling.intent
  );
  return seedHeaderFromContactProfile(extractPlainText(raw), meta, locale, signal);
}

/**
 * Best-effort company research for the cover-letter "fit" paragraph. Routes
 * through the backend enricher — the active provider's own web search +
 * synthesis, cached. Any failure or a provider that can't search yields '' so
 * the cover letter still generates. The returned brief is untrusted reference
 * text — the prompt fences it.
 */
export async function researchCompany(
  jobAd: string,
  company?: string,
  role?: string
): Promise<string> {
  try {
    // Routing (provider/model/base_url) is backend-owned (task #16) — the enricher
    // reads the active provider from the store, so nothing is threaded here.
    const res = await getClient().ai.researchCompany({
      jobAd,
      // The AI-extracted company name is far more reliable than the backend's
      // heuristic job-ad scan (which can grab a tagline), so send it when known.
      company: company?.trim() || undefined,
      // Same reasoning as `company`: the backend's heuristic falls back to the
      // ad's first short line, which on a scraped page is an apply button.
      role: role?.trim() || undefined,
      // Same effort the generation request carries — the backend's deadline
      // around search + synthesis scales with it. Without this a reasoning model
      // never finishes research inside the flat bound, and the cover letter is
      // written with no company knowledge and no visible failure.
      effort: resolveActiveProvider().providerSettings?.effort,
    });
    return res?.brief ?? '';
  } catch {
    return '';
  }
}

/**
 * Best-effort, per-question web-search reference notes for an application
 * answer — opt-in sibling of {@link researchCompany}, scoped to a single
 * question's topic (combines it with the role + company for relevance)
 * rather than a general company overview. Any failure or a provider that
 * can't search degrades to `''` so the answer still generates exactly as
 * without web search — this call must never block or fail generation.
 */
export async function researchAnswer(
  question: string,
  role: string,
  company: string
): Promise<string> {
  try {
    // Routing is backend-owned (task #16) — the enricher reads the active provider
    // from the store, so nothing is threaded here.
    const res = await getClient().ai.researchAnswer({
      question,
      role: role.trim() || undefined,
      company: company.trim() || undefined,
    });
    return res ?? '';
  } catch {
    return '';
  }
}

/**
 * Best-effort web-grounded market salary-range lookup for the salary
 * application question (C2). Routes through the backend enricher — the active
 * provider's own web search, validated and cached. Any failure, timeout, or a
 * provider that can't search yields `undefined` so the salary answer always
 * falls back to the C1 applicant-preference-only grounding — this call must
 * never block or fail the answer.
 */
export async function lookupSalaryRange(
  role: string,
  company: string,
  location: string,
  /** ISO-3166 alpha-2 job country, when known — grounds the researched currency. */
  country?: string,
  /** Authoritative ISO-4217 currency for `country` (resolve via `countryToCurrency`
   *  from `@ajh/prompts/generate`); omitted falls back to today's unconstrained
   *  "local currency for that location" behavior. */
  currency?: string
): Promise<SalaryRange | undefined> {
  try {
    // Routing is backend-owned (task #16) — the enricher reads the active provider
    // from the store, so nothing is threaded here.
    const res = await getClient().ai.lookupSalary({
      role,
      company: company.trim() || undefined,
      location: location.trim() || undefined,
      country: country?.trim() || undefined,
      currency: currency?.trim() || undefined,
      effort: resolveActiveProvider().providerSettings?.effort,
    });
    return res ?? undefined;
  } catch {
    return undefined;
  }
}

/**
 * Generate the cover letter and surface the company-research brief that informed
 * it. When `opts.researchCompany` is on, a best-effort brief is fetched and folded
 * into the prompt; it is also returned so the caller can persist it on the
 * generation record (the doc card's "Company research" section). `companyBrief` is
 * `''` when research is off or the fetch yields nothing. `text` is the cleaned,
 * link-injected letter.
 */
export async function generateCoverLetter(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void,
  opts?: { researchCompany?: boolean; market?: string }
): Promise<{ text: string; companyBrief: string }> {
  const profile = buildProviderProfile(model);

  // Opt-in: fetch a company brief and fold it into the prompt's fit paragraph.
  const companyBrief = opts?.researchCompany
    ? await researchCompany(jobAd, meta.companyName, meta.jobTitle)
    : '';

  // Resolve the cover-letter market from the job's country (decision: job
  // location, not ad language) with an optional manual override; the letter is
  // written in `meta.targetLanguage` but adopts this market's etiquette.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
    override: opts?.market,
  });
  // User-supplied preferences (salary/start date) — stated only where the market
  // expects them (e.g. DACH); never fabricated. From the global settings store.
  const applicant = usePreferencesStore.getState().applicant;
  const tone = usePreferencesStore.getState().outputTone;

  // No external writing-style sample is threaded through here: the candidate's
  // résumé is already embedded verbatim in <candidate_resume>, and the prompt
  // builder's own voice directive points there instead of duplicating it (see
  // buildResumeVoiceDirective). `hasStyleReference` stays false (default), so
  // the fictional tone exemplar (English-target only) still applies.
  // `hasBrief` mirrors `buildCoverLetterPrompt`'s own derivation exactly
  // (`Boolean(companyBrief.trim())`), so the system prompt only points at
  // <company_research> when the user prompt will actually fence one. Passing it
  // is what makes the gate live — the parameter defaults to `true` (today's
  // unconditional behavior) for callers that cannot know yet.
  const system = buildCoverLetterSystemPrompt(
    mode,
    profile,
    tone,
    meta.targetLanguage,
    false,
    Boolean(companyBrief.trim())
  );
  const user = buildCoverLetterPrompt(
    resume,
    jobAd,
    meta,
    mode,
    profile,
    companyBrief,
    market,
    applicant
  );
  // Cover letters are `prose_grounded`, not plain `prose`: "deliberately
  // creative" is a REGISTER argument (the temperature), not a license to
  // drop the traceability guard — the letter asserts real résumé
  // achievements to an employer and carries the prompt's own `LETTER_HONESTY`
  // contract (`packages/prompts/src/generate/cover-letter/cover-letter.ts`),
  // and presence_penalty pushing toward new topics is exactly the
  // invented-achievement risk that contract exists to prevent. The active
  // provider's adapter picks its own temperature/penalty numbers for this
  // intent, per model (see `AiProvider::sampling_profile`,
  // `commands/ai_provider/mod.rs`).
  const sampling = resolveSampling('cover', 'prose_grounded');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    sampling.temperature,
    locale,
    signal,
    onThinking,
    sampling.intent
  );
  return {
    text: injectLinksIntoGeneratedText(extractPlainText(raw), getLinkMap(resume)),
    companyBrief,
  };
}

/**
 * Generate a single, résumé-grounded answer to one application question. Routes
 * through the same streaming pipeline as résumé/cover-letter generation (so it
 * works for every provider with zero per-provider code) and the shared grounding
 * contract (no fabrication). Pass `companyBrief` to inform company-context
 * questions; it is fenced as untrusted by the prompt layer. Returns plain text.
 */
export async function generateApplicationAnswer(params: {
  question: string;
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  companyBrief?: string;
  /** Opt-in per-question web-search notes (see {@link researchAnswer}); fenced
   *  separately from `companyBrief` and never a source of candidate facts. */
  webSearchNotes?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
  /** This question's registry `guidance` (see `ApplicationQuestion.guidance`),
   *  when it has one — absent for user-typed custom questions. */
  guidance?: string;
  /** Web-researched market salary range (salary question only, see
   *  {@link lookupSalaryRange}); undefined when no lookup ran or it found
   *  nothing reliable. */
  salaryRange?: SalaryRange;
}): Promise<string> {
  const {
    question,
    resume,
    jobAd,
    meta,
    model,
    companyBrief = '',
    webSearchNotes = '',
    signal,
    onToken,
    guidance,
    salaryRange,
  } = params;
  const profile = buildProviderProfile(model);

  // Market drives the answer's register; applicant prefs answer logistics
  // questions (salary/start date/notice/remote) honestly without fabrication.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  const applicant = usePreferencesStore.getState().applicant;
  const tone = usePreferencesStore.getState().outputTone;

  const system = buildApplicationAnswerSystemPrompt(tone, meta.targetLanguage);
  const user = buildApplicationAnswerPrompt({
    question,
    resume,
    jobAd,
    meta,
    companyBrief,
    webSearchNotes,
    target: profile,
    market,
    applicant,
    guidance,
    salaryRange,
    // No external writing-style sample: the résumé is already in
    // <candidate_resume>, and the prompt builder's own voice directive points
    // there instead of duplicating it (see buildResumeVoiceDirective).
  });
  // Application answers are `prose_grounded`, not plain `prose`: the output
  // asserts factual claims about the candidate to a real employer, so it
  // must stay traceable to the résumé — the active provider's adapter drops
  // `presencePenalty` for this intent specifically (it pushes toward new
  // topics, i.e. invented candidate facts) while keeping every other
  // detector-resistance knob `prose` gets (see `AiProvider::sampling_profile`,
  // `commands/ai_provider/mod.rs`).
  const sampling = resolveSampling('answers', 'prose_grounded');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Summarize a single job ad into a short "key notes" digest — résumé-INDEPENDENT
 * (no résumé, no company brief, no scoring). Routes through the same streaming
 * pipeline as the other generators (zero per-provider code), at low temperature.
 * The digest is written in the ad's own language (`meta.targetLanguage`) and
 * returned as concise markdown (bold section labels survive `extractPlainText`).
 */
export async function generateJobAdSummary(params: {
  jobAd: string;
  meta?: GenerationMeta | null;
  model: string;
  language?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { jobAd, meta, model, language, signal, onToken } = params;
  // Nothing to summarize → skip the wasted API call on an empty/whitespace ad.
  if (!jobAd.trim()) return '';
  const profile = buildProviderProfile(model);

  // `language` arrives as a locale CODE ('de', 'es', …) from the picker. The prompt
  // wants a human language NAME; streamGenerate wants a code. Resolve both once from
  // OUTPUT_LANGUAGES (the allowlist) so the name interpolated into the prompt can't
  // be an arbitrary injected string and the locale isn't silently collapsed to 'en'.
  const lang = language ? OUTPUT_LANGUAGES.find((l) => l.code === language) : undefined;

  const system = buildJobAdSummarySystemPrompt(lang?.englishName);
  const user = buildJobAdSummaryPrompt(jobAd, meta, profile, lang?.englishName);
  // A factual digest, not creative writing — `deterministic`, keyed off
  // `analysis` (the same job-ad-analysis surface as `extractMetadata`, not
  // `answers` — this never touches the résumé or the candidate's own words).
  const sampling = resolveSampling('analysis', 'deterministic');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    lang?.code ?? meta?.targetLanguage ?? 'en',
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Generate AI-suggested questions the candidate can ASK the interviewer. Routes
 * through the same streaming pipeline as the other generators (zero per-provider
 * code) and the untrusted company-research fence, so web intel only adds context.
 * Pass `companyBrief` (gathered research) so questions cite concrete company/role
 * detail; `seedTopics` biases them (hybrid). Returns the raw delimited text —
 * parse with `parseInterviewQuestions`.
 */
export async function generateInterviewQuestions(params: {
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  companyBrief?: string;
  seedTopics?: string[];
  /** Target interviewers (canonical audience ids) — N questions per audience. */
  audiences?: string[];
  /** Output language: a locale CODE ('de', 'es', …) when it came from the picker,
   *  otherwise whatever the ad detection produced (a code outside the picker's
   *  allowlist, or a language NAME). Overrides `meta.targetLanguage`, and
   *  deliberately does NOT feed `resolveMarket` — the register stays that of the
   *  job's country even when only the output language changes. */
  language?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const {
    resume,
    jobAd,
    meta,
    model,
    companyBrief = '',
    seedTopics = [],
    audiences = [],
    language,
    signal,
    onToken,
  } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  // The prompt wants a human language NAME, streamGenerate wants a locale code.
  // An allowlisted picker code resolves to its English name; anything else (a
  // detected language the picker doesn't offer, e.g. 'nl') goes through
  // `getLanguageName` — 28 codes, degrading to the code itself.
  //
  // The ISO-639-1 SHAPE CHECK is defence-in-depth, not cosmetics: `language` can
  // originate from a scraped ad (ad → extractMetadata → meta.targetLanguage →
  // here), `getLanguageName` returns an unrecognised string verbatim, and the
  // result lands in the prompt as an instruction OUTSIDE the untrusted-input
  // fence. Anything that isn't code-shaped is dropped rather than echoed, which
  // leaves the `meta`-derived note to run instead. Mirrors the same guard
  // documented on `generateJobAdSummary` above. `nl`/`pl`/`pt-br` still pass.
  const lang = language ? OUTPUT_LANGUAGES.find((l) => l.code === language) : undefined;
  const isIsoCode = /^[a-z]{2}(-[a-z]{2})?$/i.test(language ?? '');
  const languageName =
    lang?.englishName ?? (language && isIsoCode ? getLanguageName(language) : undefined);
  // The anti-AI-tell lexicon keys off the CODE, and a language can arrive as a
  // NAME on extractMetadata's regex-fallback path — 'German'.slice(0, 2) is 'ge',
  // which silently misses the curated German lexicon. Normalize once, here.
  const languageCode = toLanguageCode(lang?.code ?? language ?? meta.targetLanguage ?? '');

  const system = buildInterviewQuestionsSystemPrompt(languageCode);
  const user = buildInterviewQuestionsPrompt({
    resume,
    jobAd,
    meta,
    companyBrief,
    seedTopics,
    audiences,
    target: profile,
    market,
    language: languageName,
  });
  // Interview questions are prose: creative, detector-resistant writing —
  // keyed off `questions`, not `answers` (that key is application answers
  // only; the candidate asks these, they don't answer them).
  const sampling = resolveSampling('questions', 'prose');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    // Same code the lexicon uses; `streamGenerate` clamps it via `safeLocale`,
    // so a language outside the supported set falls back to 'en' here only.
    languageCode || 'en',
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Generate likely questions the CANDIDATE will be ASKED for this role — the
 * mock-interview practice set (distinct from {@link generateInterviewQuestions},
 * where the candidate asks the interviewer). Routes through the same streaming
 * pipeline as every other generator (zero new IPC). Session-only feature:
 * nothing produced here is persisted to the aiGenerations aggregate. Returns
 * the raw delimited text — parse with `parseLikelyQuestions`.
 */
export async function generateLikelyInterviewQuestions(params: {
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { resume, jobAd, meta, model, signal, onToken } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });

  const system = buildLikelyQuestionsSystemPrompt();
  const user = buildLikelyQuestionsPrompt({ resume, jobAd, meta, target: profile, market });
  // Prose, same intent (and same `questions` key) as the other interview surfaces.
  const sampling = resolveSampling('questions', 'prose');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Generate STAR-rubric feedback on the candidate's typed practice answer to one
 * likely question — strengths, gaps vs the job ad, STAR completeness, and a
 * tightened rewrite. Routes through the same streaming pipeline (zero new IPC).
 * Session-only: nothing here persists to the aiGenerations aggregate. Returns
 * the raw delimited text — parse with `parseStarFeedback`.
 */
export async function generateStarFeedback(params: {
  question: string;
  answer: string;
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { question, answer, resume, jobAd, meta, model, signal, onToken } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });

  const system = buildStarFeedbackSystemPrompt();
  const user = buildStarFeedbackPrompt({
    question,
    answer,
    resume,
    jobAd,
    meta,
    target: profile,
    market,
  });
  // Feedback on the candidate's own answer — still prose (a written critique,
  // not a fixed-shape extraction), keyed off `questions` alongside the other
  // interview-prep surfaces above.
  const sampling = resolveSampling('questions', 'prose');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/** A résumé-ready project entry produced from one GitHub repo. Exactly the shape
 *  the resume builder's `projects` field array appends. `link` is the repo's
 *  canonical URL, re-attached verbatim post-parse — NEVER written by the AI. */
export interface GeneratedGitHubProject {
  name: string;
  description: string;
  link: string;
}

/** De-slug a repo name for the offline fallback title ("my-cool-app" → "My Cool App"). */
function deslugRepoName(name: string): string {
  return name
    .replace(/[-_./]+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Offline / failure fallback entry for one repo: real description (or a de-slugged
 *  name when empty), with the canonical link attached. Import always works. */
function fallbackProject(repo: GitHubRepo): GeneratedGitHubProject {
  const description = repo.description?.trim() || deslugRepoName(repo.name);
  return { name: deslugRepoName(repo.name), description, link: repo.htmlUrl };
}

/** Normalize a title/repo name to a match key: de-slug, lowercase, drop every
 *  non-alphanumeric. So "my-cool-app", "My Cool App", and "My_Cool_App" all key
 *  to "mycoolapp" — robust to the model de-slugging or re-spacing the title. */
function projectNameKey(name: string): string {
  return deslugRepoName(name)
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '');
}

/**
 * Turn selected GitHub repos into résumé-ready project entries via the AI provider.
 * Routes through the same streaming pipeline as the other generators (zero
 * per-provider code, NO new IPC) and the untrusted-data fence (a hostile repo
 * description can't steer the model). The model writes the title + bullets only;
 * each repo's `htmlUrl` is re-attached as `link` AFTER parsing — the AI never
 * writes a URL.
 *
 * Resilient by design: if streaming/parsing throws, or yields fewer entries than
 * repos, the missing repos fall back to their raw `description` (or de-slugged
 * name) so import ALWAYS works — even offline or with no provider configured.
 *
 * Each parsed entry is matched back to its repo by de-slugged NAME (case/space/
 * hyphen-insensitive), so a correct bullet always lands on the right repo's link
 * even if the model reorders or renames blocks; only entries with no name match
 * fall back to positional pairing. The `link` is ALWAYS the repo's own `htmlUrl`
 * (never the AI). Output is one entry per repo, in input order.
 */
export async function generateGitHubProjects(params: {
  repos: GitHubRepo[];
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<GeneratedGitHubProject[]> {
  const { repos, model, signal, onToken } = params;
  if (!repos.length) return [];

  let parsed: { name: string; description: string }[] = [];
  try {
    const profile = buildProviderProfile(model);
    const system = buildGitHubProjectsSystemPrompt();
    // Map the IPC repo shape → the prompt's URL-free input (the AI never sees a link).
    const user = buildGitHubProjectsPrompt(
      repos.map((r) => ({
        name: r.name,
        description: r.description,
        language: r.language,
        topics: r.topics,
        stars: r.stars,
        pushedAt: r.pushedAt,
      })),
      profile
    );
    // Résumé-ready bullets extracted from real repo data — factual, not
    // creative prose, so `deterministic`; keyed off `resume` (this content
    // lands directly in the résumé's `projects` field, the same surface
    // `generateResume`/`synthesizeResume` use), never had a penalty set applied.
    const sampling = resolveSampling('resume', 'deterministic');
    const raw = await streamGenerate(
      model,
      system,
      user,
      onToken ?? (() => {}),
      sampling.temperature,
      'en',
      signal,
      undefined,
      sampling.intent
    );
    // Parse the RAW stream, NOT extractPlainText(raw): extractPlainText deletes a
    // whole ```-fenced answer entirely, which a local model often emits — that
    // would silently drop every AI entry to the fallback. The parser strips
    // fences + inline markdown itself.
    parsed = parseGitHubProjects(raw);
  } catch {
    // No provider / offline / aborted-after-partial — fall back for every repo.
    parsed = [];
  }

  // Match each parsed entry to its repo by de-slugged NAME so a correct bullet
  // lands on the right repo's link even if the model reorders/renames blocks.
  // Build a name → entry index so each entry is consumed at most once.
  const byName = new Map<string, number>();
  parsed.forEach((entry, i) => {
    const key = projectNameKey(entry.name);
    if (key && !byName.has(key)) byName.set(key, i);
  });
  const used = new Array<boolean>(parsed.length).fill(false);

  return repos.map((repo, i) => {
    // Prefer a name match; fall back to the positional entry only if it is not
    // already claimed by another repo's name match.
    const nameIdx = byName.get(projectNameKey(repo.name));
    let entry: { name: string; description: string } | undefined;
    if (nameIdx !== undefined && !used[nameIdx]) {
      entry = parsed[nameIdx];
      used[nameIdx] = true;
    } else if (!used[i]) {
      entry = parsed[i];
      if (entry) used[i] = true;
    }

    const description = entry?.description.trim();
    if (description) {
      const name = entry?.name.trim() || deslugRepoName(repo.name);
      // Link is ALWAYS the repo's own URL — never the AI, never the matched entry.
      return { name, description, link: repo.htmlUrl };
    }
    return fallbackProject(repo);
  });
}

/** Per-{@link RewriteDocType} step, for the Ollama temperature override
 *  lookup only — inline rewrite ALWAYS uses `deterministic` intent
 *  regardless of docType (see {@link rewriteSelection}'s call site): a
 *  surgical edit to a selected span ("tighten this sentence") must not
 *  become a high-temperature/detector-resistant rewrite just because the
 *  surrounding document is prose — that would reintroduce drift/fabrication
 *  risk into exactly the span the user is deliberately hand-shaping. */
const REWRITE_STEP: Record<RewriteDocType, TemperatureStep> = {
  resume: 'resume',
  'cover-letter': 'cover',
  'application-answer': 'answers',
  email: 'cover',
};

/**
 * Inline AI rewrite of a selected span (F4). Mirrors {@link generateApplicationAnswer}:
 * reads the active provider config, computes the effective prompt tier, builds the
 * grounded rewrite prompt, and streams through the shared pipeline — so it works
 * for every provider with zero per-provider code and adds NO new IPC. The model is
 * instructed to return ONLY the rewritten span; `extractPlainText` strips any
 * stray markdown/thinking the model echoes. Pass `onToken` to stream the rewrite
 * into a preview and `signal` to abort an in-flight rewrite.
 */
export async function rewriteSelection(params: {
  selection: string;
  instruction: string;
  before: string;
  after: string;
  docType: RewriteDocType;
  model: string;
  /** Document language so the rewrite streams in the right locale (default 'en').
   *  Pass the generation's `meta.targetLanguage`. `streamGenerate` clamps it to a
   *  supported locale via `safeLocale`. */
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    selection,
    instruction,
    before,
    after,
    docType,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildRewritePrompt(
    { selection, instruction, before, after, docType },
    profile
  );
  // Bypassed `resolveTemperature`/`resolveSampling` entirely (bare `0.3`) —
  // now routed through the same per-docType Ollama override lookup every
  // other surface uses, but ALWAYS with `deterministic` intent (see
  // `REWRITE_STEP`'s doc comment): a surgical span edit must never inherit a
  // prose/detector-resistance profile just because the surrounding document
  // does.
  const sampling = resolveSampling(REWRITE_STEP[docType], 'deterministic');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Draft a single manual referral message (F3a) for the SELECTED format only —
 * one LLM call per format, never all three eagerly. Mirrors
 * {@link generateApplicationAnswer}: reads the active provider config, computes the
 * effective prompt tier, builds the grounded referral prompt, and streams through
 * the shared pipeline — so it works for every provider with zero per-provider code
 * and adds NO new IPC. The person's details are user-typed (no LinkedIn fetch).
 * `extractPlainText` strips any stray markdown/thinking the model echoes; the
 * connection-note ≤300 cap is enforced in the prompt and re-checked by the UI.
 */
export async function generateReferral(params: {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  format: ReferralFormat;
  /** Hard char cap for the body (defaults to 300 for connection notes). */
  charLimit?: number;
  model: string;
  /** Message language so it streams in the right locale (default 'en'). */
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    personName,
    personRole,
    companyName,
    jobTitle,
    resume,
    format,
    charLimit,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildReferralPrompt(
    { personName, personRole, companyName, jobTitle, resume, format, charLimit },
    profile
  );
  // Referral messages are `prose_grounded`, not plain `prose`: the prompt's
  // own contract is "grounded only in the candidate's résumé — no
  // fabrication, no invented shared history" (`referral.ts`), the same
  // no-fabrication requirement application answers have, so this drops
  // `presencePenalty` the same way (see `AiProvider::sampling_profile`).
  const sampling = resolveSampling('referral', 'prose_grounded');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Revise an existing referral draft per a user instruction (F3a improve). Mirrors
 * {@link generateReferral} in every way (provider config, streaming pipeline, no
 * new IPC) but uses {@link buildReferralImprovePrompt} so the revision preserves
 * the same honesty + résumé-grounding contract, channel shape, and the ≤300 hard
 * cap for connection notes.
 *
 * SECURITY: `instruction` MUST be user-originated. Never pass scraped job-ad text,
 * company-research briefs, or any untrusted source as the instruction — it is
 * treated as a live directive by the model. The draft and résumé are fenced.
 */
export async function generateReferralImprove(params: {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  draft: string;
  instruction: string;
  format: ReferralFormat;
  charLimit?: number;
  model: string;
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    personName,
    personRole,
    companyName,
    jobTitle,
    resume,
    draft,
    instruction,
    format,
    charLimit,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildReferralImprovePrompt(
    {
      personName,
      personRole,
      companyName,
      jobTitle,
      resume,
      draft,
      instruction,
      format,
      charLimit,
    },
    profile
  );
  // Referral messages are `prose_grounded`, not plain `prose`: the prompt's
  // own contract is "grounded only in the candidate's résumé — no
  // fabrication, no invented shared history" (`referral.ts`), the same
  // no-fabrication requirement application answers have, so this drops
  // `presencePenalty` the same way (see `AiProvider::sampling_profile`).
  const sampling = resolveSampling('referral', 'prose_grounded');
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling.intent
  );
  return extractPlainText(raw);
}

/**
 * Generate a short application email and stream tokens to the caller.
 * Returns the raw output — the caller splits on the first "Subject: " line
 * (see `buildApplicationEmailPrompt` OUTPUT CONTRACT). Mirrors
 * {@link generateCoverLetter}: same provider config, streaming pipeline, and
 * honesty contract — no new IPC.
 */
export async function generateApplicationEmail(params: {
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  recipientName?: string;
  recipientEmail?: string;
  companyBrief?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const {
    resume,
    jobAd,
    meta,
    model,
    recipientName,
    recipientEmail,
    companyBrief = '',
    signal,
    onToken,
  } = params;
  const profile = buildProviderProfile(model);
  // Same market resolution as the cover letter (job country first, letter
  // language as the fallback): the greeting and sign-off follow that market's
  // etiquette instead of an English default.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  const tone = usePreferencesStore.getState().outputTone;
  // No external writing-style sample: the résumé is already in
  // <candidate_resume>, and the prompt builder's own voice directive points
  // there instead of duplicating it (see buildResumeVoiceDirective).
  const { system, user } = buildApplicationEmailPrompt(
    { resume, jobAd, meta, recipientName, recipientEmail, companyBrief, market, tone },
    profile
  );
  // Application emails are `prose_grounded`, not plain `prose`: the prompt's
  // own contract is "No-fabrication: same résumé-grounded honesty contract"
  // (`application-email.ts`) — it makes candidate claims to a real employer,
  // so it drops `presencePenalty` the same way application answers do (see
  // `AiProvider::sampling_profile`).
  const sampling = resolveSampling('cover', 'prose_grounded');
  return streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage ?? 'en',
    signal,
    undefined,
    sampling.intent
  );
}
