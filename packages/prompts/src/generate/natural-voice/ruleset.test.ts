import { describe, expect, it } from 'vitest';

import { buildApplicationAnswerSystemPrompt } from '../application-questions/index.js';
import { buildCoverLetterSystemPrompt } from '../cover-letter/index.js';
import { buildReferralPrompt } from '../referral/index.js';
import { buildResumeSystemPrompt } from '../resume/index.js';
import {
  AI_TELL_LEXICAL_WORDS_DE,
  AI_TELL_LEXICAL_WORDS_EN,
  AI_TELL_LEXICAL_WORDS_IT,
  AI_TELL_PROSE_WORDS_DE,
  AI_TELL_PROSE_WORDS_EN,
  AI_TELL_PROSE_WORDS_IT,
  HUMANIZE_LEXICAL,
  HUMANIZE_PROSE,
  TEMPLATE_OPENERS_DE,
  TEMPLATE_OPENERS_EN,
  TEMPLATE_OPENERS_IT,
} from './natural-voice.js';
import {
  ANTI_AI_TELL_LEXICAL,
  ANTI_AI_TELL_PROSE,
  BRIEF_TARGET,
  FULL_TARGET,
  HUMANIZE_LEXICAL_ANCHOR,
  HUMANIZE_PROSE_ANCHOR,
  itCarriesProseRuleset,
  LEXICAL_ANCHOR,
  PROSE_EMDASH_BAN,
  STUB_RESUME,
  TASK_TARGET,
} from './test-support';

// ─── 1. DASH-FREE CONSTANTS ───────────────────────────────────────────────────

describe.each([
  ['ANTI_AI_TELL_LEXICAL', ANTI_AI_TELL_LEXICAL],
  ['ANTI_AI_TELL_PROSE', ANTI_AI_TELL_PROSE],
])('%s — dash-free constant', (_name, text) => {
  it('contains no em-dash (—)', () => {
    expect(text).not.toMatch(/—/);
  });

  it('contains no en-dash (–)', () => {
    expect(text).not.toMatch(/–/);
  });

  it('combined regex: no em-dash or en-dash', () => {
    expect(text).not.toMatch(/[—–]/);
  });
});

describe('HUMANIZE_LEXICAL — dash-free constant', () => {
  it('contains no em-dash or en-dash', () => {
    expect(HUMANIZE_LEXICAL).not.toMatch(/[—–]/);
  });

  it('carries the bullet-variety anchor and stays honesty-subordinate', () => {
    expect(HUMANIZE_LEXICAL).toContain(HUMANIZE_LEXICAL_ANCHOR);
    expect(HUMANIZE_LEXICAL).toMatch(/never licenses a new fact|already.*in the resume/i);
  });

  it('never introduces prose-imperfection (no CADENCE/CONTROLLED IMPERFECTION language)', () => {
    expect(HUMANIZE_LEXICAL).not.toContain(HUMANIZE_PROSE_ANCHOR);
    expect(HUMANIZE_LEXICAL).not.toContain('CONTROLLED IMPERFECTION');
  });
});

