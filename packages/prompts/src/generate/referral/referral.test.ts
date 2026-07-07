import { describe, expect, it } from 'vitest';

import {
  buildReferralImprovePrompt,
  buildReferralPrompt,
  CONNECTION_NOTE_LIMIT,
  type ReferralFormat,
} from './referral.js';

// ─── Fixtures ─────────────────────────────────────────────────────────────────

const RESUME_LARGE =
  'Jane Doe\nSenior Software Engineer\n\n' +
  'PROFESSIONAL SUMMARY\n' +
  'Ten years building distributed systems.\n\n' +
  'EXPERIENCE\n' +
  'Acme Corp — Staff Engineer (2019–2024)\n' +
  '- Led the migration of the billing platform to microservices.\n' +
  '- Reduced p99 latency by 40 % through async processing.\n' +
  'Beta Inc — Senior Engineer (2015–2019)\n' +
  '- Shipped the first real-time analytics dashboard.\n\n' +
  'SKILLS\n' +
  'Rust, TypeScript, PostgreSQL, Kafka\n\n' +
  'EDUCATION\n' +
  'BSc Computer Science — University of Berlin (2015)\n';

// A deliberately tiny résumé so even a small-model budget accommodates it fully,
// making the tier-differentiation test reliable: the large tier passes the entire
// body while the small tier will pass a potentially truncated version — the test
// asserts that large renders MORE résumé context than small, not an exact byte count.
const RESUME_SMALL = 'Alice Li\nFrontend Developer\nSkills: React, TypeScript\n';

const BASE: Parameters<typeof buildReferralPrompt>[0] = {
  personName: 'Bob Chen',
  personRole: 'Engineering Director',
  companyName: 'Globex',
  jobTitle: 'Senior Backend Engineer',
  resume: RESUME_LARGE,
  format: 'email',
};

// ─── connection_note — hard cap constraint ────────────────────────────────────

describe('buildReferralPrompt — connection_note cap', () => {
  it('states the ≤300 hard constraint in the SYSTEM prompt for connection_note', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'connection_note' });
    expect(system).toMatch(/300/);
    expect(system).toMatch(/absolute hard limit|hard limit/i);
  });

  it('re-enforces the hard cap in the USER prompt for connection_note', () => {
    const { user } = buildReferralPrompt({ ...BASE, format: 'connection_note' });
    expect(user).toMatch(/hard constraint/i);
    expect(user).toMatch(/300/);
  });

  it('the cap text is ABSENT from the system prompt for email', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'email' });
    // The system prompt for email must not contain the character-limit rule.
    expect(system).not.toMatch(/absolute hard limit of \d+ characters/i);
  });

  it('the cap text is ABSENT from the user prompt for email', () => {
    const { user } = buildReferralPrompt({ ...BASE, format: 'email' });
    expect(user).not.toMatch(/hard constraint/i);
  });

  it('the cap text is ABSENT from the system prompt for linkedin_message', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'linkedin_message' });
    expect(system).not.toMatch(/absolute hard limit of \d+ characters/i);
  });

  it('the cap text is ABSENT from the user prompt for linkedin_message', () => {
    const { user } = buildReferralPrompt({ ...BASE, format: 'linkedin_message' });
    expect(user).not.toMatch(/hard constraint/i);
  });

  it('uses CONNECTION_NOTE_LIMIT (300) as the default cap for connection_note', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'connection_note' });
    expect(system).toContain(String(CONNECTION_NOTE_LIMIT));
    expect(CONNECTION_NOTE_LIMIT).toBe(300);
  });
});

// ─── All three formats — shared shape ─────────────────────────────────────────

const ALL_FORMATS: ReferralFormat[] = ['email', 'linkedin_message', 'connection_note'];

