/**
 * Regression tests for the centralized anti-AI-tell ruleset (natural-voice.ts)
 * and its wiring into five generation-prompt surfaces.
 *
 * Invariants under test:
 *  1. DASH-FREE CONSTANTS  — neither exported constant contains an em- or en-dash.
 *  2. COMPOSITION          — PROSE is a strict superset of LEXICAL; the em-dash ban
 *                            is the distinguishing PROSE-only addition.
 *  3. PROSE SURFACES       — cover-letter, referral, and application-questions system
 *                            prompts carry the ruleset and are dash-free, at every
 *                            depth they support (brief / task / full).
 *  4. COVER-LETTER EXEMPLAR— the COVER_LETTER_TONE_EXEMPLAR embedded in the full
 *                            system prompt is itself dash-free.
 *  5. RESUME CONTRAST      — resume system prompt carries LEXICAL but not the prose
 *                            em-dash-ban line; its date-range en-dash convention is
 *                            preserved.
 *  6. REWRITE ROUTING      — docType=cover_letter gets PROSE rules; docType=resume
 *                            gets LEXICAL only (prose em-dash-ban absent).
 */

import { describe, expect, it } from 'vitest';

import { buildApplicationAnswerSystemPrompt } from '../application-questions/index.js';
import { buildCoverLetterSystemPrompt } from '../cover-letter/index.js';
import { buildReferralPrompt } from '../referral/index.js';
import { buildResumeSystemPrompt } from '../resume/index.js';
import { buildRewritePrompt } from '../rewrite/index.js';
import { ANTI_AI_TELL_LEXICAL, ANTI_AI_TELL_PROSE } from './natural-voice.js';

// ─── stable phrase anchors ────────────────────────────────────────────────────
// These are phrases in the current source that uniquely identify a block.
// Anchored to *concepts* in the rule text, not whitespace/punctuation, so minor
// rephrasing doesn't break the tests but removal of the rule does.

/** A phrase stable enough to identify the LEXICAL block is present. */
const LEXICAL_ANCHOR = 'Drop AI-vocabulary';
/** The em-dash hard-ban line — present in PROSE only, never in LEXICAL alone. */
const PROSE_EMDASH_BAN = 'EM-DASH HARD BAN';

// ─── DepthTargets: one PromptTarget value per resolved depth ─────────────────
// cover-letter supports brief / task / full (see cover-letter.ts + provider/index.ts).
//   'small'  → depth 'brief'  (ollama, tier small)
//   {kind:'cli'} → depth 'task'
//   'large'  → depth 'full'   (ollama, tier large)
const BRIEF_TARGET = 'small' as const;
const TASK_TARGET = { kind: 'cli' } as const;
const FULL_TARGET = 'large' as const;

// A minimal resume so prompt builders don't throw on empty input.
const STUB_RESUME = 'Jane Dev\nSenior Engineer\njane@example.com\nSkills: TypeScript, React\n';

// ─── 1. DASH-FREE CONSTANTS ───────────────────────────────────────────────────

describe('ANTI_AI_TELL_LEXICAL — dash-free constant', () => {
  it('contains no em-dash (—)', () => {
    expect(ANTI_AI_TELL_LEXICAL).not.toMatch(/—/);
  });

  it('contains no en-dash (–)', () => {
    expect(ANTI_AI_TELL_LEXICAL).not.toMatch(/–/);
  });

  it('combined regex: no em-dash or en-dash', () => {
    expect(ANTI_AI_TELL_LEXICAL).not.toMatch(/[—–]/);
  });
});

describe('ANTI_AI_TELL_PROSE — dash-free constant', () => {
  it('contains no em-dash (—)', () => {
    expect(ANTI_AI_TELL_PROSE).not.toMatch(/—/);
  });

  it('contains no en-dash (–)', () => {
    expect(ANTI_AI_TELL_PROSE).not.toMatch(/–/);
  });

  it('combined regex: no em-dash or en-dash', () => {
    expect(ANTI_AI_TELL_PROSE).not.toMatch(/[—–]/);
  });
});

// ─── 2. COMPOSITION ──────────────────────────────────────────────────────────

describe('ANTI_AI_TELL_PROSE composition', () => {
  it('includes the full LEXICAL text (single source of truth)', () => {
    // PROSE is built via template literal starting with LEXICAL, so the entire
    // LEXICAL string must appear verbatim inside PROSE.
    expect(ANTI_AI_TELL_PROSE).toContain(ANTI_AI_TELL_LEXICAL);
  });

  it('adds the em-dash ban line that LEXICAL does not contain', () => {
    expect(ANTI_AI_TELL_PROSE).toMatch(new RegExp(PROSE_EMDASH_BAN));
    expect(ANTI_AI_TELL_LEXICAL).not.toMatch(new RegExp(PROSE_EMDASH_BAN));
  });

  it('PROSE is strictly longer than LEXICAL', () => {
    expect(ANTI_AI_TELL_PROSE.length).toBeGreaterThan(ANTI_AI_TELL_LEXICAL.length);
  });

  it('LEXICAL contains the lexical-ban anchor phrase', () => {
    expect(ANTI_AI_TELL_LEXICAL).toContain(LEXICAL_ANCHOR);
  });
});