describe('HUMANIZE_PROSE — dash-free constant', () => {
  it('contains no em-dash or en-dash', () => {
    expect(HUMANIZE_PROSE).not.toMatch(/[—–]/);
  });

  it('carries the cadence anchor and stays honesty-subordinate', () => {
    expect(HUMANIZE_PROSE).toContain(HUMANIZE_PROSE_ANCHOR);
    expect(HUMANIZE_PROSE).toMatch(/honesty rules above require/i);
  });

  it('gates controlled imperfection to the requested register (never a typo/grammar error)', () => {
    expect(HUMANIZE_PROSE).toMatch(/CONTROLLED IMPERFECTION/);
    expect(HUMANIZE_PROSE).toMatch(/never a typo or a grammar mistake/i);
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

// ─── 12. ARRAY -> PROMPT DIRECTION GUARD ──────────────────────────────────────
// The prompt -> Rust direction is already pinned mechanically: `pnpm
// gen:prompts:check` (CI) fails whenever `lexicon.rs` drifts from these same
// arrays. This is the missing reverse direction (ai-provider-expert M-3):
// every entry the Rust validator bans must also be something the PROMPT
// actually told the model to avoid — otherwise the validator flags prose the
// model was never instructed to avoid, a false-positive machine.
//
// Scoped to AI_TELL_LEXICAL_WORDS_*/AI_TELL_PROSE_WORDS_*, which are designed
// as near-verbatim mirrors of the prose (every entry is meant to be spelled
// out literally — see ANTI_AI_TELL_LEXICAL_EN's comma list). TEMPLATE_OPENERS_
// EN/DE are deliberately NOT exhaustively checked here: the prompt only ever
// quotes 2-3 REPRESENTATIVE openers by design (quoting all 10 EN + 6 DE
// clichés would bloat the prompt for no detection benefit — the Rust
// validator is what needs the exhaustive list, the prompt just needs to make
// the pattern clear). The representative subset the prompt DOES quote is
// pinned exactly by section 11 above instead, which is the achievable form of
// this same direction guard for that array.

describe('array -> prompt direction guard (AI_TELL_* — mirrors the existing prompt -> Rust codegen pin)', () => {
  const RESUME_EN = buildResumeSystemPrompt('ats', FULL_TARGET, undefined, 'en');
  const RESUME_DE = buildResumeSystemPrompt('ats', FULL_TARGET, undefined, 'de');
  const RESUME_IT = buildResumeSystemPrompt('ats', FULL_TARGET, undefined, 'it');
  const LETTER_EN = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'en');
  const LETTER_DE = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'de');
  const LETTER_IT = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'it');

  // "it's" / "it is" are both valid AI_TELL_PROSE_WORDS_EN entries (a generated
  // letter may spell the contraction either way), but the prompt prose only
  // spells out one form ("it's not about X, it's about Y") — normalize the
  // contraction so the substring check still finds the expanded entry.
  const normalize = (s: string) => s.toLowerCase().replace(/it's/g, 'it is');
  const bannedBy = (prompt: string, entry: string) => normalize(prompt).includes(normalize(entry));

  it.each(AI_TELL_LEXICAL_WORDS_EN)(
    'AI_TELL_LEXICAL_WORDS_EN entry %j is banned by the resume prompt',
    (entry) => {
      expect(bannedBy(RESUME_EN, entry)).toBe(true);
    }
  );

  it.each(AI_TELL_LEXICAL_WORDS_DE)(
    'AI_TELL_LEXICAL_WORDS_DE entry %j is banned by the resume prompt',
    (entry) => {
      expect(bannedBy(RESUME_DE, entry)).toBe(true);
    }
  );

  it.each(AI_TELL_LEXICAL_WORDS_IT)(
    'AI_TELL_LEXICAL_WORDS_IT entry %j is banned by the resume prompt',
    (entry) => {
      expect(bannedBy(RESUME_IT, entry)).toBe(true);
    }
  );

  it.each(AI_TELL_PROSE_WORDS_EN)(
    'AI_TELL_PROSE_WORDS_EN entry %j is banned by the cover-letter prompt',
    (entry) => {
      expect(bannedBy(LETTER_EN, entry)).toBe(true);
    }
  );

  it.each(AI_TELL_PROSE_WORDS_DE)(
    'AI_TELL_PROSE_WORDS_DE entry %j is banned by the cover-letter prompt',
    (entry) => {
      expect(bannedBy(LETTER_DE, entry)).toBe(true);
    }
  );

  it.each(AI_TELL_PROSE_WORDS_IT)(
    'AI_TELL_PROSE_WORDS_IT entry %j is banned by the cover-letter prompt',
    (entry) => {
      expect(bannedBy(LETTER_IT, entry)).toBe(true);
    }
  );
});

// ─── 13. CONSTRUCTION-DEPENDENT RULES ARE PROMPT-ONLY ─────────────────────────
// Section 12 above proves every lexicon entry is SPELLED OUT in the prompt.
// That is necessary but not sufficient (MEDIUM, PR #963 round 8): the prompt
// can spell a word out while banning it only in a specific CONSTRUCTION, and a
// substring check in the Rust validator has no way to see the construction. It
// flagged "a dashboard highlighting anomalies in real time" and "this was not
// just a side project" — prose the prompt explicitly permits.
//
// So the split is: constructions live in the prompt prose (the model can judge
// them), phrases live in the array (a substring check can judge those). These
// tests pin BOTH halves — the guidance must not quietly disappear with the
// lexicon entries, and the entries must not quietly come back.

describe('construction-dependent prose rules: kept in the prompt, absent from the lexicon', () => {
  const LETTER_EN = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'en');

  it('the prompt still bans negative parallelism, with both worked examples', () => {
    expect(LETTER_EN).toContain('No negative parallelisms');
    expect(LETTER_EN).toContain('not just X, but Y');
    expect(LETTER_EN).toContain("it's not about X, it's about Y");
  });

  it('the prompt still bans superficial "-ing" openers and tails, by example', () => {
    expect(LETTER_EN).toContain('No superficial "-ing" openers or tails');
    for (const word of ['highlighting', 'showcasing', 'underscoring']) {
      expect(LETTER_EN).toContain(word);
    }
  });

  it.each([
    'not just',
    "it's not about",
    'it is not about',
    'highlighting',
    'showcasing',
    'underscoring',
  ])('%j is prompt-only: a bare substring ban would flag permitted prose', (phrase) => {
    expect(AI_TELL_PROSE_WORDS_EN).not.toContain(phrase);
  });

  it('every surviving EN entry is a phrase the prompt bans wherever it appears', () => {
    expect(AI_TELL_PROSE_WORDS_EN).toEqual([
      'it is important to note',
      "it's important to note",
      'it is worth noting',
      "it's worth noting",
      "in today's world",
      'generally speaking',
      'with that in mind',
      'building on this',
    ]);
  });

  // The German twin of the same defect (PR #963 round 9). ANTI_AI_TELL_LEXICAL_DE
  // bans a Nominalstil sentence OPENER and quotes "Die Umsetzung von X erfolgte
  // durch..." as the illustrative example; 'erfolgte durch' in the array flagged
  // the phrase wherever it appeared, including mid-sentence clauses the prompt
  // permits. Both halves are pinned: the guidance stays, the entry goes.
  const LETTER_DE_PROMPT = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'de');

  it('the German prompt still bans the Nominalstil opener, with its worked example', () => {
    expect(LETTER_DE_PROMPT).toContain('formelhafte Nominalstil-Einstiege');
    expect(LETTER_DE_PROMPT).toContain('erfolgte durch');
    expect(LETTER_DE_PROMPT).toContain('verbführenden Satz');
  });

  it("'erfolgte durch' is prompt-only: the ban is on the opener, not the phrase", () => {
    expect(AI_TELL_PROSE_WORDS_DE).not.toContain('erfolgte durch');
  });

  // An empty list is the honest outcome, not an oversight — see the array's
  // own doc. Pinned so a later "the DE list looks empty, let's add something"
  // has to argue with the rule instead of the emptiness.
  it('the DE prose lexicon is empty: German has no unconditionally-banned prose phrase', () => {
    expect(AI_TELL_PROSE_WORDS_DE).toEqual([]);
  });

  it('the DE prose rules that remain are all judgements a substring cannot make', () => {
    for (const rule of [
      'Kein Dreiklang-Zwang',
      'Kein identischer Absatzanfang',
      'Variiere Satzlänge',
    ]) {
      expect(LETTER_DE_PROMPT).toContain(rule);
    }
  });
});