describe('buildReferralPrompt — shared shape for all formats', () => {
  for (const format of ALL_FORMATS) {
    it(`${format}: system + user are non-empty strings`, () => {
      const { system, user } = buildReferralPrompt({ ...BASE, format });
      expect(typeof system).toBe('string');
      expect(system.length).toBeGreaterThan(0);
      expect(typeof user).toBe('string');
      expect(user.length).toBeGreaterThan(0);
    });

    it(`${format}: user prompt contains <candidate_resume> fenced block`, () => {
      const { user } = buildReferralPrompt({ ...BASE, format });
      expect(user).toContain('<candidate_resume>');
      expect(user).toContain('</candidate_resume>');
    });

    it(`${format}: user prompt contains trimmed recipient name`, () => {
      const { user } = buildReferralPrompt({ ...BASE, format });
      expect(user).toContain('Bob Chen');
    });

    it(`${format}: user prompt contains company name`, () => {
      const { user } = buildReferralPrompt({ ...BASE, format });
      expect(user).toContain('Globex');
    });

    it(`${format}: user prompt contains job title`, () => {
      const { user } = buildReferralPrompt({ ...BASE, format });
      expect(user).toContain('Senior Backend Engineer');
    });
  }
});

// ─── Grounding / honesty instruction ─────────────────────────────────────────

describe('buildReferralPrompt — honesty contract', () => {
  it('system prompt forbids fabricating skills/employers/titles/metrics', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'email' });
    expect(system).toMatch(/never invent/i);
    expect(system).toMatch(/skills|employers|titles|metrics/i);
  });

  it('system prompt prohibits inventing a shared history with the recipient', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'linkedin_message' });
    expect(system).toMatch(/shared history|already know each other/i);
  });

  it('user prompt re-states the grounding rule (traceable to candidate_resume)', () => {
    const { user } = buildReferralPrompt({ ...BASE, format: 'connection_note' });
    expect(user).toMatch(/traceable to/i);
    expect(user).toContain('candidate_resume');
  });
});

// ─── resolveProfile is consumed — tier differentiates rendered résumé context ─

describe('buildReferralPrompt — context budget vs provider tier', () => {
  it('large tier renders MORE résumé context in user prompt than small tier', () => {
    // Use a long résumé so both tiers have something to truncate.
    const longResume = 'Jane Doe\nSenior Engineer\n\nEXPERIENCE\n' + 'X'.repeat(20_000);
    const params = { ...BASE, resume: longResume, format: 'email' as ReferralFormat };

    const { user: userLarge } = buildReferralPrompt(params, 'large');
    const { user: userSmall } = buildReferralPrompt(params, 'small');

    // Count the résumé content that made it through truncation inside the fence.
    const extractResume = (u: string): string => {
      const m = /<candidate_resume>([\s\S]*?)<\/candidate_resume>/.exec(u);
      if (!m?.[1]) throw new Error('candidate_resume block not found in user prompt');
      return m[1];
    };

    const largeResumeLen = extractResume(userLarge).length;
    const smallResumeLen = extractResume(userSmall).length;

    expect(largeResumeLen).toBeGreaterThan(smallResumeLen);
  });

  it('small résumé passes through unchanged for both tiers (no truncation)', () => {
    // RESUME_SMALL is tiny — both tiers must carry it in full.
    const params = { ...BASE, resume: RESUME_SMALL, format: 'email' as ReferralFormat };

    const extractResume = (u: string): string => {
      const m = /<candidate_resume>([\s\S]*?)<\/candidate_resume>/.exec(u);
      if (!m?.[1]) throw new Error('candidate_resume block not found in user prompt');
      return m[1];
    };

    const { user: userLarge } = buildReferralPrompt(params, 'large');
    const { user: userSmall } = buildReferralPrompt(params, 'small');

    // Both render something — size is not the assertion, presence of the résumé is.
    expect(extractResume(userLarge)).toContain('Alice Li');
    expect(extractResume(userSmall)).toContain('Alice Li');
  });
});

// ─── Whitespace trimming ──────────────────────────────────────────────────────

describe('buildReferralPrompt — whitespace trimming', () => {
  it('trims leading/trailing whitespace from companyName in user prompt', () => {
    const { user } = buildReferralPrompt({ ...BASE, companyName: '  Globex  ', format: 'email' });
    // Trimmed value present, padded form absent.
    expect(user).toContain('Globex');
    expect(user).not.toContain('  Globex  ');
  });

  it('trims leading/trailing whitespace from jobTitle in user prompt', () => {
    const { user } = buildReferralPrompt({
      ...BASE,
      jobTitle: '\n  Staff Engineer\n',
      format: 'email',
    });
    expect(user).toContain('Staff Engineer');
    expect(user).not.toContain('\n  Staff Engineer\n');
  });

  it('trims leading/trailing whitespace from personName in user prompt', () => {
    const { user } = buildReferralPrompt({ ...BASE, personName: '  Lena  ', format: 'email' });
    expect(user).toContain('Lena');
    expect(user).not.toContain('  Lena  ');
  });
});