// ─── 3. PROSE SURFACES — cover-letter ────────────────────────────────────────

describe('buildCoverLetterSystemPrompt — carries PROSE ruleset, dash-free, all depths', () => {
  for (const [label, target] of [
    ['brief (small)', BRIEF_TARGET],
    ['task (cli)', TASK_TARGET],
    ['full (large)', FULL_TARGET],
  ] as const) {
    describe(`depth: ${label}`, () => {
      it('system prompt carries the LEXICAL-ban anchor', () => {
        const prompt = buildCoverLetterSystemPrompt('recruiter', target);
        expect(prompt).toContain(LEXICAL_ANCHOR);
      });

      it('system prompt carries the PROSE em-dash-ban line', () => {
        const prompt = buildCoverLetterSystemPrompt('recruiter', target);
        expect(prompt).toContain(PROSE_EMDASH_BAN);
      });

      it('assembled system prompt has no em-dash (—)', () => {
        const prompt = buildCoverLetterSystemPrompt('recruiter', target);
        expect(prompt).not.toMatch(/—/);
      });

      it('assembled system prompt has no en-dash (–)', () => {
        const prompt = buildCoverLetterSystemPrompt('recruiter', target);
        expect(prompt).not.toMatch(/–/);
      });
    });
  }
});

// ─── 3. PROSE SURFACES — referral ────────────────────────────────────────────
// referral uses a single buildReferralPrompt builder; depth varies by tier target.
// All three tier targets are tested so every depth path is covered.

describe('buildReferralPrompt — carries PROSE ruleset, dash-free, all tier targets', () => {
  const BASE_PARAMS = {
    personName: 'Alex Kim',
    companyName: 'Acme',
    jobTitle: 'Senior Engineer',
    resume: STUB_RESUME,
    format: 'email' as const,
  };

  for (const [label, target] of [
    ['small (brief)', BRIEF_TARGET],
    ['task (cli)', TASK_TARGET],
    ['large (full)', FULL_TARGET],
  ] as const) {
    describe(`tier: ${label}`, () => {
      it('system prompt carries the LEXICAL-ban anchor', () => {
        const { system } = buildReferralPrompt(BASE_PARAMS, target);
        expect(system).toContain(LEXICAL_ANCHOR);
      });

      it('system prompt carries the PROSE em-dash-ban line', () => {
        const { system } = buildReferralPrompt(BASE_PARAMS, target);
        expect(system).toContain(PROSE_EMDASH_BAN);
      });

      it('assembled system prompt has no em-dash (—)', () => {
        const { system } = buildReferralPrompt(BASE_PARAMS, target);
        expect(system).not.toMatch(/—/);
      });

      it('assembled system prompt has no en-dash (–)', () => {
        const { system } = buildReferralPrompt(BASE_PARAMS, target);
        expect(system).not.toMatch(/–/);
      });
    });
  }
});

// ─── 3. PROSE SURFACES — application-questions ───────────────────────────────

describe('buildApplicationAnswerSystemPrompt — carries PROSE ruleset, dash-free', () => {
  it('system prompt carries the LEXICAL-ban anchor', () => {
    const prompt = buildApplicationAnswerSystemPrompt();
    expect(prompt).toContain(LEXICAL_ANCHOR);
  });

  it('system prompt carries the PROSE em-dash-ban line', () => {
    const prompt = buildApplicationAnswerSystemPrompt();
    expect(prompt).toContain(PROSE_EMDASH_BAN);
  });

  it('assembled system prompt has no em-dash (—)', () => {
    expect(buildApplicationAnswerSystemPrompt()).not.toMatch(/—/);
  });

  it('assembled system prompt has no en-dash (–)', () => {
    expect(buildApplicationAnswerSystemPrompt()).not.toMatch(/–/);
  });
});

// ─── 4. COVER-LETTER EXEMPLAR is dash-free ───────────────────────────────────
// The tone exemplar is embedded only in the 'full' depth system prompt.

