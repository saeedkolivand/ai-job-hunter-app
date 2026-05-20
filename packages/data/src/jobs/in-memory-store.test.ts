import { beforeEach, describe, expect, it } from 'vitest';

import { InMemoryJobStore } from './in-memory-store';

const makeJob = (id: string) => ({
  id,
  source: 'linkedin',
  externalId: id,
  url: `https://linkedin.com/jobs/${id}`,
  title: `Job ${id}`,
  company: 'Acme',
  description: 'Some job description',
  capturedAt: Date.now(),
});

describe('InMemoryJobStore', () => {
  let store: InMemoryJobStore;

  beforeEach(() => {
    store = new InMemoryJobStore();
  });

  it('starts empty', () => {
    expect(store.getAll()).toHaveLength(0);
  });

  it('adds and retrieves jobs', () => {
    store.add('scrape-1', makeJob('job-1'));
    store.add('scrape-1', makeJob('job-2'));
    expect(store.getAll()).toHaveLength(2);
  });

  it('clearAll removes everything', () => {
    store.add('scrape-1', makeJob('job-1'));
    store.clearAll();
    expect(store.getAll()).toHaveLength(0);
  });

  it('does not duplicate the same job id', () => {
    const job = makeJob('job-1');
    store.add('scrape-1', job);
    store.add('scrape-1', job); // same id again
    expect(store.getAll()).toHaveLength(1);
  });
});
