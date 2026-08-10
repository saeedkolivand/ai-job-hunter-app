import { describe, expect, it } from 'vitest';

import {
  atsModeHintKey,
  isDesignTier,
  isPhotoTemplate,
  isTwoColumnTemplate,
  LETTER_LAYOUT_IDS,
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

  it('is consistent with isTwoColumnTemplate/isPhotoTemplate for every template', () => {
    for (const id of Object.keys(TEMPLATES) as (keyof typeof TEMPLATES)[]) {
      const key = atsModeHintKey(id);
      if (isTwoColumnTemplate(id)) {
        expect(key).toBe('aiGenerate.atsModeHintTwoColumn');
      } else if (isPhotoTemplate(id)) {
        expect(key).toBe('aiGenerate.atsModeHintPhoto');
      } else {
        expect(key).toBe('aiGenerate.atsModeHintDecorative');
      }
    }
  });
});

describe('LETTER_LAYOUT_IDS', () => {
  // Sync guard: MUST equal the Rust `LetterLayout` enum (export/types.rs, kebab-case)
  // and the shared contract union (BaseExportRequest.letterLayoutId). `classic` is
  // first (the default the backend renders for an omitted value).
  it('is the canonical four-layout id set, classic first', () => {
    expect([...LETTER_LAYOUT_IDS]).toEqual(['classic', 'refined', 'banded', 'navy']);
  });
});
