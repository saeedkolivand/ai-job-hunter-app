import { describe, expect, it } from 'vitest';

import { buildCoverLetterSystemPrompt } from '../cover-letter/index.js';
import { buildResumeSystemPrompt } from '../resume/index.js';
import {
  AI_TELL_LEXICAL_WORDS_DE,
  AI_TELL_LEXICAL_WORDS_EN,
  AI_TELL_LEXICAL_WORDS_IT,
  AI_TELL_PROSE_WORDS_DE,
  AI_TELL_PROSE_WORDS_EN,
  AI_TELL_PROSE_WORDS_IT,
} from './natural-voice.js';
import { FULL_TARGET } from './test-support';

// ─── 15. NO-AI-SLOP TIERING ──────────────────────────────────────────────────
// The `no-ai-slop` pattern catalog was curated into three dispositions, and the
// disposition IS the decision worth pinning: a later "this word is obviously an
// AI tell, add it to the array" has to argue with the reason instead of with an
// absence. See `AI_TELL_LEXICAL_WORDS_EN`'s doc for the four-part test.

describe('no-ai-slop catalog — validated tier (fixed form, zero factual content)', () => {
  const RESUME_EN = buildResumeSystemPrompt('ats', FULL_TARGET, undefined, 'en');
  const LETTER_EN = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'en');

  it.each(['multifaceted', 'ever-evolving', 'paradigm shift', 'meticulous', 'widely regarded as'])(
    '%j is checked by the validator AND spelled out in the résumé prompt',
    (entry) => {
      expect(AI_TELL_LEXICAL_WORDS_EN).toContain(entry);
      expect(RESUME_EN.toLowerCase()).toContain(entry);
    }
  );

  it('"it is worth noting" is a PROSE-tier entry: letters only, never a résumé bullet', () => {
    expect(AI_TELL_PROSE_WORDS_EN).toContain('it is worth noting');
    expect(AI_TELL_LEXICAL_WORDS_EN).not.toContain('it is worth noting');
    expect(LETTER_EN.toLowerCase()).toContain('it is worth noting');
    expect(RESUME_EN.toLowerCase()).not.toContain('it is worth noting');
  });

  // The contraction spellings used to be excluded for a MECHANICAL reason: the
  // matcher normalized case and whitespace but not punctuation, so an ASCII
  // apostrophe missed every U+2019 document. `voice.rs::ai_tell_issues` now
  // folds U+2019 onto U+0027 before matching, which makes an apostrophe entry
  // whole instead of half-dead — so the twins are checked rather than skipped.
  it.each(["it's worth noting", "it's important to note"])(
    'contraction twin %j is checked now that the matcher folds apostrophes',
    (entry) => {
      expect(AI_TELL_PROSE_WORDS_EN).toContain(entry);
      // Its expanded twin is checked too: a model writes both.
      expect(AI_TELL_PROSE_WORDS_EN).toContain(entry.replace("it's", 'it is'));
    }
  );

  // Same promotion, same reason: the phrase is a fixed form with zero factual
  // content and the prompt bans it outright, and it is now MATCHABLE. It is
  // prose tier, not lexical, because its ban lives in the letter-register
  // filler line (an ATS bullet cannot contain it).
  it('"in today\'s world" is a checked PROSE-tier entry, banned in the letter prompt only', () => {
    expect(AI_TELL_PROSE_WORDS_EN).toContain("in today's world");
    expect(AI_TELL_LEXICAL_WORDS_EN).not.toContain("in today's world");
    expect(LETTER_EN.toLowerCase()).toContain("in today's world");
    expect(RESUME_EN.toLowerCase()).not.toContain("in today's world");
  });

  // The DE twin was already validated (German spells it without an
  // apostrophe), so this closes the asymmetry the first pass recorded.
  it('the German twin of "in today\'s world" stays validated and untranslated', () => {
    expect(AI_TELL_LEXICAL_WORDS_DE).toContain('in der heutigen zeit');
    expect(AI_TELL_LEXICAL_WORDS_DE).toContain('in der heutigen welt');
    expect(AI_TELL_PROSE_WORDS_DE).not.toContain("in today's world");
  });

  it('"meticulous" joins the promotional family "detail-oriented" already belongs to', () => {
    // Banning one synonym and not the other is an incoherent catalog, which is
    // the whole argument for this entry.
    expect(AI_TELL_LEXICAL_WORDS_EN).toContain('detail-oriented');
    expect(AI_TELL_LEXICAL_WORDS_EN).toContain('meticulous');
  });
});

