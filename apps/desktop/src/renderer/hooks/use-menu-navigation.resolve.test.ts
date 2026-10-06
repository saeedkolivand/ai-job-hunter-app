import { describe, expect, it } from 'vitest';

import { resolveJobDeepLinkTarget } from './use-menu-navigation';

const URL = 'https://boards.greenhouse.io/acme/jobs/1';

describe('resolveJobDeepLinkTarget', () => {
  const applications = [{ id: 'app-1', jobUrl: URL }];

  it('routes generate-for-job to the Documents tab of a matching application', () => {
    expect(resolveJobDeepLinkTarget('generate-for-job', URL, applications)).toEqual({
      kind: 'application',
      id: 'app-1',
      tab: 'documents',
    });
  });

  it('routes open-job to a matching application with no forced tab', () => {
    expect(resolveJobDeepLinkTarget('open-job', URL, applications)).toEqual({
      kind: 'application',
      id: 'app-1',
    });
  });

  it('falls back to a prefilled generate session when no application matches', () => {
    expect(resolveJobDeepLinkTarget('generate-for-job', URL, [])).toEqual({
      kind: 'generate-prefill',
      url: URL,
    });
  });

  it('falls back to a jobs-list search when no application matches', () => {
    expect(resolveJobDeepLinkTarget('open-job', URL, [])).toEqual({
      kind: 'jobs-search',
      url: URL,
    });
  });

  it('routes prep-for-job to the Interview-prep tab of a matching application', () => {
    expect(resolveJobDeepLinkTarget('prep-for-job', URL, applications)).toEqual({
      kind: 'application',
      id: 'app-1',
      tab: 'interview',
    });
  });

  it('falls back to a prefilled generate session when no application matches prep-for-job', () => {
    expect(resolveJobDeepLinkTarget('prep-for-job', URL, [])).toEqual({
      kind: 'generate-prefill',
      url: URL,
    });
  });

  it('never matches on an unnormalizable url (e.g. a non-http scheme)', () => {
    expect(
      resolveJobDeepLinkTarget('open-job', 'javascript:alert(1)', [
        { id: 'app-1', jobUrl: 'javascript:alert(1)' },
      ])
    ).toEqual({ kind: 'jobs-search', url: 'javascript:alert(1)' });
  });
});