describe('cover-letter tone exemplar — dash-free', () => {
  it('the full system prompt (which includes the tone exemplar) has no em-dash', () => {
    // The full depth is where COVER_LETTER_TONE_EXEMPLAR is rendered.
    const prompt = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET);
    expect(prompt).toContain('TONE REFERENCE');
    expect(prompt).not.toMatch(/—/);
  });

  it('the full system prompt (which includes the tone exemplar) has no en-dash', () => {
    const prompt = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET);
    expect(prompt).not.toMatch(/–/);
  });
});

// ─── 5. RESUME CONTRAST ──────────────────────────────────────────────────────

describe('buildResumeSystemPrompt — LEXICAL only, deliberate en-dash date convention kept', () => {
  // Depth facts (verified against resume.ts + provider/index.ts):
  //   brief (small) → buildResumeSystemPrompt inline body: contains literal en-dash
  //                   in "January 2021 – March 2023" date example.
  //   task  (cli)   → buildResumeSystemTaskBrief: contains literal en-dash in the
  //                   numeric range "max 2–3 per bullet" (line 77). No date example,
  //                   no PROSE_EMDASH_BAN, no negative parallelisms rule.
  //   full  (large) → buildResumeSystemFull: contains "Always use en-dash (–) not
  //                   hyphen (-) for date ranges" — literal en-dash present.
  // All three depths contain at least one literal en-dash for different legitimate
  // reasons; all three deliberately omit PROSE_EMDASH_BAN.

  for (const [label, target] of [
    ['brief (small)', BRIEF_TARGET],
    ['task (cli)', TASK_TARGET],
    ['full (large)', FULL_TARGET],
  ] as const) {
    describe(`depth: ${label}`, () => {
      it('carries the LEXICAL-ban anchor', () => {
        expect(buildResumeSystemPrompt('ats', target)).toContain(LEXICAL_ANCHOR);
      });

      it('does NOT carry the prose em-dash-ban line', () => {
        // The resume deliberately omits the em-dash HARD BAN because resume
        // bullet conventions differ from prose.
        expect(buildResumeSystemPrompt('ats', target)).not.toContain(PROSE_EMDASH_BAN);
      });

      it('does NOT carry PROSE-only prose-flow rules (negative parallelism ban)', () => {
        // "No negative parallelisms" is a PROSE-only rule. Its absence guards
        // the boundary — resume bullets must not be burdened with prose-flow rules.
        expect(buildResumeSystemPrompt('ats', target)).not.toContain('No negative parallelisms');
      });

      it('preserves a deliberate en-dash (numeric range or date-format instruction)', () => {
        // Every resume depth intentionally embeds at least one literal en-dash:
        //   brief → date example "January 2021 – March 2023"
        //   task  → numeric range "max 2–3 per bullet" in the acceptance check
        //   full  → "Always use en-dash (–) not hyphen (-) for date ranges"
        // This assertion guards against accidentally removing these carve-outs.
        expect(buildResumeSystemPrompt('ats', target)).toMatch(/–/);
      });
    });
  }
});

// ─── 6. REWRITE ROUTING ──────────────────────────────────────────────────────

describe('buildRewritePrompt — docType routes to correct voice ruleset', () => {
  const BASE = {
    selection: 'Led the migration of the billing platform to microservices.',
    instruction: 'Make it punchier.',
    before: 'WORK EXPERIENCE\nAcme Corp — Staff Engineer (2021–2024)\n',
    after: '\nSkills: TypeScript',
  };

  describe('docType=cover-letter → PROSE rules', () => {
    it('system prompt carries the LEXICAL-ban anchor', () => {
      const { system } = buildRewritePrompt({ ...BASE, docType: 'cover-letter' });
      expect(system).toContain(LEXICAL_ANCHOR);
    });

    it('system prompt carries the PROSE em-dash-ban line', () => {
      const { system } = buildRewritePrompt({ ...BASE, docType: 'cover-letter' });
      expect(system).toContain(PROSE_EMDASH_BAN);
    });

    it('system prompt contains the prose-flow section header', () => {
      const { system } = buildRewritePrompt({ ...BASE, docType: 'cover-letter' });
      expect(system).toContain('PROSE FLOW');
    });
  });

  describe('docType=resume → LEXICAL rules only', () => {
    it('system prompt carries the LEXICAL-ban anchor', () => {
      const { system } = buildRewritePrompt({ ...BASE, docType: 'resume' });
      expect(system).toContain(LEXICAL_ANCHOR);
    });

    it('system prompt does NOT carry the PROSE em-dash-ban line', () => {
      const { system } = buildRewritePrompt({ ...BASE, docType: 'resume' });
      expect(system).not.toContain(PROSE_EMDASH_BAN);
    });

    it('system prompt does NOT contain the prose-flow section header', () => {
      const { system } = buildRewritePrompt({ ...BASE, docType: 'resume' });
      expect(system).not.toContain('PROSE FLOW');
    });
  });
});
