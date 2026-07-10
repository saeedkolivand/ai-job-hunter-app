import { describe, expect, it } from 'vitest';

import { isDesignTier, TEMPLATES } from './templates';

describe('TEMPLATES', () => {
  const ids = Object.keys(TEMPLATES);

  it('exposes the eight document templates keyed by id', () => {
    expect(ids).toHaveLength(8);
    for (const id of ids) {
      expect(TEMPLATES[id as keyof typeof TEMPLATES].id).toBe(id);
    }
  });

  // Sync guard: this id set MUST equal the Rust `TemplateId` enum (export/types.rs,
  // kebab-case) and the shared contract union (packages/shared/.../documents.ts).
  // The Rust round-trip test pins the other side; if either drifts, a guard fails.
  it('matches the canonical 8-template id set', () => {
    expect([...ids].sort()).toEqual([
      'academic',
      'atelier',
      'classic',
      'lebenslauf',
      'meridian',
      'portrait',
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
      'classic',
      'meridian',
      'swiss-minimal',
      'throughline',
    ]);
    expect(idsByTier('design')).toEqual(['atelier', 'lebenslauf', 'portrait']);
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
