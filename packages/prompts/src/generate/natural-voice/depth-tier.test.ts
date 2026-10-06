import { describe, expect, it } from 'vitest';

import { buildCoverLetterSystemPrompt } from '../cover-letter/index.js';
import { buildResumeSystemPrompt } from '../resume/index.js';
import {
  AI_TELL_LEXICAL_WORDS_EN,
  AI_TELL_PROSE_WORDS_EN,
  antiAiTellLexical,
  antiAiTellProse,
} from './natural-voice.js';
import { BRIEF_TARGET, FULL_TARGET, TASK_TARGET } from './test-support';

// ─── 16. DEPTH-AWARE GUIDANCE TIER ───────────────────────────────────────────
// The whole anti-AI-tell ruleset used to be appended undifferentiated at every
// depth. Measured against `main`, the block was 25.0% of the BRIEF cover-letter
// prompt and 22.3% of the BRIEF résumé prompt; the expanded catalog would have
// taken those to 47.8% and ~42% had it shipped undifferentiated. Either way it
// is style rules on the one path whose model has the least room to apply them.
// The split is by VERIFIABILITY, which is also the honesty rule: `brief` keeps
// every line a deterministic check will verify (so the validator is never
// stricter than the instruction it exists to verify, at any depth) and drops
// the judgement calls a small model cannot act on anyway.

