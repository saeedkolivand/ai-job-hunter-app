import { describe, expect, it } from 'vitest';

import { buildRewritePrompt, type RewriteDocType } from './rewrite.js';

// ─── Fixtures ─────────────────────────────────────────────────────────────────

const BASE_PARAMS = {
  selection: 'Led a team of engineers to deliver the migration.',
  instruction: 'Make it more impactful with a quantified outcome.',
  before: 'WORK EXPERIENCE\nAcme Corp — Senior Engineer (2021–2024)\n',
  after: '\nSkills: TypeScript, React, PostgreSQL',
  docType: 'resume' as RewriteDocType,
};

// ─── System prompt contract ───────────────────────────────────────────────────

describe('buildRewritePrompt — system prompt', () => {
  it('instructs the model to return ONLY the rewritten span (no preamble, no quotes)', () => {
    const { system } = buildRewritePrompt(BASE_PARAMS);
    // Rule 2 in the source: "Output the rewritten span ONLY — no preamble, no
    // explanation, no quotation marks, no surrounding context, no labels."
    expect(system).toMatch(/output the rewritten span only/i);
    expect(system).toMatch(/no preamble/i);
    expect(system).toMatch(/no quotation marks/i);
  });

  it('instructs the model to preserve language, tense, voice, and grammatical person', () => {
    const { system } = buildRewritePrompt(BASE_PARAMS);
    expect(system).toMatch(/tense/i);
    expect(system).toMatch(/voice/i);
    expect(system).toMatch(/grammatical person/i);
    expect(system).toMatch(/same language/i);
  });

  it('forbids fabricating facts', () => {
    const { system } = buildRewritePrompt(BASE_PARAMS);
    expect(system).toMatch(/never fabricate/i);
    // The source specifically mentions keeping concrete claims.
    expect(system).toMatch(/skills|employers|titles|metrics|dates/i);
  });

  it('instructs the model to treat <before> and <after> as read-only context', () => {
    const { system } = buildRewritePrompt(BASE_PARAMS);
    expect(system).toMatch(/read-only context/i);
    expect(system).toMatch(/<before>/);
    expect(system).toMatch(/<after>/);
  });
});

// ─── User prompt content ──────────────────────────────────────────────────────

describe('buildRewritePrompt — user prompt', () => {
  it('contains the <selection> tag wrapping the selected span', () => {
    const { user } = buildRewritePrompt(BASE_PARAMS);
    expect(user).toContain('<selection>');
    expect(user).toContain('</selection>');
    expect(user).toContain(BASE_PARAMS.selection);
  });

  it('contains the before context', () => {
    const { user } = buildRewritePrompt(BASE_PARAMS);
    expect(user).toContain('<before>');
    // The before text is sliced from the tail — check that the text appears.
    expect(user).toContain('Senior Engineer');
  });

  it('contains the after context', () => {
    const { user } = buildRewritePrompt(BASE_PARAMS);
    expect(user).toContain('<after>');
    expect(user).toContain('TypeScript, React, PostgreSQL');
  });

  it('contains the instruction', () => {
    const { user } = buildRewritePrompt(BASE_PARAMS);
    expect(user).toContain(BASE_PARAMS.instruction);
  });
});

// ─── Selection cap ────────────────────────────────────────────────────────────

describe('buildRewritePrompt — selection cap', () => {
  it('truncates a selection longer than 4000 chars in the user prompt', () => {
    const longSelection = 'A'.repeat(5000);
    const { user } = buildRewritePrompt({ ...BASE_PARAMS, selection: longSelection });

    // The capped span (4000 chars of 'A') must appear; the full 5000 must not
    // be present as a contiguous run.
    expect(user).toContain('A'.repeat(4000));
    expect(user).not.toContain('A'.repeat(4001));
  });

  it('passes through a selection exactly at the 4000-char boundary unchanged', () => {
    const exactSelection = 'B'.repeat(4000);
    const { user } = buildRewritePrompt({ ...BASE_PARAMS, selection: exactSelection });
    expect(user).toContain('B'.repeat(4000));
  });
});

// ─── Context budget scales with provider tier ─────────────────────────────────

// Expected budgets — derived once from the source formula (Math.max(400, floor(jobAdChars/2)))
// and pinned as hard constants so the tests break if the code diverges from resolveProfile.
// large (ollama/large): jobAdChars=5000 → budget=2500
// medium (ollama/medium): jobAdChars=2500 → budget=1250
// small (ollama/small): jobAdChars=2500 → budget=1250
const BUDGET_LARGE = 2500;
const BUDGET_MEDIUM = 1250;
const BUDGET_SMALL = 1250;

/** Extract the text captured by a named tag-pair regex; throws if the match is absent. */
function extractTag(user: string, tagRegex: RegExp): string {
  const m = tagRegex.exec(user);
  if (!m) throw new Error(`Tag not found in rendered user prompt. Regex: ${tagRegex}`);
  const captured = m[1];
  if (captured === undefined)
    throw new Error(`Capture group 1 missing in match. Regex: ${tagRegex}`);
  return captured;
}

