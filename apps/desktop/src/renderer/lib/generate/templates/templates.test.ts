import { describe, expect, it } from 'vitest';

import {
  atsModeHintKey,
  isDecoratedLetterLayout,
  isDesignTier,
  isPhotoTemplate,
  isTwoColumnTemplate,
  LETTER_LAYOUT_IDS,
  shouldClearAtsMode,
  TEMPLATES,
} from './templates';

describe('TEMPLATES', () => {
  const ids = Object.keys(TEMPLATES);

  it('exposes the sixteen document templates keyed by id', () => {
    expect(ids).toHaveLength(16);
    for (const id of ids) {
      expect(TEMPLATES[id as keyof typeof TEMPLATES].id).toBe(id);
    }
  });

  // Sync guard: this id set MUST equal the Rust `TemplateId` enum (export/types.rs,
  // kebab-case) and the shared contract union (packages/shared/.../documents.ts).
  // The Rust round-trip test pins the other side; if either drifts, a guard fails.
  it('matches the canonical 16-template id set', () => {
    expect([...ids].sort()).toEqual([
      'academic',
      'aria',
      'atelier',
      'awesome',
      'cadence',
      'classic',
      'cologne-navy',
      'deedy',
      'jake',
      'lebenslauf',
      'meridian',
      'portrait',
      'regent',
      'saffron',
      'swiss-minimal',
      'throughline',
    ]);
  });

  it('uses 6-digit hex colours without a leading hash', () => {
    for (const t of Object.values(TEMPLATES)) {
      for (const color of [t.nameColor, t.sectionColor, t.bodyColor, t.ruleColor]) {
        expect(color).toMatch(/^[0-9A-Fa-f]{6}$/);
      }
    }
  });

  it('declares positive font sizes and a known section style', () => {
    for (const t of Object.values(TEMPLATES)) {
      expect(t.namePt).toBeGreaterThan(0);
      expect(t.bodyPt).toBeGreaterThan(0);
      expect(['ruled-bottom', 'underline', 'bold-only']).toContain(t.sectionStyle);
    }
  });

  // ── tier metadata (mirrors the Rust `TemplateTier`) ─────────────────────────

  it('assigns every template an ats or design tier', () => {
    for (const t of Object.values(TEMPLATES)) {
      expect(['ats', 'design']).toContain(t.tier);
    }
  });

  it('mirrors the Rust TemplateTier split (ats: single-column · design: photo/two-column)', () => {
    const idsByTier = (tier: 'ats' | 'design') =>
      Object.values(TEMPLATES)
        .filter((t) => t.tier === tier)
        .map((t) => t.id)
        .sort();
    expect(idsByTier('ats')).toEqual([
      'academic',
      'cadence',
      'classic',
      'cologne-navy',
      'jake',
      'meridian',
      'regent',
      'swiss-minimal',
      'throughline',
    ]);
    expect(idsByTier('design')).toEqual([
      'aria',
      'atelier',
      'awesome',
      'deedy',
      'lebenslauf',
      'portrait',
      'saffron',
    ]);
  });

  it('isDesignTier is true exactly for design-tier templates', () => {
    for (const t of Object.values(TEMPLATES)) {
      expect(isDesignTier(t.id)).toBe(t.tier === 'design');
    }
    // Lebenslauf is design tier despite being single-column — the toggle-gate fix.
    expect(isDesignTier('lebenslauf')).toBe(true);
    expect(isDesignTier('classic')).toBe(false);
  });
});

describe('isPhotoTemplate', () => {
  it('is true exactly for the photo-bearing templates', () => {
    expect(
      Object.values(TEMPLATES)
        .filter((t) => isPhotoTemplate(t.id))
        .map((t) => t.id)
        .sort()
    ).toEqual(['aria', 'lebenslauf', 'portrait', 'saffron']);
  });

  it('is false for Awesome and Deedy — design-tier but no photo', () => {
    expect(isPhotoTemplate('awesome')).toBe(false);
    expect(isPhotoTemplate('deedy')).toBe(false);
  });
});

describe('atsModeHintKey', () => {
  it('picks the two-column key whenever the template is two-column, even if it also has a photo', () => {
    // Portrait/Aria/Saffron are two-column AND photo-bearing — two-column wins
    // (its copy is the inclusive one, covering both facts).
    for (const id of ['atelier', 'portrait', 'aria', 'saffron'] as const) {
      expect(atsModeHintKey(id)).toBe('aiGenerate.atsModeHintTwoColumn');
    }
  });

  it('picks the photo key for a photo-only (single-column) template', () => {
    expect(atsModeHintKey('lebenslauf')).toBe('aiGenerate.atsModeHintPhoto');
  });

  // F1: Awesome/Deedy are design-tier but neither two-column nor photo-bearing —
  // the old binary routing sent them to the (factually false) photo hint.
  it('picks the decorative key for a design template with neither a photo nor two columns', () => {
    expect(atsModeHintKey('awesome')).toBe('aiGenerate.atsModeHintDecorative');
    expect(atsModeHintKey('deedy')).toBe('aiGenerate.atsModeHintDecorative');
  });

  // The two tests below replace a prior one that re-derived atsModeHintKey's
  // own if/isTwoColumnTemplate/isPhotoTemplate/else branching and so could
  // never fail on its own — it was the implementation, restated. These instead
  // pin structural invariants the literal per-id pins above don't cover.

  it('resolves every design-tier template to one of the three valid hint keys (totality)', () => {
    const VALID_KEYS = new Set<string>([
      'aiGenerate.atsModeHintTwoColumn',
      'aiGenerate.atsModeHintPhoto',
      'aiGenerate.atsModeHintDecorative',
    ]);
    const designIds = Object.values(TEMPLATES)
      .filter((t) => t.tier === 'design')
      .map((t) => t.id);
    expect(designIds.length).toBeGreaterThan(0); // guard against a vacuous pass
    for (const id of designIds) {
      expect(VALID_KEYS.has(atsModeHintKey(id)), id).toBe(true);
    }
  });

  it('the two-column and photo id sets overlap only where documented (portrait, aria, saffron) — no other template is claimed by both', () => {
    // Portrait/Aria/Saffron are DELIBERATELY in both sets (see templates.ts) —
    // atsModeHintKey's precedence gives two-column priority for them, which is
    // exactly what the first test in this block pins. A genuinely disjoint-sets
    // assertion would be false for this codebase by design, so this asserts the
    // narrower, real invariant: the overlap is EXACTLY the known three ids, not
    // more (an accidental extra addition to either set) and not fewer (an
    // accidental removal).
    const ids = Object.keys(TEMPLATES) as (keyof typeof TEMPLATES)[];
    const twoColumnIds = ids.filter((id) => isTwoColumnTemplate(id));
    const photoIds = ids.filter((id) => isPhotoTemplate(id));
    const overlap = twoColumnIds.filter((id) => photoIds.includes(id)).sort();
    expect(overlap).toEqual(['aria', 'portrait', 'saffron']);
  });
});