describe('no-ai-slop catalog — prompt-guidance tier (instructed, never validated)', () => {
  const RESUME_EN = buildResumeSystemPrompt('ats', FULL_TARGET, undefined, 'en');
  const LETTER_EN = buildCoverLetterSystemPrompt('recruiter', FULL_TARGET, undefined, 'en');

  const isValidated = (word: string) =>
    AI_TELL_LEXICAL_WORDS_EN.includes(word) || AI_TELL_PROSE_WORDS_EN.includes(word);

  // Words that name something a real candidate really DID. Flagging one tells a
  // truthful user their own work reads as machine-written, so the model is told
  // to prefer the plain verb and the checker never sees them.
  it.each(['utilize', 'facilitate', 'supercharge', 'embark'])(
    'résumé-plausible verb %j is prompt-only',
    (word) => {
      expect(isValidated(word)).toBe(false);
      expect(RESUME_EN.toLowerCase()).toContain(word);
    }
  );

  // "beacon" is the domain-collision case: a BLE/iBeacon fleet is real
  // infrastructure a real engineer really shipped.
  it('"beacon" is prompt-only because it has a real technical meaning', () => {
    expect(isValidated('beacon')).toBe(false);
    expect(RESUME_EN.toLowerCase()).toContain('beacon');
  });

  // Same rule-4 failure, found on the second pass: "transformative learning"
  // (Mezirow, the standard L&D curriculum term) and "transformative justice"
  // (social work) are NAMED FIELDS, not decoration. A candidate who ran either
  // programme cannot write their own job title without tripping the checker.
  it('"transformative" is prompt-only: it names real fields in L&D and social work', () => {
    expect(isValidated('transformative')).toBe(false);
    expect(RESUME_EN.toLowerCase()).toContain('transformative');
  });

  // Rule 4 again, third pass, and the variant that motivated naming PROPER
  // NOUNS in the rule: Paramount Global / Pictures / Network are real
  // employers, and `ai_tell_issues`' per-phrase exemption reads only the source
  // RÉSUMÉ — never the job ad — so a letter addressed to Paramount was told the
  // employer's own name is an AI tell. Same exit "transformative" took.
  it('"paramount" is prompt-only: it is a real employer name the exemption cannot see', () => {
    expect(isValidated('paramount')).toBe(false);
    expect(RESUME_EN.toLowerCase()).toContain('paramount');
  });

  // Fillers a truthful human writes constantly. Zero factual content, but a
  // Warning reading "this is an AI tell" on one of them is the trust cost.
  // These two are lexical-tier register, so they reach the résumé prompt too.
  it.each(['game changer', 'many argue'])('conversational filler %j is prompt-only', (phrase) => {
    expect(isValidated(phrase)).toBe(false);
    expect(RESUME_EN.toLowerCase()).toContain(phrase);
  });

  // Letter register, and PROSE-tier since the guidance-tier split: none of
  // these can occur in an ATS bullet, so banning them in the résumé prompt was
  // ~120 characters of dead instruction per generation.
  it.each([
    'at the end of the day',
    'when it comes to',
    'at its core',
    'in terms of',
    'with regard to',
    'going forward',
    'in conclusion',
    'as you can see',
    'the key point is',
    'in other words',
  ])('letter-register filler %j is prompt-only and letter-only', (phrase) => {
    expect(isValidated(phrase)).toBe(false);
    expect(LETTER_EN.toLowerCase()).toContain(phrase);
    expect(RESUME_EN.toLowerCase()).not.toContain(phrase);
  });

  // Redundant rather than rejected: the single word already fires, so adding
  // the phrase would report ONE span twice.
  it.each([
    ['stands as a testament', 'testament'],
    ['marks a pivotal moment', 'pivotal'],
    ['plays a vital role', 'vital'],
  ])('puffery phrase %j is prompt-only because %j already fires on it', (phrase, word) => {
    expect(isValidated(phrase)).toBe(false);
    expect(AI_TELL_LEXICAL_WORDS_EN).toContain(word);
    expect(RESUME_EN.toLowerCase()).toContain(phrase);
  });

  it.each([
    ['binary contrast', "the question isn't X, it's Y"],
    ['negative listing', 'Not a X. Not a Y. A Z.'],
    ['throat-clearing opener', "Here's the thing"],
    ['faux-insight setup', 'What most people get wrong'],
    ['rhetorical setup', 'What if I told you'],
    ['self-answered question', 'a question you immediately answer yourself'],
    ['colon reveal', 'The best part: it learns'],
    ['dramatic fragmentation', "That's it. That's the whole thing."],
    ['synonym cycling', 'synonym cycling'],
    ['fake-profound kicker', 'fake-profound kicker'],
    ['summary-recap ending', 'In conclusion'],
    ['formatting slop', 'Formatting follows the content'],
  ])('the %s construction reaches the cover-letter prompt as prose (%j)', (_name, anchor) => {
    expect(LETTER_EN).toContain(anchor);
  });

  it.each([
    ['portability test', 'PORTABILITY TEST'],
    ['show-do-not-tell', 'SHOW, DO NOT TELL'],
    ['plain verbs', 'Plain verbs beat bloated ones'],
    ['empty adverbs', 'Cut empty adverbs'],
    ['importance puffery', 'No importance puffery'],
  ])('the %s rule reaches the résumé prompt too (%j)', (_name, anchor) => {
    expect(RESUME_EN).toContain(anchor);
  });

  // Constraint from the module doc: the English catalog grew, German did not.
  // A translated English tell in the German list bans phrasing no German writer
  // produces and misses the real KI-Floskeln.
  it('the German arrays gained no translated English tell', () => {
    for (const english of [
      'paramount',
      'multifaceted',
      'ever-evolving',
      'paradigm shift',
      'meticulous',
      'widely regarded as',
      'it is worth noting',
    ]) {
      expect(AI_TELL_LEXICAL_WORDS_DE).not.toContain(english);
      expect(AI_TELL_PROSE_WORDS_DE).not.toContain(english);
    }
    // "robust" is in BOTH curated lists, and legitimately so: it is a tell a
    // German-language model really produces, arrived at on German evidence
    // rather than carried across. That is the distinction this test draws.
    expect(AI_TELL_LEXICAL_WORDS_DE).toContain('robust');
    // The DE prose tier is still empty for its own documented reason.
    expect(AI_TELL_PROSE_WORDS_DE).toEqual([]);
  });

  // Same constraint, the Italian side: a bulk translation of the English (or
  // German) catalog would ban phrasing no Italian writer would produce.
  it('the Italian arrays are not a bulk translation of the English or German catalogs', () => {
    for (const english of [
      'cutting-edge',
      'proven track record',
      'team player',
      'results-driven',
      'meticulous',
      'detail-oriented',
      'it is worth noting',
      "in today's world",
    ]) {
      expect(AI_TELL_LEXICAL_WORDS_IT).not.toContain(english);
      expect(AI_TELL_PROSE_WORDS_IT).not.toContain(english);
    }
    for (const german of ['robust', 'nahtlos', 'teamplayer', 'darüber hinaus', 'weltklasse']) {
      expect(AI_TELL_LEXICAL_WORDS_IT).not.toContain(german);
    }
  });
});