// ─── personRole optional ──────────────────────────────────────────────────────

describe('buildReferralPrompt — personRole', () => {
  it('includes role in recipient line when personRole is provided', () => {
    const { user } = buildReferralPrompt({ ...BASE, format: 'email' });
    // Should render as "Bob Chen (Engineering Director)"
    expect(user).toContain('Bob Chen (Engineering Director)');
  });

  it('omits parenthetical role when personRole is absent', () => {
    const { user } = buildReferralPrompt({ ...BASE, personRole: undefined, format: 'email' });
    expect(user).toContain('Bob Chen');
    expect(user).not.toContain('undefined');
    expect(user).not.toContain('()');
  });

  it('trims personRole before interpolating into recipient line', () => {
    const { user } = buildReferralPrompt({
      ...BASE,
      personRole: '  Staff VP  ',
      format: 'email',
    });
    expect(user).toContain('Bob Chen (Staff VP)');
    expect(user).not.toContain('  Staff VP  ');
  });
});

// ─── custom charLimit override for connection_note ────────────────────────────

describe('buildReferralPrompt — custom charLimit override', () => {
  it('uses the supplied charLimit instead of CONNECTION_NOTE_LIMIT in both prompts', () => {
    const { system, user } = buildReferralPrompt({
      ...BASE,
      format: 'connection_note',
      charLimit: 200,
    });

    // Both the system format-rule and the user HARD CONSTRAINT must carry the
    // overridden limit (200), not the default (300).
    expect(system).toContain('200');
    expect(user).toContain('200');

    expect(system).not.toContain('300');
    expect(user).not.toContain('300');
  });
});

// ─── buildReferralImprovePrompt — revise an existing draft ─────────────────────

const DRAFT =
  'Hi Bob, I noticed Globex is hiring a Senior Backend Engineer. ' +
  'I led a billing-platform migration at Acme and would value a referral. Thanks!';

const IMPROVE_BASE: Parameters<typeof buildReferralImprovePrompt>[0] = {
  ...BASE,
  draft: DRAFT,
  instruction: 'Make it warmer and a bit shorter.',
};

