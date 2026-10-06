import { describe, expect, it } from 'vitest';

import type { PromptTarget } from '../../provider/index.js';
import { buildApplicationEmailPrompt } from '../application-email/index.js';
import { buildApplicationAnswerSystemPrompt } from '../application-questions/index.js';
import { buildCoverLetterSystemPrompt } from '../cover-letter/index.js';
import {
  buildLikelyQuestionsSystemPrompt,
  buildStarFeedbackSystemPrompt,
} from '../interview-practice/index.js';
import { buildInterviewQuestionsSystemPrompt } from '../interview-questions/index.js';
import type { GenerationMeta } from '../modes/index.js';
import { buildReferralImprovePrompt, buildReferralPrompt } from '../referral/index.js';
import { buildResumeSystemPrompt } from '../resume/index.js';
import { buildRewritePrompt } from '../rewrite/index.js';
import { antiAiTellLexical, antiAiTellProse } from './natural-voice.js';
import {
  BRIEF_TARGET,
  FULL_TARGET,
  LEXICAL_ANCHOR,
  PROSE_EMDASH_BAN,
  STUB_RESUME,
  TASK_TARGET,
} from './test-support';

// ─── 17. BOLD-BAN SCOPE ──────────────────────────────────────────────────────
// The shared prose block's formatting rule used to read "no bold sprinkled
// mid-sentence for emphasis" — an UNQUALIFIED ban, landing above four letter
// instructions that require bolding 3 to 4 job-ad keywords. That bolding is a
// real downstream feature (the exporter/parser consumes the `**`), so the two
// rules have to coexist: the ban is scoped to DECORATIVE bold, and the
// requirement survives at every depth.

describe('bold: the decorative ban and the job-ad-keyword requirement coexist', () => {
  const DEPTHS = [
    ['brief (small)', BRIEF_TARGET],
    ['task (cli)', TASK_TARGET],
    ['full (large)', FULL_TARGET],
  ] as const;
  /** The unqualified wording that contradicted the output rules. */
  const UNQUALIFIED_BOLD_BAN = 'no bold sprinkled mid-sentence';
  /** The scoped replacement — a ban that names its own exception. */
  const SCOPED_BOLD_BAN = 'no decorative bold';

  it.each(DEPTHS)('the letter prompt still requires job-ad-keyword bolding at %s', (_l, target) => {
    const prompt = buildCoverLetterSystemPrompt('recruiter', target, undefined, 'en');
    expect(prompt).toMatch(/3 to 4 job-ad keywords/);
    expect(prompt).toMatch(/\*\*/);
  });

  it.each(DEPTHS)('the letter prompt carries no unqualified bold ban at %s', (_l, target) => {
    const prompt = buildCoverLetterSystemPrompt('recruiter', target, undefined, 'en');
    expect(prompt).not.toContain(UNQUALIFIED_BOLD_BAN);
  });

  // Only `task`/`full` carry a bold ban at all (the guidance tier is where the
  // formatting rule lives), so those are the only depths where "the ban names
  // its exception" is a claim with content. The earlier version of this ran
  // over all three depths behind an `if (!/\bbold\b/)` guard that could never
  // fire — the letter's own "Bold only 3 to 4 job-ad keywords" rule contains
  // the word — and then did nothing at `brief`, where `scoped` is false.
  const BAN_BEARING_DEPTHS = [
    ['task (cli)', TASK_TARGET],
    ['full (large)', FULL_TARGET],
  ] as const;

  it.each(BAN_BEARING_DEPTHS)(
    'at %s the bold ban and the bolding requirement are both present and compatible',
    (_l, target) => {
      const prompt = buildCoverLetterSystemPrompt('recruiter', target, undefined, 'en');
      expect(prompt).toContain(SCOPED_BOLD_BAN);
      expect(prompt).toMatch(/no decorative bold beyond the .*job-ad keywords/);
      expect(prompt).toMatch(/3 to 4 job-ad keywords/);
    }
  );

  it('the BRIEF letter prompt carries no bold BAN at all, only the bolding rule', () => {
    const prompt = buildCoverLetterSystemPrompt('recruiter', BRIEF_TARGET, undefined, 'en');
    expect(prompt).not.toContain(SCOPED_BOLD_BAN);
    expect(prompt).not.toContain(UNQUALIFIED_BOLD_BAN);
    expect(prompt).toMatch(/3 to 4 job-ad keywords/);
  });

  it('the résumé prompt keeps its own keyword-emphasis rule unopposed', () => {
    for (const target of [BRIEF_TARGET, TASK_TARGET, FULL_TARGET]) {
      const prompt = buildResumeSystemPrompt('ats', target, undefined, 'en');
      expect(prompt).not.toContain(UNQUALIFIED_BOLD_BAN);
      expect(prompt).toMatch(/\*\*double asterisks\*\*|\*\*bold\*\*/);
    }
  });

  // Every OTHER prose surface composes the same block, and none of them asks
  // for bold — the scoped wording has to stay true there too (it does: those
  // prompts require no bold, so "beyond what the output rules ask for" is zero).
  it('the shared prose block never states the ban unqualified', () => {
    expect(antiAiTellProse('en')).not.toContain(UNQUALIFIED_BOLD_BAN);
    expect(antiAiTellProse('en')).toContain(SCOPED_BOLD_BAN);
  });
});

