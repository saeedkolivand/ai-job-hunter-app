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
  validateMetadata,
} from '@ajh/prompts/generate';
import type { ContactProfile, GitHubRepo } from '@ajh/shared';
import { detectLanguages, getLanguageName, toLanguageCode } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from '../../app-client';
import { OUTPUT_LANGUAGES, safeLocale } from '../locales';
import {
  buildProviderProfile,
  resolveActiveProvider,
  resolveEffectiveTier,
} from '../provider-context';
import { awaitAiStream } from '../stream-promise';

export type { GenerationMeta, GenerationMode };
export { MODES } from '@ajh/prompts/generate';

// ─── LLM helpers ─────────────────────────────────────────────────────────────

/** One generation step that can carry its own per-model temperature override. */
type TemperatureStep = 'analysis' | 'resume' | 'cover' | 'answers' | 'referral';

/** Effective sampling temperature for one generation step. A user-set per-model,
 *  per-step temperature override (settings → local model limits) wins for that
 *  step; otherwise the per-step default applies. Each step is independent — an
 *  unset step falls back to its default. Override is Ollama-only — cloud/CLI
 *  providers always use the per-step default. */
function resolveTemperature(step: TemperatureStep, stepDefault: number): number {
  // The active provider/model come from the backend store (task #16); the per-model
  // temperature override is a renderer-side tuning knob (Ollama-only) read from the
  // resolver's Zustand-sourced `providerSettings`.
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider();
  if (activeProvider !== 'ollama') return stepDefault;
  const override = activeModel
    ? providerSettings?.modelLimits?.[activeModel]?.temperature?.[step]
    : undefined;
  return override ?? stepDefault;
}

/** Effective sampling parameters for one generation step. */
interface SamplingParams {
  temperature: number;
  topP?: number;
  frequencyPenalty?: number;
  presencePenalty?: number;
  repeatPenalty?: number;
}

// ponytail: detector-resistance sampling knobs. RAID (ACL 2024) found that
// random sampling + repetition/frequency penalties drop AI-detector accuracy
// by up to 38 points — today the app only plumbs temperature. Applied ONLY to
// PROSE generation surfaces (cover letter, application answers, email,
// referral, interview); resume/analysis/inline-rewrite stay excluded because
// frequency/presence penalties would suppress the exact job-ad keyword
// repetition ATS keyword-matching needs. NOTE: on Anthropic's extended-thinking
// path these knobs are a near-no-op — `top_p` is dropped and temperature is
// forced to 1.0 (the API rejects `top_p` alongside `thinking`), and Anthropic
// has no frequency/presence/repeat penalty params at all — don't assume this
// set is "active" there.
const PROSE_SAMPLING = {
  topP: 0.95,
  frequencyPenalty: 0.3,
  presencePenalty: 0.2,
  repeatPenalty: 1.15,
} as const;

/** Generalizes {@link resolveTemperature} into a per-step sampling resolver:
 *  the temperature override lookup is unchanged, and `prose: true` layers on
 *  the shared {@link PROSE_SAMPLING} penalty set for detector-resistant steps.
 *  `overrides` lets one surface tune a specific knob (e.g. drop a penalty, or
 *  tighten topP for a drift-prone small model) without forking the shared set. */
function resolveSampling(
  step: TemperatureStep,
  temperatureDefault: number,
  prose = false,
  overrides?: Partial<Omit<SamplingParams, 'temperature'>>
): SamplingParams {
  const temperature = resolveTemperature(step, temperatureDefault);
  return prose ? { temperature, ...PROSE_SAMPLING, ...overrides } : { temperature };
}