describe('buildRewritePrompt — context budget vs provider tier', () => {
  it('large target gives more surrounding-context chars than small target', () => {
    const longBefore = 'X'.repeat(8000);

    const { user: userLarge } = buildRewritePrompt({ ...BASE_PARAMS, before: longBefore }, 'large');
    const { user: userSmall } = buildRewritePrompt({ ...BASE_PARAMS, before: longBefore }, 'small');

    // Count how many 'X's made it into each user prompt — large must have more.
    const xCountLarge = (userLarge.match(/X/g) ?? []).length;
    const xCountSmall = (userSmall.match(/X/g) ?? []).length;

    expect(xCountLarge).toBeGreaterThan(xCountSmall);
  });

  it('the rendered <before> context length in the user prompt equals the exact budget for "large"', () => {
    // Pass a before string longer than the largest budget so truncation always triggers.
    const overlong = 'X'.repeat(BUDGET_LARGE + 500);
    const { user } = buildRewritePrompt({ ...BASE_PARAMS, before: overlong }, 'large');

    // extractTag throws if the tag is absent — no non-null assertion needed.
    const renderedBefore = extractTag(user, /<before>\n([\s\S]*?)\n<\/before>/);

    // The rendered before context must be EXACTLY BUDGET_LARGE characters of 'X'.
    // If buildRewritePrompt stopped using resolveProfile (e.g. hardcoded a constant),
    // this assertion breaks unless that constant happens to equal 2500.
    expect(renderedBefore.length).toBe(BUDGET_LARGE);
    expect(renderedBefore).toBe('X'.repeat(BUDGET_LARGE));
  });

  it('the rendered <after> context length in the user prompt equals the exact budget for "large"', () => {
    const overlong = 'Y'.repeat(BUDGET_LARGE + 500);
    const { user } = buildRewritePrompt({ ...BASE_PARAMS, after: overlong }, 'large');

    const renderedAfter = extractTag(user, /<after>\n([\s\S]*?)\n<\/after>/);

    expect(renderedAfter.length).toBe(BUDGET_LARGE);
    expect(renderedAfter).toBe('Y'.repeat(BUDGET_LARGE));
  });

  it('the rendered <before> context for "medium" is exactly BUDGET_MEDIUM chars (smaller than large)', () => {
    const overlong = 'X'.repeat(BUDGET_LARGE + 500);
    const { user } = buildRewritePrompt({ ...BASE_PARAMS, before: overlong }, 'medium');

    const renderedBefore = extractTag(user, /<before>\n([\s\S]*?)\n<\/before>/);

    expect(renderedBefore.length).toBe(BUDGET_MEDIUM);
    // Medium budget must be strictly smaller than large, proving tier differentiation.
    expect(BUDGET_MEDIUM).toBeLessThan(BUDGET_LARGE);
  });

  it('the rendered <before> context for "small" is exactly BUDGET_SMALL chars', () => {
    const overlong = 'X'.repeat(BUDGET_LARGE + 500);
    const { user } = buildRewritePrompt({ ...BASE_PARAMS, before: overlong }, 'small');

    const renderedBefore = extractTag(user, /<before>\n([\s\S]*?)\n<\/before>/);

    expect(renderedBefore.length).toBe(BUDGET_SMALL);
  });

  it('the context budget is derived from resolveProfile — changing tier changes the rendered budget', () => {
    // This is the regression guard: pass identical inputs to all three tiers and
    // assert the rendered before-context lengths differ for large vs. medium/small.
    const overlong = 'X'.repeat(BUDGET_LARGE + 500);
    const extractBeforeLen = (tier: 'large' | 'medium' | 'small') => {
      const { user } = buildRewritePrompt({ ...BASE_PARAMS, before: overlong }, tier);
      return extractTag(user, /<before>\n([\s\S]*?)\n<\/before>/).length;
    };

    expect(extractBeforeLen('large')).toBe(BUDGET_LARGE);
    expect(extractBeforeLen('medium')).toBe(BUDGET_MEDIUM);
    expect(extractBeforeLen('small')).toBe(BUDGET_SMALL);
    // large must be strictly greater than both sub-tiers.
    expect(BUDGET_LARGE).toBeGreaterThan(BUDGET_MEDIUM);
    expect(BUDGET_LARGE).toBeGreaterThan(BUDGET_SMALL);
  });
});

// ─── DOC_LABELS — both document types are handled ────────────────────────────

describe('buildRewritePrompt — docType label in system prompt', () => {
  it('embeds "résumé" in the system prompt for docType "resume"', () => {
    const { system } = buildRewritePrompt({ ...BASE_PARAMS, docType: 'resume' });
    expect(system).toContain('résumé');
  });

  it('embeds "cover letter" in the system prompt for docType "cover-letter"', () => {
    const { system } = buildRewritePrompt({ ...BASE_PARAMS, docType: 'cover-letter' });
    expect(system).toContain('cover letter');
  });

  it('embeds "application answer" in the system prompt for docType "application-answer"', () => {
    const { system } = buildRewritePrompt({ ...BASE_PARAMS, docType: 'application-answer' });
    expect(system).toContain('application answer');
  });

  it('applies the prose voice ruleset (em-dash ban) for docType "application-answer"', () => {
    // application-answer is first-person prose like a cover letter, so it must
    // carry the PROSE FLOW rules (em-dash hard ban), not just the résumé lexical
    // bans. Guards the DOC_VOICE arm against silently regressing to LEXICAL.
    const { system } = buildRewritePrompt({ ...BASE_PARAMS, docType: 'application-answer' });
    expect(system).toMatch(/em-dash hard ban/i);

    // And it must NOT match the résumé arm, which omits PROSE FLOW.
    const { system: resumeSystem } = buildRewritePrompt({ ...BASE_PARAMS, docType: 'resume' });
    expect(resumeSystem).not.toMatch(/em-dash hard ban/i);
  });
});
