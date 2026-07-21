import { describe, expect, it } from 'vitest';

import {
  AGENT_COUNT,
  AUTHORS,
  BY_NAME,
  CRITICS,
  CROSS,
  CROSS_NODES,
  PAIRS,
  ROUTES,
  STATIONS,
} from './agent-fleet';

// Typed-data invariants — the same "everything references a real agent" checks
// the drift guard (scripts/check-agent-system.mjs) enforces, as fast unit tests.
describe('agent-fleet data integrity', () => {
  it('BY_NAME indexes every author, critic, and cross agent exactly once', () => {
    expect(BY_NAME.size).toBe(AGENT_COUNT);
    expect(AGENT_COUNT).toBe(AUTHORS.length + CRITICS.length + CROSS.length);
  });

  it('assigns each roster group its declared role', () => {
    for (const tuple of AUTHORS) expect(tuple[1]).toBe('author');
    for (const tuple of CRITICS) expect(tuple[1]).toBe('critic');
    for (const tuple of CROSS) expect(tuple[1]).toBe('cross');
  });

  it('every PAIRS author + critic resolves to a real agent', () => {
    for (const [author, critics] of PAIRS) {
      expect(BY_NAME.get(author)?.[1]).toBe('author');
      for (const critic of critics) expect(BY_NAME.has(critic)).toBe(true);
    }
  });

  it('every CROSS_NODES entry resolves to a real agent', () => {
    for (const name of CROSS_NODES) expect(BY_NAME.has(name)).toBe(true);
  });

  it('every named ROUTES row references a real agent', () => {
    for (const route of ROUTES) {
      for (const row of route.rows) {
        if (row.kind !== 'area') expect(BY_NAME.has(row.name)).toBe(true);
      }
    }
  });

  it('every non-empty station agentTag references a real agent', () => {
    for (const station of STATIONS) {
      if (station.agentTag) expect(BY_NAME.has(station.agentTag)).toBe(true);
    }
    expect(STATIONS).toHaveLength(9);
  });
});