async function streamGenerate(
  model: string,
  system: string,
  user: string,
  onToken: (tok: string) => void,
  temperature = 0.3,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void,
  sampling?: Omit<SamplingParams, 'temperature'>
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
    temperature,
    // Detector-resistance sampling knobs — present only for prose steps that
    // opted in (see PROSE_SAMPLING); omitted (undefined) everywhere else.
    topP: sampling?.topP,
    frequencyPenalty: sampling?.frequencyPenalty,
    presencePenalty: sampling?.presencePenalty,
    repeatPenalty: sampling?.repeatPenalty,
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

  return awaitAiStream(api, res.jobId, { onToken, onThinking, signal });
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
    // Analysis carries its own per-model temperature override (user's chosen design).
    const raw = await streamGenerate(
      model,
      system,
      user,
      () => {},
      resolveTemperature('analysis', 0.15),
      locale
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
  } catch {
    /* fall through */
  }

  const nameMatch = resume.match(/^([A-Z][a-z]+ [A-Z][a-z]+(?:\s[A-Z][a-z]+)?)/m);
  const titleMatch = jobAd.match(/(?:position|role|title|job)[:\s]+([^\n]+)/i);
  const companyMatch = jobAd.match(/(?:at|@|company|employer|firm)[:\s]+([^\n,]+)/i);
  return {
    candidateName: nameMatch?.[1] ?? '',
    jobTitle: titleMatch?.[1]?.trim() ?? '',
    companyName: companyMatch?.[1]?.trim() ?? '',
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
 * Index 0 is never a candidate for the replacement scan below (which starts
 * at `i = 1`), including when there's no `fullName` and line 0 is already
 * contact-shaped by Rust's narrower line-0 rule (`isFirstLineContactShaped`)
 * — a combined "Jane Doe | jane@example.com" with no separate name line.
 * Were it eligible and the ONLY candidate, overwriting it would erase "Jane
 * Doe" entirely (there is no separate name line to fall back to), which is
 * worse than leaving it untouched and inserting the profile's line right
 * after — so it always falls to the "insert" branch instead.
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
  if (fullName) {
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

  if (contactLine.trim()) {
    // The header block: line 0 up to (not including) the first blank line,
    // or the whole text if there is none. Hard ceiling for the scan below —
    // see the STRUCTURAL bound note above.
    let blockEnd = lines.length;
    for (let i = 1; i < lines.length; i++) {
      if ((lines[i] ?? '').trim() === '') {
        blockEnd = i;
        break;
      }
    }

    // Starts at i = 1: index 0 is never a candidate here — see the doc
    // comment above.
    const matches: number[] = [];
    for (let i = 1; i < blockEnd; i++) {
      const line = lines[i] ?? '';
      if (looksLikeHeaderBoundary(line)) break;
      if (isHeaderContactLine(line)) matches.push(i);
    }

    if (matches.length > 0) {
      lines[pickReplacementIndex(lines, matches)] = contactLine;
    } else {
      // CodeRabbit (security re-review): index 1 assumes line 0 is always
      // the name — true after the fullName-driven unshift/replace above, or
      // when the model already wrote a name/contact line on its own. But
      // when there's no fullName to seed AND line 0 is itself a section
      // heading (the model omitted the name line entirely), splicing at 1
      // put the contact line INSIDE that section, right under its heading,
      // not in the header block above it. Insert at 0 (ahead of the
      // heading) in that case instead.
      const insertAt = looksLikeHeaderBoundary(lines[0] ?? '') ? 0 : 1;
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
 */
async function seedHeaderFromContactProfile(
  text: string,
  meta: GenerationMeta,
  locale: string
): Promise<string> {
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
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    resolveTemperature('resume', 0.3),
    locale,
    signal,
    onThinking
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
  return seedHeaderFromContactProfile(injected, meta, locale);
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
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    resolveTemperature('resume', 0.3),
    locale,
    signal,
    onThinking
  );
  return seedHeaderFromContactProfile(extractPlainText(raw), meta, locale);
}

/**
 * Best-effort company research for the cover-letter "fit" paragraph. Routes
 * through the backend enricher — the active provider's own web search +
 * synthesis, cached. Any failure or a provider that can't search yields '' so
 * the cover letter still generates. The returned brief is untrusted reference
 * text — the prompt fences it.
 */
export async function researchCompany(jobAd: string, company?: string): Promise<string> {
  try {
    // Routing (provider/model/base_url) is backend-owned (task #16) — the enricher
    // reads the active provider from the store, so nothing is threaded here.
    const res = await getClient().ai.researchCompany({
      jobAd,
      // The AI-extracted company name is far more reliable than the backend's
      // heuristic job-ad scan (which can grab a tagline), so send it when known.
      company: company?.trim() || undefined,
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
  const { activeModel, activeProvider } = resolveActiveProvider(model);
  const tier = resolveEffectiveTier(activeModel, activeProvider);
  const profile = buildProviderProfile(model);

  // Opt-in: fetch a company brief and fold it into the prompt's fit paragraph.
  const companyBrief = opts?.researchCompany ? await researchCompany(jobAd, meta.companyName) : '';

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
  const system = buildCoverLetterSystemPrompt(mode, profile, tone, meta.targetLanguage);
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
  // Cover letters are prose: more temperature + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) loosens the phrasing so it reads human, not
  // mechanical, and resists AI-detector fingerprinting. Small models stay lower
  // to limit drift (raised proportionally from the previous 0.4/0.55 split). A
  // per-model override (if set) wins over this tier-based default. Small local
  // models (7-8B) also compound drift when the full topP randomness stacks with
  // repeatPenalty, so tighten topP for the small tier only; large stays at the
  // shared PROSE_SAMPLING default.
  const stepDefault = tier === 'small' ? 0.58 : 0.8;
  const sampling = resolveSampling(
    'cover',
    stepDefault,
    true,
    tier === 'small' ? { topP: 0.9 } : undefined
  );
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    sampling.temperature,
    locale,
    signal,
    onThinking,
    sampling
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
  // Application answers are prose but résumé-grounded (no-fabrication surface):
  // keep topP/frequencyPenalty/repeatPenalty for detector resistance, but drop
  // presencePenalty (it pushes toward new topics, which risks factual drift
  // here) and use a lower temperature than the freer prose surfaces (cover
  // letter, referral) to keep answers traceable to the résumé.
  const sampling = resolveSampling('answers', 0.5, true, { presencePenalty: undefined });
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling
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
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    resolveTemperature('answers', 0.3),
    lang?.code ?? meta?.targetLanguage ?? 'en',
    signal
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
  // Interview questions are prose: keep the existing 0.5 temperature default,
  // adding only the shared detector-resistance penalty set (see PROSE_SAMPLING).
  const sampling = resolveSampling('answers', 0.5, true);
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
    sampling
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
  // Prose, same detector-resistance treatment as the other interview surfaces.
  const sampling = resolveSampling('answers', 0.5, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling
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
  // Slightly lower temperature than free-form prose — feedback should stay
  // traceable to the candidate's actual answer, not wander.
  const sampling = resolveSampling('answers', 0.4, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling
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
    const raw = await streamGenerate(
      model,
      system,
      user,
      onToken ?? (() => {}),
      resolveTemperature('answers', 0.4),
      'en',
      signal
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
  const raw = await streamGenerate(model, system, user, onToken ?? (() => {}), 0.3, locale, signal);
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
  // Referral messages are prose: randomness + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) resist AI-detector fingerprinting.
  const sampling = resolveSampling('referral', 0.7, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling
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
  // Referral messages are prose: randomness + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) resist AI-detector fingerprinting.
  const sampling = resolveSampling('referral', 0.7, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling
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
  // Application emails are prose: randomness + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) resist AI-detector fingerprinting.
  const sampling = resolveSampling('cover', 0.7, true);
  return streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage ?? 'en',
    signal,
    undefined,
    sampling
  );
}
