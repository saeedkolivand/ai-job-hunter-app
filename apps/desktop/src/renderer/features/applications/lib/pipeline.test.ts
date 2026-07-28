import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { type Application, APPLICATION_STAGES } from '@ajh/shared';

import {
  overdueCount,
  PIPELINE_GROUPS,
  pipelineCounts,
  sortApplications,
  stagesForGroup,
  toApplicationSort,
} from './pipeline';

function makeApp(overrides: Partial<Application>): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: 1000,
    updatedAt: 1000,
    jobUrl: '',
    board: '',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    jobDescription: '',
    jobSummary: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

describe('PIPELINE_GROUPS', () => {
  it('covers EVERY backend stage exactly once (a new stage cannot fall out of the strip)', () => {
    const covered = PIPELINE_GROUPS.flatMap((g) => [...g.stages]);
    const stageIds = APPLICATION_STAGES.map((s) => s.id);

    expect([...covered].sort()).toEqual([...stageIds].sort());
    expect(new Set(covered).size).toBe(covered.length);
  });

  it('renders six cards, with `closed` aggregating the four end-states', () => {
    expect(PIPELINE_GROUPS).toHaveLength(6);
    const closed = PIPELINE_GROUPS.find((g) => g.id === 'closed');
    expect(closed?.stages).toEqual(['accepted', 'rejected', 'ghosted', 'withdrawn']);
  });
});

describe('stagesForGroup', () => {
  it('returns the stage list for a known group', () => {
    expect(stagesForGroup('offer')).toEqual(['offer']);
    expect(stagesForGroup('closed')).toHaveLength(4);
  });

  it('degrades an unknown or absent group to "no filter" rather than an empty list', () => {
    expect(stagesForGroup(null)).toBeNull();
    expect(stagesForGroup(undefined)).toBeNull();
    expect(stagesForGroup('')).toBeNull();
    expect(stagesForGroup('not-a-group')).toBeNull();
  });
});

describe('pipelineCounts', () => {
  it('always carries an entry for every group, including zeros', () => {
    const counts = pipelineCounts([]);
    expect(Object.keys(counts).sort()).toEqual(PIPELINE_GROUPS.map((g) => g.id).sort());
    expect(Object.values(counts).every((n) => n === 0)).toBe(true);
  });

  it('folds all four end-states into `closed`', () => {
    const counts = pipelineCounts([
      makeApp({ id: 'a', status: 'accepted' }),
      makeApp({ id: 'b', status: 'rejected' }),
      makeApp({ id: 'c', status: 'ghosted' }),
      makeApp({ id: 'd', status: 'withdrawn' }),
      makeApp({ id: 'e', status: 'offer' }),
    ]);
    expect(counts.closed).toBe(4);
    expect(counts.offer).toBe(1);
    expect(counts.saved).toBe(0);
  });
});

describe('overdueCount', () => {
  const NOW = 1_700_000_000_000;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => vi.useRealTimers());

  it('counts only reminders already in the past', () => {
    const apps = [
      makeApp({ id: 'a', nextActionAt: NOW - 86_400_000 }),
      makeApp({ id: 'b', nextActionAt: NOW - 2 * 86_400_000 }),
      makeApp({ id: 'c', nextActionAt: NOW + 86_400_000 }),
      makeApp({ id: 'd', nextActionAt: undefined }),
    ];
    expect(overdueCount(apps)).toBe(2);
  });

  it('is zero for an empty list', () => {
    expect(overdueCount([])).toBe(0);
  });
});

describe('toApplicationSort', () => {
  it('passes through the known sorts', () => {
    expect(toApplicationSort('company')).toBe('company');
    expect(toApplicationSort('nextAction')).toBe('nextAction');
  });

  it('falls back to `updated` for anything else (stale persisted state)', () => {
    expect(toApplicationSort(null)).toBe('updated');
    expect(toApplicationSort(undefined)).toBe('updated');
    expect(toApplicationSort('bogus')).toBe('updated');
  });
});

describe('sortApplications', () => {
  const apps = [
    makeApp({ id: 'a', company: 'Zeta', updatedAt: 300, nextActionAt: 900 }),
    makeApp({ id: 'b', company: 'Alpha', updatedAt: 100, nextActionAt: undefined }),
    makeApp({ id: 'c', company: 'Mid', updatedAt: 200, nextActionAt: 500 }),
  ];

  it('does not mutate the input array', () => {
    const input = [...apps];
    sortApplications(input, 'company');
    expect(input.map((a) => a.id)).toEqual(['a', 'b', 'c']);
  });

  it('`updated` is most-recent-first', () => {
    expect(sortApplications(apps, 'updated').map((a) => a.id)).toEqual(['a', 'c', 'b']);
  });

  it('`company` is A→Z, tie-broken by title', () => {
    expect(sortApplications(apps, 'company').map((a) => a.id)).toEqual(['b', 'c', 'a']);

    const sameCompany = [
      makeApp({ id: 'x', company: 'Acme', title: 'Zookeeper' }),
      makeApp({ id: 'y', company: 'Acme', title: 'Architect' }),
    ];
    expect(sortApplications(sameCompany, 'company').map((a) => a.id)).toEqual(['y', 'x']);
  });

  it('`nextAction` puts the soonest reminder first and sinks reminder-less rows', () => {
    expect(sortApplications(apps, 'nextAction').map((a) => a.id)).toEqual(['c', 'a', 'b']);
  });

  it('`nextAction` orders reminder-less rows among themselves by recency', () => {
    const none = [makeApp({ id: 'old', updatedAt: 100 }), makeApp({ id: 'new', updatedAt: 900 })];
    expect(sortApplications(none, 'nextAction').map((a) => a.id)).toEqual(['new', 'old']);
  });
});