// ─── 18. THE DEPTH TIER REACHES EVERY SURFACE ────────────────────────────────
// Section 16 proves the BLOCK is depth-aware. That is only half the wiring: a
// surface that calls `antiAiTellProse()` with no depth silently gets `full`,
// so the guidance tier reached five prose surfaces at EVERY depth even after
// the split (referral generate + improve, application answers, interview
// questions, likely questions, STAR feedback, inline rewrite). A brief referral
// connection note carried a 4200-character style block for a 3-sentence output.
//
// One case per surface, both directions asserted, so dropping any single
// surface's threading fails that surface's own pin rather than a shared one.

describe('depth reaches every surface that composes the anti-AI-tell block', () => {
  /** Judgement lines the `full`/`task` tier adds — absent at `brief`. */
  const PROSE_GUIDANCE_ANCHOR = 'No colon reveals';
  const LEXICAL_GUIDANCE_ANCHOR = 'PORTABILITY TEST';

  const REFERRAL_PARAMS = {
    personName: 'Alex Kim',
    companyName: 'Acme',
    jobTitle: 'Senior Engineer',
    resume: STUB_RESUME,
    format: 'connection_note' as const,
  };
  const IMPROVE_PARAMS = {
    ...REFERRAL_PARAMS,
    draft: 'Hi Alex, I saw the Senior Engineer role at Acme and wondered if you might refer me.',
    instruction: 'make it warmer',
  };
  const REWRITE_PARAMS = {
    selection: 'I built the settlement ledger.',
    instruction: 'tighten it',
    before: 'At Acme I worked on payments. ',
    after: ' It still runs nightly.',
  };

  const EMAIL_PARAMS = {
    resume: STUB_RESUME,
    jobAd: 'Acme is hiring a Senior Engineer to scale the settlement platform.',
    meta: {
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      candidateName: 'Jane Dev',
      jobTitle: 'Senior Engineer',
      companyName: 'Acme',
      targetLanguage: 'en',
      topRequirements: ['Rust', 'payments'],
    } satisfies GenerationMeta,
  };

  /** Every prose surface, as a builder taking only the provider target. */
  const PROSE_SURFACES: ReadonlyArray<readonly [string, (target: PromptTarget) => string]> = [
    ['referral (generate)', (t) => buildReferralPrompt(REFERRAL_PARAMS, t).system],
    ['referral (improve)', (t) => buildReferralImprovePrompt(IMPROVE_PARAMS, t).system],
    ['application answers', (t) => buildApplicationAnswerSystemPrompt(undefined, 'en', t)],
    // Absent from this list until round 4: the block was composed in the `full`
    // branch only, so threading `depth` had nothing to tier on the other two.
    ['application email', (t) => buildApplicationEmailPrompt(EMAIL_PARAMS, t).system],
    ['interview questions', (t) => buildInterviewQuestionsSystemPrompt('en', t)],
    ['likely interview questions', (t) => buildLikelyQuestionsSystemPrompt(t)],
    ['STAR feedback', (t) => buildStarFeedbackSystemPrompt(t)],
    [
      'inline rewrite (cover-letter span)',
      (t) => buildRewritePrompt({ ...REWRITE_PARAMS, docType: 'cover-letter' }, t).system,
    ],
    [
      'inline rewrite (application-answer span)',
      (t) => buildRewritePrompt({ ...REWRITE_PARAMS, docType: 'application-answer' }, t).system,
    ],
    [
      'inline rewrite (email span)',
      (t) => buildRewritePrompt({ ...REWRITE_PARAMS, docType: 'email' }, t).system,
    ],
  ];

  it.each(PROSE_SURFACES)('%s keeps the checked core at BRIEF depth', (_name, build) => {
    expect(build(BRIEF_TARGET)).toContain(PROSE_EMDASH_BAN);
    expect(build(BRIEF_TARGET)).toContain(LEXICAL_ANCHOR);
  });

  it.each(PROSE_SURFACES)('%s drops the construction guidance at BRIEF depth', (_name, build) => {
    expect(build(FULL_TARGET)).toContain(PROSE_GUIDANCE_ANCHOR);
    expect(build(BRIEF_TARGET)).not.toContain(PROSE_GUIDANCE_ANCHOR);
  });

  it.each(PROSE_SURFACES)('%s composes the brief block verbatim at BRIEF depth', (_name, build) => {
    expect(build(BRIEF_TARGET)).toContain(antiAiTellProse('en', 'brief'));
    expect(build(FULL_TARGET)).toContain(antiAiTellProse('en'));
  });

  // The résumé-tier rewrite span takes the LEXICAL block, so it has its own
  // anchor — same wiring, different tier.
  it('the inline rewrite of a RÉSUMÉ span is depth-aware on the lexical tier', () => {
    const build = (t: PromptTarget) =>
      buildRewritePrompt({ ...REWRITE_PARAMS, docType: 'resume' }, t).system;
    expect(build(FULL_TARGET)).toContain(LEXICAL_GUIDANCE_ANCHOR);
    expect(build(BRIEF_TARGET)).not.toContain(LEXICAL_GUIDANCE_ANCHOR);
    expect(build(BRIEF_TARGET)).toContain(antiAiTellLexical('en', 'brief'));
    expect(build(FULL_TARGET)).toContain(antiAiTellLexical('en'));
  });

  it.each(PROSE_SURFACES)('%s is materially smaller at BRIEF than at FULL', (_name, build) => {
    expect(build(BRIEF_TARGET).length).toBeLessThan(build(FULL_TARGET).length);
  });
});
