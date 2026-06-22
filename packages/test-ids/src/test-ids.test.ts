import { describe, expect, it } from 'vitest';

import { TEST_IDS } from './test-ids';

/** Recursively collect all leaf string values from the nested TEST_IDS tree. */
function collectLeaves(
  node: Record<string, unknown>,
  path = ''
): Array<{ path: string; value: string }> {
  const results: Array<{ path: string; value: string }> = [];
  for (const [key, val] of Object.entries(node)) {
    const fullPath = path ? `${path}.${key}` : key;
    if (typeof val === 'string') {
      results.push({ path: fullPath, value: val });
    } else if (val !== null && typeof val === 'object') {
      results.push(...collectLeaves(val as Record<string, unknown>, fullPath));
    }
  }
  return results;
}

describe('TEST_IDS', () => {
  it('all leaf values are globally unique (no silent string collisions)', () => {
    const leaves = collectLeaves(TEST_IDS as unknown as Record<string, unknown>);
    const seen = new Map<string, string>();
    const duplicates: string[] = [];

    for (const { path, value } of leaves) {
      if (seen.has(value)) {
        duplicates.push(`"${value}" appears at both "${seen.get(value)}" and "${path}"`);
      } else {
        seen.set(value, path);
      }
    }

    expect(duplicates, `Duplicate test-id values found:\n${duplicates.join('\n')}`).toHaveLength(0);
  });

  it('all leaf values are non-empty strings', () => {
    const leaves = collectLeaves(TEST_IDS as unknown as Record<string, unknown>);
    const empty = leaves.filter(({ value }) => !value || value.trim() === '');
    expect(empty, 'Found empty test-id values').toHaveLength(0);
  });

  it('has expected top-level namespaces', () => {
    const keys = Object.keys(TEST_IDS);
    expect(keys).toContain('layout');
    expect(keys).toContain('jobs');
    expect(keys).toContain('settings');
    expect(keys).toContain('autopilot');
    expect(keys).toContain('applications');
    expect(keys).toContain('documents');
    expect(keys).toContain('resume');
    expect(keys).toContain('generation');
    expect(keys).toContain('onboarding');
  });
});
