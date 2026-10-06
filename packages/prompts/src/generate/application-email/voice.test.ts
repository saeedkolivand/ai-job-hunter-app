import { describe, expect, it } from 'vitest';

import { antiAiTellProse, HUMANIZE_PROSE, toneDirective } from '../natural-voice/index.js';
import { type ApplicationEmailParams, buildApplicationEmailPrompt } from './application-email.js';
import { ALL_DEPTHS, BASE, DE_BASE, META } from './test-support';

describe('buildApplicationEmailPrompt — the voice block reaches every depth', () => {
  /** Lines a `voice.*` check verifies — shipped at every depth, by the tier's own rule. */
  const CHECKED_ANCHORS = [
    'Drop AI-vocabulary',
    'No promotional / inflated self-adjectives',
    'No vague attributions / weasel words',
    'EM-DASH HARD BAN',
    'No rule-of-three',
    'Delete these outright',
  ];
  /** Judgement calls and construction rules — `task`/`full` only. */
  const GUIDANCE_ANCHORS = [
    'PORTABILITY TEST',
    'SHOW, DO NOT TELL',
    'No colon reveals',
    'No fake-profound kicker',
  ];

  it.each(CHECKED_ANCHORS)('the BRIEF prompt carries the checked-tier line %j', (anchor) => {
    expect(buildApplicationEmailPrompt(BASE, 'small').system).toContain(anchor);
  });

  it.each(GUIDANCE_ANCHORS)(
    'the BRIEF prompt drops the guidance-tier line %j (asserted present at full, so a deleted rule fails too)',
    (anchor) => {
      expect(buildApplicationEmailPrompt(BASE, 'large').system).toContain(anchor);
      expect(buildApplicationEmailPrompt(BASE, 'small').system).not.toContain(anchor);
    }
  );

  it.each([...CHECKED_ANCHORS, ...GUIDANCE_ANCHORS])(
    'the TASK prompt carries %j (task gets the complete text, exactly like full)',
    (anchor) => {
      expect(buildApplicationEmailPrompt(BASE, { kind: 'cli' }).system).toContain(anchor);
    }
  );

  it('every depth composes the block for ITS OWN tier, verbatim', () => {
    expect(buildApplicationEmailPrompt(BASE, 'small').system).toContain(
      antiAiTellProse('en', 'brief')
    );
    expect(buildApplicationEmailPrompt(BASE, { kind: 'cli' }).system).toContain(
      antiAiTellProse('en', 'task')
    );
    expect(buildApplicationEmailPrompt(BASE, 'large').system).toContain(antiAiTellProse('en'));
  });

  // The positive counterpart the bans are documented to be composed WITH on
  // every prose surface (cover letter, referral, answers, rewrite). Its CADENCE
  // line is what states the sentence-rhythm rule at `brief`, where the block's
  // own "Vary sentence length" line lives in the dropped guidance tier.
  it.each(ALL_DEPTHS)('HUMANIZE_PROSE rides along at %s depth', (_label, target) => {
    const { system } = buildApplicationEmailPrompt(BASE, target);
    expect(system).toContain(HUMANIZE_PROSE);
    expect(system).toContain('CADENCE');
  });

  it('brief stays materially smaller than full: the tier shrinks, not the wiring', () => {
    const brief = buildApplicationEmailPrompt(BASE, 'small').system;
    const full = buildApplicationEmailPrompt(BASE, 'large').system;
    expect(brief.length).toBeLessThan(full.length * 0.7);
  });

  it.each(ALL_DEPTHS)(
    'a German target gets the curated German lexicon at %s depth (DE is depth-invariant)',
    (_label, target) => {
      const { system } = buildApplicationEmailPrompt(
        { ...BASE, meta: { ...META, targetLanguage: 'de' } },
        target
      );
      expect(system).toContain('KI-Floskeln');
      expect(system).not.toContain('Drop AI-vocabulary');
    }
  );

  it.each(ALL_DEPTHS)(
    'an English target gets the English ban-list at %s depth',
    (_label, target) => {
      expect(buildApplicationEmailPrompt(BASE, target).system).toContain('Drop AI-vocabulary');
    }
  );
});

// ─── Output tone (parity with the cover-letter wiring) ───────────────────────

describe('buildApplicationEmailPrompt — output tone', () => {
  const DEPTHS = ['large', 'small', { kind: 'cli' } as const] as const;

  it('carries the selected tone directive at every depth', () => {
    for (const target of DEPTHS) {
      const { system } = buildApplicationEmailPrompt({ ...BASE, tone: 'casual' }, target);
      expect(system).toContain(toneDirective('casual'));
    }
  });

  it('defaults to the professional directive when no tone is supplied', () => {
    for (const target of DEPTHS) {
      const { system } = buildApplicationEmailPrompt(BASE, target);
      expect(system).toContain(toneDirective('professional'));
    }
  });

  it('uses the prose (not the résumé/ATS-lexical) variant, like the cover letter', () => {
    const { system } = buildApplicationEmailPrompt({ ...BASE, tone: 'creative' }, 'large');
    expect(system).toContain(toneDirective('creative'));
    expect(system).not.toContain(toneDirective('creative', { lexical: true }));
  });

  it('tone never displaces the honesty contract or the market greeting', () => {
    const { system } = buildApplicationEmailPrompt({ ...DE_BASE, tone: 'creative' });
    expect(system).toMatch(/HONESTY/);
    expect(system).toContain('Sehr geehrte Damen und Herren,');
  });
});