describe('depth-aware anti-AI-tell tier (brief vs full/task)', () => {
  /** Lines that back a `voice.*` check — must survive at EVERY depth. */
  const CHECKED_LEXICAL = [
    'Drop AI-vocabulary',
    'No promotional / inflated self-adjectives',
    'No vague attributions / weasel words',
    'Cut filler phrases',
  ];
  /** Judgement calls — `full`/`task` only. */
  const GUIDANCE_LEXICAL = [
    'Drop these too, unless the word is genuinely the subject',
    'More weasel attribution',
    'No importance puffery',
    'Plain verbs beat bloated ones',
    'Cut empty adverbs',
    'PORTABILITY TEST',
    'SHOW, DO NOT TELL',
  ];
  const CHECKED_PROSE = [
    'EM-DASH HARD BAN', // voice.em_dash_overuse
    'No rule-of-three', // voice.rule_of_three_density
    // The prose array, reported under voice.ai_tell_lexical like the lexical
    // one ("in today's world", "it is worth noting"). There is no prose CODE.
    'Delete these outright',
  ];
  /** Constructions a substring check cannot judge — `full`/`task` only. */
  const GUIDANCE_PROSE = [
    'Cut the stock connectives',
    'Never tell the reader what to notice',
    'Vary sentence length and rhythm',
    'No negative parallelisms',
    'No superficial "-ing" openers or tails',
    'No throat-clearing, faux-insight, or rhetorical setups',
    'No colon reveals',
    'No stacked punchy fragments',
    'No fake-profound kicker',
    'Formatting follows the content',
    'No passive voice where active is natural',
    'Concrete over abstract',
  ];

  describe('full / task / default are the same complete text', () => {
    it.each(['full', 'task'] as const)('antiAiTellLexical("en", %j) is the full block', (depth) => {
      expect(antiAiTellLexical('en', depth)).toBe(antiAiTellLexical('en'));
    });

    it.each(['full', 'task'] as const)('antiAiTellProse("en", %j) is the full block', (depth) => {
      expect(antiAiTellProse('en', depth)).toBe(antiAiTellProse('en'));
    });
  });

  describe('brief keeps the checked bans and drops the judgement rules', () => {
    const briefLexical = antiAiTellLexical('en', 'brief');
    const fullLexical = antiAiTellLexical('en');
    const briefProse = antiAiTellProse('en', 'brief');
    const fullProse = antiAiTellProse('en');

    it.each(CHECKED_LEXICAL)('brief lexical keeps %j (a validated ban)', (anchor) => {
      expect(briefLexical).toContain(anchor);
    });

    it.each(GUIDANCE_LEXICAL)('brief lexical drops %j (a judgement call)', (anchor) => {
      expect(fullLexical).toContain(anchor);
      expect(briefLexical).not.toContain(anchor);
    });

    it.each(CHECKED_PROSE)('brief prose keeps %j (it backs a voice.* check)', (anchor) => {
      expect(briefProse).toContain(anchor);
    });

    it.each(GUIDANCE_PROSE)('brief prose drops %j (a construction rule)', (anchor) => {
      expect(fullProse).toContain(anchor);
      expect(briefProse).not.toContain(anchor);
    });

    it('brief is a strict SUBSET of full: no wording invented for the small path', () => {
      for (const line of briefLexical.split('\n')) expect(fullLexical).toContain(line);
      for (const line of briefProse.split('\n')) expect(fullProse).toContain(line);
    });

    it('brief composes prose on top of the SAME brief lexical core', () => {
      expect(briefProse).toContain(briefLexical);
      expect(briefProse.length).toBeGreaterThan(briefLexical.length);
    });

    it('brief is materially smaller than full (the point of the split)', () => {
      expect(briefLexical.length).toBeLessThan(fullLexical.length * 0.6);
      expect(briefProse.length).toBeLessThan(fullProse.length * 0.5);
    });

    it('brief stays dash-free like every other block', () => {
      expect(briefLexical).not.toMatch(/[—–]/);
      expect(briefProse).not.toMatch(/[—–]/);
    });
  });

  describe('per-surface budget: the guidance tier stops dominating the small path', () => {
    // Bounds, not exact sizes, so ordinary rewording does not churn the test —
    // but ratcheted to just above the measured value so a regression that
    // re-adds a tier's worth of text fails. Measured: 20.9% (letter, 25.0% on
    // `main`) and 22.8% (résumé, 22.3% on `main`).
    it('the anti-tell block is a quarter of the BRIEF cover-letter prompt at most', () => {
      const prompt = buildCoverLetterSystemPrompt('recruiter', BRIEF_TARGET, undefined, 'en');
      const block = antiAiTellProse('en', 'brief');
      expect(prompt).toContain(block);
      expect(block.length / prompt.length).toBeLessThan(0.25);
    });

    it('the anti-tell block is a quarter of the BRIEF résumé prompt at most', () => {
      const prompt = buildResumeSystemPrompt('ats', BRIEF_TARGET, undefined, 'en');
      const block = antiAiTellLexical('en', 'brief');
      expect(prompt).toContain(block);
      expect(block.length / prompt.length).toBeLessThan(0.25);
    });

    it('the FULL prompts still carry the complete block (depth is wired, not hardcoded)', () => {
      expect(buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'en')).toContain(
        antiAiTellProse('en')
      );
      expect(buildResumeSystemPrompt('ats', FULL_TARGET, undefined, 'en')).toContain(
        antiAiTellLexical('en')
      );
      expect(buildCoverLetterSystemPrompt('recruiter', TASK_TARGET, undefined, 'en')).toContain(
        antiAiTellProse('en')
      );
      expect(buildResumeSystemPrompt('ats', TASK_TARGET, undefined, 'en')).toContain(
        antiAiTellLexical('en')
      );
    });
  });

  // The reason the split is by verifiability and not by "what looks least
  // important": the Rust validator runs on the OUTPUT, and knows nothing about
  // which depth produced it. A ban that survives in the lexicon but not in the
  // brief prompt is a Warning the model was never told about on that path.
  describe('every validated entry is still spelled out at BRIEF depth', () => {
    const RESUME_BRIEF = buildResumeSystemPrompt('ats', BRIEF_TARGET, undefined, 'en');
    const LETTER_BRIEF = buildCoverLetterSystemPrompt('recruiter', BRIEF_TARGET, undefined, 'en');
    /**
     * The same two folds the checker applies, so a mismatch here can only ever
     * mean "the prompt does not ban this entry" and never "the two spell the
     * apostrophe differently":
     *
     * 1. U+2019 -> U+0027, exactly what `voice.rs::ai_tell_issues` (and
     *    `template_opener_issues`) do before matching. Without it an entry like
     *    "in today's world" fails the moment a prompt line is written with a
     *    typographic apostrophe — a spelling failure wearing a missing-ban
     *    failure's clothes.
     * 2. The its/it-is class: the arrays deliberately carry BOTH spellings
     *    ("it's worth noting" is its own entry because a full-phrase match
     *    cannot cross the contraction), while the prompt spells the pair out
     *    once, expanded.
     */
    const normalize = (s: string) => s.toLowerCase().replace(/’/g, "'").replace(/it's/g, 'it is');
    const bannedBy = (prompt: string, entry: string) =>
      normalize(prompt).includes(normalize(entry));

    // Mutation-visible on its own: drop either fold and one direction breaks.
    // The curly renderings stand in for a future prompt (or lexicon) line typed
    // with the apostrophe a word processor inserts.
    const curly = (s: string) => s.replace(/'/g, '’');

    it.each(AI_TELL_PROSE_WORDS_EN.filter((entry) => entry.includes("'")))(
      'apostrophe-bearing entry %j matches whichever apostrophe either side spells',
      (entry) => {
        expect(bannedBy(LETTER_BRIEF, entry)).toBe(true);
        expect(bannedBy(curly(LETTER_BRIEF), entry)).toBe(true);
        expect(bannedBy(LETTER_BRIEF, curly(entry))).toBe(true);
        expect(bannedBy(curly(LETTER_BRIEF), curly(entry))).toBe(true);
      }
    );

    it.each(AI_TELL_LEXICAL_WORDS_EN)(
      'lexical entry %j is banned by the BRIEF résumé prompt',
      (entry) => {
        expect(bannedBy(RESUME_BRIEF, entry)).toBe(true);
      }
    );

    it.each(AI_TELL_LEXICAL_WORDS_EN)(
      'lexical entry %j is banned by the BRIEF cover-letter prompt',
      (entry) => {
        expect(bannedBy(LETTER_BRIEF, entry)).toBe(true);
      }
    );

    it.each(AI_TELL_PROSE_WORDS_EN)(
      'prose entry %j is banned by the BRIEF cover-letter prompt',
      (entry) => {
        expect(bannedBy(LETTER_BRIEF, entry)).toBe(true);
      }
    );
  });

  // The REVERSE direction of the honesty invariant above, and the half that was
  // missing: "every validated entry is spelled out at brief" says nothing about
  // phrases the CHECKED tier ships that NO check will ever verify. Ten of the
  // twelve phrases the two CHECKED prose lines quoted were never validated (and
  // this file's own section-15 cases classify nine of them as prompt-guidance),
  // so the small path was paying for judgement calls a 3B model cannot act on
  // while the tier's stated rule said otherwise.
  //
  // These read the SHIPPED brief block rather than the private constants, so
  // they measure what a small model actually receives.
  describe('nothing in the BRIEF tier is a phrase no check will ever verify', () => {
    const briefLexical = antiAiTellLexical('en', 'brief');
    const briefProse = antiAiTellProse('en', 'brief');
    const VALIDATED = new Set<string>([...AI_TELL_LEXICAL_WORDS_EN, ...AI_TELL_PROSE_WORDS_EN]);

    /**
     * Every double-quoted phrase in a block, minus the REPLACEMENTS a
     * `"in order to" -> "to"` pair also quotes (those are the plain word to
     * reach for, not a ban). Split on the quote character rather than matched
     * with a regex: odd-indexed segments are the quoted ones, and the segment
     * before each says whether an arrow introduced it.
     *
     * That "odd-indexed" rule is only true while every line closes the quotes
     * it opens, so the parity of the split is checked before it is trusted. One
     * missing quote silently swaps the two halves of the line from there on:
     * the prose between two bans becomes a "ban" (and fails against the lexicon
     * for the wrong reason) while the real bans become prose and go unchecked.
     * A parser that guesses is worse than one that stops.
     */
    const quotedBans = (block: string): string[] => {
      const bans: string[] = [];
      for (const line of block.split('\n')) {
        const parts = line.split('"');
        if (parts.length % 2 === 0) {
          throw new Error(
            `unbalanced double quotes (${parts.length - 1}, expected an even count) in the ` +
              `BRIEF block line ${JSON.stringify(line)} — every quoted ban after it would be ` +
              `parsed as prose and silently stop being checked. Fix the quoting, not this test.`
          );
        }
        for (let i = 1; i < parts.length; i += 2) {
          if ((parts[i - 1] ?? '').trimEnd().endsWith('->')) continue;
          bans.push((parts[i] ?? '').toLowerCase());
        }
      }
      return bans;
    };

    it('the quoted-ban parser refuses an unbalanced line instead of mis-parsing it', () => {
      const unbalanced = '- Delete these outright: "in today\'s world, "it is worth noting".';
      expect(() => quotedBans(unbalanced)).toThrow(/unbalanced double quotes \(3,/);
      // The balanced form of the same line parses, so the guard rejects the
      // defect and not the shape.
      expect(
        quotedBans('- Delete these outright: "in today\'s world", "it is worth noting".')
      ).toEqual(["in today's world", 'it is worth noting']);
    });

    /** The comma-separated word list a `- <label>: a, b, c.` line bans. */
    const listedBans = (block: string, label: string): string[] => {
      const line = block.split('\n').find((l) => l.startsWith(label));
      if (!line) throw new Error(`no BRIEF line starts with ${JSON.stringify(label)}`);
      return (line.slice(label.length).split('. ')[0] ?? '')
        .replace(/\.$/, '')
        .split(',')
        .map((w) => w.trim().toLowerCase())
        .filter(Boolean);
    };

    it('every phrase the BRIEF block QUOTES as a ban is a validated lexicon entry', () => {
      // briefProse composes briefLexical, so dedupe before reporting.
      const quoted = [...new Set([...quotedBans(briefLexical), ...quotedBans(briefProse)])];
      expect(quoted.length).toBeGreaterThan(4); // the parser found something
      expect(quoted.filter((phrase) => !VALIDATED.has(phrase))).toEqual([]);
    });

    it.each(['- Drop AI-vocabulary: ', '- No promotional / inflated self-adjectives: '])(
      'every word the BRIEF line %j LISTS is a validated lexicon entry',
      (label) => {
        const listed = listedBans(briefLexical, label);
        expect(listed.length).toBeGreaterThan(3);
        expect(listed.filter((word) => !VALIDATED.has(word))).toEqual([]);
      }
    );

    // The behavioural half: the exact phrases section 15 pins as prompt-only
    // must not reach the small path at all. Belt and braces with the two
    // mechanical rules above — a future CHECKED line in a shape neither parser
    // recognises still fails here.
    const GUIDANCE_CLASSIFIED = [
      'utilize',
      'facilitate',
      'supercharge',
      'embark',
      'beacon',
      'transformative',
      'paramount',
      'game changer',
      'many argue',
      'at the end of the day',
      'when it comes to',
      'at its core',
      'in terms of',
      'with regard to',
      'going forward',
      'in conclusion',
      'as you can see',
      'the key point is',
      'this distinction matters',
      'in other words',
      'stands as a testament',
      'marks a pivotal moment',
      'plays a vital role',
    ];

    it.each(GUIDANCE_CLASSIFIED)(
      'guidance-classified phrase %j never reaches the BRIEF block',
      (phrase) => {
        expect(briefLexical.toLowerCase()).not.toContain(phrase);
        expect(briefProse.toLowerCase()).not.toContain(phrase);
      }
    );

    it.each(GUIDANCE_CLASSIFIED)(
      'guidance-classified phrase %j is still instructed at FULL depth',
      (phrase) => {
        const full = `${antiAiTellProse('en')}\n${antiAiTellLexical('en')}`.toLowerCase();
        expect(full).toContain(phrase);
      }
    );
  });

  // German is curated on German evidence, and which German lines a small model
  // can apply is a question only German evidence answers (see the module doc's
  // follow-up). Until that evidence exists, DE and the generic directive are
  // depth-invariant rather than guessed at.
  describe('non-English rulesets are depth-invariant', () => {
    it.each(['brief', 'task', 'full'] as const)('de is unchanged at %j depth', (depth) => {
      expect(antiAiTellLexical('de', depth)).toBe(antiAiTellLexical('de'));
      expect(antiAiTellProse('de', depth)).toBe(antiAiTellProse('de'));
    });

    it.each(['brief', 'task', 'full'] as const)('it is unchanged at %j depth', (depth) => {
      expect(antiAiTellLexical('it', depth)).toBe(antiAiTellLexical('it'));
      expect(antiAiTellProse('it', depth)).toBe(antiAiTellProse('it'));
    });

    it('a generic locale is unchanged at brief depth', () => {
      expect(antiAiTellLexical('fr', 'brief')).toBe(antiAiTellLexical('fr'));
      expect(antiAiTellProse('fr', 'brief')).toBe(antiAiTellProse('fr'));
    });
  });
});
