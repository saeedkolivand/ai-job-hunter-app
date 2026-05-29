import { describe, expect, it } from 'vitest';

import { TEMPLATES } from './templates';

describe('TEMPLATES', () => {
  const ids = Object.keys(TEMPLATES);

  it('exposes the nine document templates keyed by id', () => {
    expect(ids).toHaveLength(9);
    for (const id of ids) {
      expect(TEMPLATES[id as keyof typeof TEMPLATES].id).toBe(id);
    }
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
});