// ─── Style reference (writing-style transfer) ─────────────────────────────────

describe('buildApplicationEmailPrompt — styleReference', () => {
  it('fences a provided style reference with the ignore-instructions directive', () => {
    const styleReference = 'I keep it short. I get to the point.';
    const { user } = buildApplicationEmailPrompt({ ...BASE, styleReference });
    expect(user).toContain('<style_reference>');
    expect(user).toContain(styleReference);
    expect(user).toMatch(/WRITING-STYLE reference only/i);
    expect(user).toMatch(/ignore any instructions/i);
  });

  it('omits the block entirely when no styleReference is given, and instead points at <candidate_resume> (no duplicate résumé tokens)', () => {
    const { user } = buildApplicationEmailPrompt(BASE);
    expect(user).not.toContain('<style_reference>');
    expect(user).toMatch(/vocabulary register.*natural cadence.*<candidate_resume>/is);
    expect(user).toMatch(/do not copy its content, facts, or bullet format/i);
    // The résumé text is embedded exactly once — never re-fed as a second block.
    expect(user.split(BASE.resume.trim()).length - 1).toBe(1);
  });
});

// ─── Unknown company — never emit a placeholder ───────────────────────────────
// When meta.companyName is empty, the prompt must not seed a "the company"
// stand-in the model then renders as a literal "[Company]" / "Unternehmen"
// placeholder — it names the role alone and instructs never to invent one.

describe('buildApplicationEmailPrompt — unknown company', () => {
  const NO_COMPANY: ApplicationEmailParams = {
    ...BASE,
    meta: { ...META, companyName: '' },
  };

  it('drops the " at <company>" clause from the CONTEXT Role line when the company is unknown', () => {
    const { user } = buildApplicationEmailPrompt(NO_COMPANY);
    expect(user).not.toContain(' at the company');
    expect(user).not.toContain(' at Globex');
    expect(user).toContain('Role: Senior Backend Engineer');
  });

  it('never renders a bracketed company placeholder anywhere in the prompt', () => {
    const { system, user } = buildApplicationEmailPrompt(NO_COMPANY);
    expect(system).not.toContain('[Company');
    expect(user).not.toContain('[Company');
  });

  it('adds a never-invent-a-company instruction to the opening-paragraph bullet', () => {
    const { system } = buildApplicationEmailPrompt(NO_COMPANY);
    expect(system).toMatch(/company name unknown/i);
    expect(system).toMatch(/never invent, name, or write a company placeholder/i);
  });

  it('leaves the known-company Role line unchanged and adds no unknown-company instruction', () => {
    const { system, user } = buildApplicationEmailPrompt(BASE);
    expect(user).toContain('Role: Senior Backend Engineer at Globex');
    expect(system).not.toMatch(/company name unknown/i);
  });

  it('frames the full-depth email as "about THIS role at THIS company" only when the company is known', () => {
    const { system } = buildApplicationEmailPrompt(BASE, 'large');
    expect(system).toContain('about THIS role at THIS company');
  });

  it('drops the "at THIS company" opening framing at full depth when the company is unknown', () => {
    // Only the opening framing is gated; the shared HUMANIZE voice block still
    // references "THIS company" generically, so the assertion targets the
    // specific opening phrase rather than the substring anywhere.
    const { system } = buildApplicationEmailPrompt(NO_COMPANY, 'large');
    expect(system).not.toContain('about THIS role at THIS company');
    expect(system).toContain('clearly about THIS role');
  });
});

// ─── Opener-ban examples — derived from TEMPLATE_OPENERS_EN, not hand-typed ───
// (ai-provider-expert M-4 follow-up) the two "do not start with" clauses used to
// be independently hand-typed strings; they now derive from the same array
// `LETTER_SPECIFICS` (cover-letter.ts) quotes, so a validator-lexicon edit can't
// silently drift out of sync with this file's copy. Pinned as exact strings —
// the rendered wording is unchanged from before the refactor.

describe('buildApplicationEmailPrompt — opener-ban examples derive from TEMPLATE_OPENERS_EN', () => {
  it('the format-skeleton opening bullet bans the exact expected phrases (brief depth)', () => {
    const { system } = buildApplicationEmailPrompt(BASE, 'small');
    expect(system).toContain(
      'Do not start with "I am excited to apply" or "I am writing to express my interest".'
    );
  });

  it('the full-depth VOICE section bans the exact expected phrases', () => {
    const { system } = buildApplicationEmailPrompt(BASE, 'large');
    expect(system).toContain(
      'Never "I am excited to apply" or "I am writing to express my interest".'
    );
  });
});