// ─── 14. CATALOG SHAPE ────────────────────────────────────────────────────────
// Every array here is generated verbatim into `lexicon.rs` and compared against
// `flattened_lower` text (lowercased, whitespace-collapsed, punctuation
// UNTOUCHED) with a word boundary at both ends. Each rule below is a way an
// entry can be silently DEAD rather than wrong, which is the failure mode a
// list like this actually has (the German inflection bug, PR #963 R4-F5).

describe('lexicon arrays — shape rules that keep an entry from being silently dead', () => {
  const ARRAYS = {
    AI_TELL_LEXICAL_WORDS_EN,
    AI_TELL_LEXICAL_WORDS_DE,
    AI_TELL_LEXICAL_WORDS_IT,
    AI_TELL_PROSE_WORDS_EN,
    AI_TELL_PROSE_WORDS_DE,
    AI_TELL_PROSE_WORDS_IT,
    TEMPLATE_OPENERS_EN,
    TEMPLATE_OPENERS_DE,
    TEMPLATE_OPENERS_IT,
  } as const;

  for (const [name, entries] of Object.entries(ARRAYS)) {
    describe(name, () => {
      it('has no duplicate entry', () => {
        expect([...new Set(entries)]).toEqual([...entries]);
      });

      it('is lowercase and trimmed (the haystack is lowercased before matching)', () => {
        for (const entry of entries) {
          expect(entry).toBe(entry.toLowerCase());
          expect(entry).toBe(entry.trim());
          expect(entry.length).toBeGreaterThan(0);
        }
      });

      it('contains no em- or en-dash (self-consistency with the dash ban)', () => {
        for (const entry of entries) expect(entry).not.toMatch(/[—–]/);
      });

      // A model writes the typographic apostrophe (U+2019) about as often as
      // the ASCII one. `voice.rs::ai_tell_issues` folds U+2019 onto U+0027
      // before matching, so an ASCII-apostrophe entry now catches BOTH
      // spellings — but a U+2019 entry catches NEITHER (the haystack no longer
      // contains that character at all). The ban is therefore on the curly
      // form only, and it is a hard one: such an entry is silently dead.
      it('carries no typographic apostrophe (U+2019) — the matcher folds it away', () => {
        for (const entry of entries) expect(entry).not.toMatch(/’/);
      });

      // `flattened_lower` collapses every whitespace run to a single space, so
      // a two-space entry can never match anything.
      it('has no double internal space (the haystack collapses whitespace runs)', () => {
        for (const entry of entries) expect(entry).not.toMatch(/ {2}/);
      });

      // Matching requires a non-word character (or string edge) on both sides.
      // An entry that STARTS with punctuation therefore demands a non-word char
      // before that punctuation, which real text almost never provides.
      it('starts with a word character (a leading punctuation char cannot match)', () => {
        for (const entry of entries) expect(entry).toMatch(/^[\p{L}\p{N}]/u);
      });
    });
  }

  it('no phrase is listed in both the lexical and the prose tier of one language', () => {
    for (const [lexical, prose] of [
      [AI_TELL_LEXICAL_WORDS_EN, AI_TELL_PROSE_WORDS_EN],
      [AI_TELL_LEXICAL_WORDS_DE, AI_TELL_PROSE_WORDS_DE],
      [AI_TELL_LEXICAL_WORDS_IT, AI_TELL_PROSE_WORDS_IT],
    ] as const) {
      expect(prose.filter((entry) => (lexical as readonly string[]).includes(entry))).toEqual([]);
    }
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
      itCarriesProseRuleset(() => buildCoverLetterSystemPrompt('recruiter', target));
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
      itCarriesProseRuleset(() => buildReferralPrompt(BASE_PARAMS, target).system);
    });
  }
});

// ─── 3. PROSE SURFACES — application-questions ───────────────────────────────

describe('buildApplicationAnswerSystemPrompt — carries PROSE ruleset, dash-free', () => {
  itCarriesProseRuleset(() => buildApplicationAnswerSystemPrompt());
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