describe('LETTER_LAYOUT_IDS', () => {
  // Sync guard: MUST equal the Rust `LetterLayout` enum (export/types.rs, kebab-case)
  // and the shared contract union (BaseExportRequest.letterLayoutId). `classic` is
  // first (the default the backend renders for an omitted value).
  it('is the canonical layout id set, classic first', () => {
    expect([...LETTER_LAYOUT_IDS]).toEqual([
      'classic',
      'refined',
      'banded',
      'navy',
      'sidebar',
      'monogram',
    ]);
  });
});

describe('isDecoratedLetterLayout', () => {
  // Mirrors the `ats` gates in the letter .typ files one-for-one. A layout is
  // "decorated" iff ATS mode visibly changes it — that is the ONLY honest reason
  // to surface an ATS toggle on a cover-letter surface.
  it.each(['banded', 'sidebar', 'monogram'] as const)(
    'is true for %s (its .typ drops a decoration under data.opts.ats)',
    (id) => {
      expect(isDecoratedLetterLayout(id)).toBe(true);
    }
  );

  it.each(['classic', 'refined', 'navy'] as const)(
    'is false for %s (no ats gate in its .typ — the toggle would do nothing)',
    (id) => {
      expect(isDecoratedLetterLayout(id)).toBe(false);
    }
  );

  it('treats an unset layout as classic (the backend default), i.e. not decorated', () => {
    expect(isDecoratedLetterLayout(undefined)).toBe(false);
  });

  it('classifies every known layout id — no id falls through to undefined', () => {
    for (const id of LETTER_LAYOUT_IDS) {
      expect(typeof isDecoratedLetterLayout(id), id).toBe('boolean');
    }
    // Guard against a vacuous pass if the id list is ever emptied.
    expect(LETTER_LAYOUT_IDS.filter((id) => isDecoratedLetterLayout(id))).toHaveLength(3);
  });
});

describe('shouldClearAtsMode', () => {
  it('clears for an ATS-tier résumé template when no letter is in play', () => {
    expect(shouldClearAtsMode('classic')).toBe(true);
    expect(shouldClearAtsMode('swiss-minimal')).toBe(true);
  });

  it('keeps the flag for a design-tier template (the toggle is still live there)', () => {
    expect(shouldClearAtsMode('atelier')).toBe(false);
    expect(shouldClearAtsMode('lebenslauf')).toBe(false);
  });

  // The exact review case: ATS-tier résumé template + a decorated cover letter.
  // Clearing here would strand the letter's decoration with no off switch.
  it('does NOT clear when a decorated cover letter is still reading the flag', () => {
    expect(shouldClearAtsMode('classic', isDecoratedLetterLayout('monogram'))).toBe(false);
    expect(shouldClearAtsMode('classic', isDecoratedLetterLayout('sidebar'))).toBe(false);
    expect(shouldClearAtsMode('classic', isDecoratedLetterLayout('banded'))).toBe(false);
  });

  it('still clears for an ATS-tier template when the letter layout is undecorated', () => {
    expect(shouldClearAtsMode('classic', isDecoratedLetterLayout('classic'))).toBe(true);
    expect(shouldClearAtsMode('classic', isDecoratedLetterLayout('navy'))).toBe(true);
  });

  // A cover-ONLY run still carries a templateId (it supplies the letter's
  // palette) but renders no résumé from it, so a design-tier id must not keep
  // the shared flag alive for a document that is not in the export.
  it('ignores the template tier when no résumé is in the run', () => {
    expect(shouldClearAtsMode('atelier', false, true)).toBe(false); // résumé reads it
    expect(shouldClearAtsMode('atelier', false, false)).toBe(true); // nobody does
  });

  it('still keeps the flag in a cover-only run while the letter is decorated', () => {
    expect(shouldClearAtsMode('atelier', true, false)).toBe(false);
    expect(shouldClearAtsMode('classic', true, false)).toBe(false);
  });

  it('defaults to a résumé-bearing run, so existing two-argument callers are unchanged', () => {
    expect(shouldClearAtsMode('atelier', false)).toBe(shouldClearAtsMode('atelier', false, true));
    expect(shouldClearAtsMode('classic', false)).toBe(shouldClearAtsMode('classic', false, true));
  });
});
