/** Assisted autofill (`fill`) and the passive fillable-fields probe (`fieldsProbe`). */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  activeTab,
  DESKTOP_DOWN,
  EMAIL_SUMMARY,
  executeScriptMock,
  FAKE_TOKEN,
  getTokenMock,
  mockClient,
  resetMocks,
  scriptResults,
  send,
  tabsQueryMock,
} from './test-support';

beforeEach(resetMocks);

const AUTOFILL_OFF = 'Autofill is off. Turn it on in AI Job Hunter → Settings → Browser extension.';

describe('fill request — desktop refusal', () => {
  it('surfaces the profile.result refusal payload (opt-in off) and never injects the filler', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({ error: AUTOFILL_OFF });

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: false, error: AUTOFILL_OFF });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('surfaces a transport failure when getProfile REJECTS (desktop unreachable), never injects the filler', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockRejectedValue(new Error(DESKTOP_DOWN));

    const res = await send({ kind: 'fill' });

    // The dispatcher's outer try/catch converts a thrown Error to ok:false.
    expect(res).toEqual({ ok: false, error: DESKTOP_DOWN });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

describe('fill request — injection', () => {
  beforeEach(() => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
  });

  it('surfaces "No active tab to fill." when the tab query returns none', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: false, error: 'No active tab to fill.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('surfaces "Could not fill the form on this page." when the injected func returns a non-summary', async () => {
    activeTab('https://example.com/apply');
    scriptResults(undefined, null);

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: false, error: 'Could not fill the form on this page.' });
    expect(executeScriptMock).toHaveBeenCalledTimes(2);
  });

  it('returns the fill summary when the profile, tab, and injection all succeed', async () => {
    activeTab('https://example.com/apply');
    scriptResults(undefined, EMAIL_SUMMARY);

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: true, kind: 'fill', summary: EMAIL_SUMMARY });
  });

  it('forwards extraLinks from the profile.result reply into the injected fill fields', async () => {
    mockClient.getProfile.mockResolvedValue({
      email: 'saeed@example.com',
      extraLinks: [{ label: 'Portfolio', url: 'https://saeed.dev' }],
    });
    activeTab('https://example.com/apply');
    scriptResults(undefined, EMAIL_SUMMARY);

    await send({ kind: 'fill' });

    const secondCallArgs = executeScriptMock.mock.calls[1]?.[0] as { args?: unknown[] };
    const [fields] = secondCallArgs.args as [{ extraLinks?: unknown }];
    expect(fields.extraLinks).toEqual([{ label: 'Portfolio', url: 'https://saeed.dev' }]);
  });
});

describe('fieldsProbe request — always ok:true, EVERY failure fails OPEN', () => {
  const probe = (hasFormFields: boolean, hasAnswerFields: boolean) => ({
    ok: true,
    kind: 'fieldsProbe',
    hasFormFields,
    hasAnswerFields,
  });

  it('returns the injected probe result on success (both signals true)', async () => {
    activeTab('https://jobs.example.com/apply');
    scriptResults({ hasFormFields: true, hasAnswerFields: true });

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual(probe(true, true));
    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 7 },
      files: ['probe-fields.js'],
    });
    // Never touches the token/bridge — this is a page-only, offline-safe read.
    expect(getTokenMock).not.toHaveBeenCalled();
  });

  it.each([
    [
      'an identity-only-form result (hasFormFields true, hasAnswerFields false — the union split)',
      true,
      false,
    ],
    ['no fields at all', false, false],
  ])('passes through %s', async (_label, hasFormFields, hasAnswerFields) => {
    activeTab('https://jobs.example.com/apply');
    scriptResults({ hasFormFields, hasAnswerFields });

    expect(await send({ kind: 'fieldsProbe' })).toEqual(probe(hasFormFields, hasAnswerFields));
  });

  it('fails OPEN (both signals true) when there is no active tab', async () => {
    tabsQueryMock.mockResolvedValue([]);

    expect(await send({ kind: 'fieldsProbe' })).toEqual(probe(true, true));
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it.each([
    [
      'the injected result is malformed (missing/non-boolean fields)',
      () => scriptResults({ hasFormFields: true }),
    ],
    [
      'executeScript REJECTS (restricted page/scripting denied)',
      () => executeScriptMock.mockRejectedValueOnce(new Error('Cannot access a chrome:// URL')),
    ],
  ])('fails OPEN (both signals true) when %s', async (_label, arrange) => {
    activeTab('https://jobs.example.com/apply');
    arrange();

    expect(await send({ kind: 'fieldsProbe' })).toEqual(probe(true, true));
  });
});