describe('buildReferralImprovePrompt — draft + instruction inclusion', () => {
  it('embeds the current draft inside a fenced <current_draft> block', () => {
    const { user } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(user).toContain('<current_draft>');
    expect(user).toContain('</current_draft>');
    expect(user).toContain('billing-platform migration at Acme');
  });

  it('includes the user instruction in an ### INSTRUCTION ### block', () => {
    const { user } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(user).toMatch(/### INSTRUCTION ###/);
    expect(user).toContain('Make it warmer and a bit shorter.');
  });

  it('truncates an over-long draft to the MAX_DRAFT_CHARS cap', () => {
    const huge = 'A'.repeat(10_000);
    const { user } = buildReferralImprovePrompt({ ...IMPROVE_BASE, draft: huge });
    const m = /<current_draft>\n([\s\S]*?)\n<\/current_draft>/.exec(user);
    if (!m?.[1]) throw new Error('current_draft block not found');
    // Bounded well below the raw 10k input (the cap is 4000).
    expect(m[1].length).toBeLessThanOrEqual(4000);
    expect(m[1].length).toBeGreaterThan(0);
  });

  it('truncates an over-long instruction to the MAX_INSTRUCTION_CHARS cap', () => {
    // A huge instruction (e.g. a pasted blob) must not be interpolated unbounded —
    // it would blow provider context budgets / spike cost.
    const huge = 'B'.repeat(10_000);
    const { user } = buildReferralImprovePrompt({ ...IMPROVE_BASE, instruction: huge });
    const m = /### INSTRUCTION ###\n([\s\S]*?)\n\nRevise <current_draft>/.exec(user);
    if (!m?.[1]) throw new Error('INSTRUCTION block not found');
    // Bounded well below the raw 10k input (the cap is 1000).
    expect(m[1].length).toBeLessThanOrEqual(1000);
    expect(m[1].length).toBeGreaterThan(0);
  });

  it('trims surrounding whitespace from the instruction before interpolating', () => {
    const { user } = buildReferralImprovePrompt({
      ...IMPROVE_BASE,
      instruction: '   Make it shorter.   \n',
    });
    expect(user).toContain('### INSTRUCTION ###\nMake it shorter.\n');
    expect(user).not.toContain('### INSTRUCTION ###\n   Make it shorter.');
  });
});

describe('buildReferralImprovePrompt — preserves generate grounding', () => {
  it('carries the fenced <candidate_resume> block', () => {
    const { user } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(user).toContain('<candidate_resume>');
    expect(user).toContain('</candidate_resume>');
  });

  it('re-states the no-fabrication rule for instruction-driven additions', () => {
    const { user } = buildReferralImprovePrompt(IMPROVE_BASE);
    // The instruction must not become a license to fabricate.
    expect(user).toMatch(/never invent/i);
    expect(user).toMatch(/instruction asks for that the résumé does not support/i);
  });

  it('system prompt keeps the honesty contract (no invented shared history)', () => {
    const { system } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(system).toMatch(/never invent/i);
    expect(system).toMatch(/shared history|already know each other/i);
  });

  it('system prompt tells the model to keep the rules over the instruction', () => {
    const { system } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(system).toMatch(/revise the existing draft/i);
    expect(system).toMatch(/follow the rule/i);
  });

  it('includes recipient, company, and job title in the user prompt', () => {
    const { user } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(user).toContain('Bob Chen (Engineering Director)');
    expect(user).toContain('Globex');
    expect(user).toContain('Senior Backend Engineer');
  });
});

describe('buildReferralImprovePrompt — channel + length cap', () => {
  it('enforces the ≤300 hard cap in both prompts for connection_note', () => {
    const { system, user } = buildReferralImprovePrompt({
      ...IMPROVE_BASE,
      format: 'connection_note',
    });
    expect(system).toMatch(/hard limit/i);
    expect(system).toContain('300');
    expect(user).toMatch(/hard constraint/i);
    expect(user).toContain('300');
  });

  it('honors a custom charLimit override for connection_note', () => {
    const { system, user } = buildReferralImprovePrompt({
      ...IMPROVE_BASE,
      format: 'connection_note',
      charLimit: 180,
    });
    expect(system).toContain('180');
    expect(user).toContain('180');
    expect(system).not.toContain('300');
    expect(user).not.toContain('300');
  });

  it('omits the hard-cap clause for email and linkedin_message', () => {
    for (const format of ['email', 'linkedin_message'] as ReferralFormat[]) {
      const { user } = buildReferralImprovePrompt({ ...IMPROVE_BASE, format });
      expect(user).not.toMatch(/hard constraint/i);
    }
  });

  it('large tier renders MORE résumé context than small tier', () => {
    const longResume = 'Jane Doe\nSenior Engineer\n\nEXPERIENCE\n' + 'X'.repeat(20_000);
    const params = { ...IMPROVE_BASE, resume: longResume };
    const extract = (u: string): string => {
      const m = /<candidate_resume>([\s\S]*?)<\/candidate_resume>/.exec(u);
      if (!m?.[1]) throw new Error('candidate_resume block not found');
      return m[1];
    };
    const { user: large } = buildReferralImprovePrompt(params, 'large');
    const { user: small } = buildReferralImprovePrompt(params, 'small');
    expect(extract(large).length).toBeGreaterThan(extract(small).length);
  });
});

// ─── Humanization (positive HUMANIZE_PROSE block) ─────────────────────────────

describe('buildReferralPrompt / buildReferralImprovePrompt — humanization', () => {
  it('the generate builder carries the positive HUMANIZE_PROSE cadence anchor', () => {
    const { system } = buildReferralPrompt(BASE);
    expect(system).toContain('CADENCE');
  });

  it('the improve builder carries the positive HUMANIZE_PROSE cadence anchor too', () => {
    const { system } = buildReferralImprovePrompt(IMPROVE_BASE);
    expect(system).toContain('CADENCE');
  });

  it('humanization is present even for the char-capped connection_note format', () => {
    const { system } = buildReferralPrompt({ ...BASE, format: 'connection_note' });
    expect(system).toContain('CADENCE');
  });
});
