/**
 * useHelpChat — retrieval and answer assembly: what the hook sends to
 * `help:search`, what it feeds the model, and how failures surface.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { generateHelpAnswer } from '@/lib/generate';
import { renderHookWithClient } from '@/test-support';

import { useHelpChat } from '../use-help-chat';
import {
  ANSWER,
  ask,
  client,
  dataReads,
  generateArg,
  HIT,
  hybridReply,
  renderChat,
  resetChatMocks,
  searchArg,
} from './test-support';

vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('@ajh/prompts/generate', async (importOriginal) =>
  (await import('./test-mocks')).promptsMock(await importOriginal())
);

/** A keyword-only reply: the dense arm did not run, for `dense` reason. */
const keywordReply = (dense: string) => ({
  results: [HIT],
  mode: 'keyword',
  arms: { lexical: 'ran', dense },
});

describe('useHelpChat — retrieval', () => {
  beforeEach(resetChatMocks);

  it('sends the whole active-locale corpus to help:search, then answers from the hits', async () => {
    const { result, mock } = renderChat();
    await ask(result, '  how do i export a pdf  ');

    const req = searchArg(mock);
    // Trimmed, and the corpus is every shipped entry — Rust does the ranking,
    // the renderer only supplies the text.
    expect(req.query).toBe('how do i export a pdf');
    expect(req.entries.length).toBeGreaterThan(50);
    expect(req.entries.map((entry) => entry.id)).toContain('exportDoc');
    // The `limit` is the prompt builder's own entry budget for this profile.
    expect(req.limit).toBe(3);
    // ONLY the three fields the contract names travel: the section each entry
    // came from is a local routing hint, not part of the wire shape.
    expect(Object.keys(req.entries[0] ?? {}).sort()).toEqual(['body', 'id', 'title']);

    // Only the RANKED entry reaches the model, keyed back by id.
    const gen = generateArg();
    expect(gen.entries).toHaveLength(1);
    expect(gen.question).toBe('how do i export a pdf');
    expect(gen.model).toBe('llama3:70b');

    // Both turns land, and the answer carries its provenance.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user', 'assistant']);
    expect(result.current.turns[1]?.content).toBe(ANSWER);
    expect(result.current.turns[1]?.sources?.map((s) => s.id)).toEqual(['exportDoc']);
    expect(result.current.turns[1]?.mode).toBe('hybrid');
    expect(result.current.streaming).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('asks for fewer entries on a SMALL model (the prompt budget drives the request)', async () => {
    const { result, mock } = renderChat('llama3.2:1b');
    await ask(result, 'how do i export a pdf');
    expect(searchArg(mock).limit).toBe(2);
  });

  it('sends the sidebar’s own page names, translated, as the app-pages list', async () => {
    const { result } = renderChat();
    await ask(result, 'how do i export a pdf');

    const appPages = generateArg().appPages ?? [];
    // Shipped `nav.*` COPY, not the keys: an answer that names a page has to
    // name it the way the sidebar does, or the user cannot find it.
    expect(appPages.map((section) => section.section)).toEqual(['Workspace', 'Automation', '']);
    expect(appPages[0]?.pages).toContain('Dashboard');
    expect(appPages[1]?.pages).toEqual(['Autopilot', 'Best Matches', 'Monitoring']);
    // The pinned group ships no heading and no `nav.sections.*` key names it,
    // so it travels with an empty section name — its PAGES are the part the
    // answer needs, and Settings is where a great many of them end.
    expect(appPages[2]?.pages).toEqual(['Help & Support', 'Settings']);
  });

  it('passes the PRIOR turns as history, never the question being asked', async () => {
    const { result } = renderChat();
    await ask(result, 'first question');
    await ask(result, 'second question');

    const second = vi.mocked(generateHelpAnswer).mock.calls[1]?.[0];
    expect(second?.history?.map((turn) => turn.content)).toEqual(['first question', ANSWER]);
    expect(second?.history?.map((turn) => turn.content)).not.toContain('second question');
  });

  it('flags a keyword-only answer on the turn, WITH the reason the dense arm did not run', async () => {
    const { result } = renderChat('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue(keywordReply('skipped')),
    });
    await ask(result, 'how do i export a pdf');

    expect(result.current.turns[1]?.mode).toBe('keyword');
    // `skipped` (the user's opt-out) and `unavailable` (an embedding failure)
    // both produce `mode: 'keyword'` but need different copy, so the arm status
    // has to survive onto the turn.
    expect(result.current.turns[1]?.dense).toBe('skipped');
  });

  it('carries dense=unavailable through, distinct from the opt-out', async () => {
    const { result } = renderChat('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue(keywordReply('unavailable')),
    });
    await ask(result, 'how do i export a pdf');

    expect(result.current.turns[1]?.dense).toBe('unavailable');
  });

  it('surfaces a retrieval failure and never asks the model to answer from nothing', async () => {
    const { result, mock } = renderChat('llama3:70b', {
      'help.search': vi.fn().mockRejectedValue(new Error('help_search failed')),
    });

    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.send('how do i export a pdf');
    });

    // The boolean is what tells the UI to KEEP the typed question: a failed
    // question that was cleared from the box has to be retyped from memory.
    expect(answered).toBe(false);
    expect(result.current.error).toBe('help_search failed');
    expect(generateHelpAnswer).not.toHaveBeenCalled();
    // The question stays on screen; no assistant turn is fabricated.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user']);
    expect(result.current.streaming).toBe(false);
    // A failed retrieval never reaches the user's own lists either.
    for (const read of dataReads(mock)) expect(read).not.toHaveBeenCalled();
  });

  it('retry re-answers the failed question in place, without asking it twice', async () => {
    const search = vi
      .fn()
      .mockRejectedValueOnce(new Error('help_search failed'))
      .mockResolvedValue(hybridReply(HIT));
    const { result } = renderChat('llama3:70b', { 'help.search': search });

    await ask(result, 'how do i export a pdf');
    expect(result.current.error).toBe('help_search failed');

    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.retry();
    });

    expect(answered).toBe(true);
    expect(result.current.error).toBeNull();
    // ONE user turn, not two: the failed question is already in the transcript.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user', 'assistant']);
    expect(result.current.turns[0]?.content).toBe('how do i export a pdf');
    expect(search.mock.calls[1]?.[0]).toMatchObject({ query: 'how do i export a pdf' });
    // …and the retried question is not fed back to the model as its own history.
    expect(generateArg().history).toEqual([]);
  });

  it('retry does nothing when there is no question to re-ask', async () => {
    const { result, mock } = renderChat();
    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.retry();
    });
    expect(answered).toBe(false);
    expect(mock.help.search).not.toHaveBeenCalled();
  });

  it('does nothing for a blank question or while AI is unavailable', async () => {
    const mock = client();
    const { result } = renderHookWithClient(
      () => useHelpChat({ model: 'llama3:70b', canUse: false }),
      { client: mock }
    );

    await ask(result, 'a real question');
    expect(mock.help.search).not.toHaveBeenCalled();

    const usable = renderChat();
    await ask(usable.result, '   ');
    expect(usable.mock.help.search).not.toHaveBeenCalled();
    expect(usable.result.current.turns).toHaveLength(0);
  });
});
