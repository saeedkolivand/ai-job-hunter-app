import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

// The generation half is the seam: stub it so the test asserts what the hook
// FEEDS the model (entries, glance, history) rather than re-testing streaming.
vi.mock('@/lib/generate', () => ({
  generateHelpAnswer: vi.fn().mockResolvedValue('Open the document and click Export.'),
}));

/**
 * What the hook HANDED `buildHelpDataGlance`, recorded without changing what it
 * builds. The rendered glance cannot answer "how much did the hook disclose":
 * the prompt renders at most 10 autopilots itself, so a hook that passed 500
 * would produce a byte-identical string. This is the boundary where the
 * disclosure actually happens, so this is where it is measured.
 */
const glanceRecorder = vi.hoisted(() => ({ inputs: [] as unknown[] }));

vi.mock('@ajh/prompts/generate', async (importOriginal) => {
  const actual = await importOriginal<typeof PromptsGenerate>();
  return {
    ...actual,
    buildHelpDataGlance: (input: Parameters<typeof actual.buildHelpDataGlance>[0]) => {
      glanceRecorder.inputs.push(input);
      return actual.buildHelpDataGlance(input);
    },
  };
});

/**
 * Opt-in for ONE test: hand the hook the real, mutable i18n instance.
 *
 * react-i18next v17 does not return the instance from `useTranslation()` — it
 * returns a per-render COPY of it, so `i18n.language` inside a closure is
 * frozen at the render that made the closure and a mid-flight switch is
 * invisible. That freeze is a dependency's implementation detail (v16 returned
 * the live instance, and the app's own instance is live), which is exactly what
 * a test of OUR invariant must not lean on. Off for every other test here.
 */
const live = vi.hoisted(() => ({ i18n: false }));

/** Just the two members this file re-wraps — the mock factory's return is not
 *  checked against the real module, and callers keep the real module's types. */
interface TranslationsModule {
  default: unknown;
  useTranslation: (...args: unknown[]) => { t: unknown; i18n: unknown; ready: boolean };
}

vi.mock('@ajh/translations', async (importOriginal) => {
  const actual = await importOriginal<TranslationsModule>();
  return {
    ...actual,
    useTranslation: (...args: unknown[]) => {
      const result = actual.useTranslation(...args);
      if (!live.i18n) return result;
      // The same array-plus-properties shape react-i18next returns, with the
      // frozen copy swapped for the instance the test can actually mutate.
      return Object.assign([result.t, actual.default, result.ready], {
        t: result.t,
        i18n: actual.default,
        ready: result.ready,
      });
    },
  };
});

import type * as PromptsGenerate from '@ajh/prompts/generate';
import i18n from '@ajh/translations';

import { generateHelpAnswer } from '@/lib/generate';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { useHelpChat } from './use-help-chat';

/** `exportDoc` lives in the aiGenerate section — NOT the applications one. */
const HIT = { id: 'exportDoc', score: 0.9 };
/** `trackJob` is a `support.faq.applicationsQuestions.*` entry. */
const APPLICATIONS_HIT = { id: 'trackJob', score: 0.9 };
/** `setUpAutopilot` is a `support.faq.autopilotQuestions.*` entry. */
const AUTOPILOT_HIT = { id: 'setUpAutopilot', score: 0.9 };

/**
 * Two autopilots as the backend returns them: user-typed names, and the rest of
 * the record — including the résumé text the glance must never carry.
 */
const AUTOPILOTS = [
  {
    _id: 'ap1',
    name: 'Berlin React roles',
    status: 'active',
    runStatus: 'completed',
    totalFound: 12,
    resumeText: 'SECRET resume text',
  },
  { _id: 'ap2', name: 'Remote Rust', status: 'paused', totalFound: 0 },
];

function client(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  return createMockClient({
    'help.search': vi.fn().mockResolvedValue({
      results: [HIT],
      mode: 'hybrid',
      arms: { lexical: 'ran', dense: 'ran' },
    }),
    'ai.embeddingStatus': vi
      .fn()
      .mockResolvedValue({ documents: { total: 3, indexedInActiveSpace: 3, stale: 0 } }),
    'scrape.listInteractions': vi.fn().mockResolvedValue([
      { interactionType: 'viewed' },
      { interactionType: 'viewed' },
      // `dismissed` is NOT a tracked type — it must never reach the glance.
      { interactionType: 'dismissed' },
    ]),
    'applications.list': vi
      .fn()
      .mockResolvedValue([
        { id: 'a1', title: 'Senior Engineer', company: 'Acme', status: 'applied', updatedAt: 2 },
      ]),
    'autopilot.list': vi.fn().mockResolvedValue(AUTOPILOTS),
    ...overrides,
  });
}

/**
 * Render the hook. Nothing is awaited here on purpose: the four lists behind the
 * data glance are fetched inside `send`, not on mount, so there is no
 * "wait for the queries to land" step — see the privacy test below.
 *
 * `llama3` has no parseable parameter size, so `detectModelSize` classifies it
 * `small`; `llama3:70b` is the large-tier model in these tests.
 */
function render(model = 'llama3:70b', overrides = {}) {
  const mock = client(overrides);
  const rendered = renderHookWithClient(() => useHelpChat({ model, canUse: true }), {
    client: mock,
  });
  return { ...rendered, mock };
}

interface SearchRequest {
  queryId: string;
  locale: string;
  query: string;
  entries: Array<{ id: string; title: string; body: string }>;
  limit: number;
}

const searchArgAt = (search: ReturnType<typeof vi.fn>, index: number) =>
  search.mock.calls[index]?.[0] as SearchRequest;

const searchArg = (mock: ReturnType<typeof client>) =>
  searchArgAt(mock.help.search as ReturnType<typeof vi.fn>, 0);

const generateArg = () =>
  vi.mocked(generateHelpAnswer).mock.calls[0]?.[0] as Parameters<typeof generateHelpAnswer>[0];

/** The one field these tests read back off {@link glanceRecorder}. */
interface RecordedGlanceInput {
  autopilots?: ReadonlyArray<{ name: string }> | null;
}

/** The autopilot list as the hook passed it, before the prompt renders it. */
const glanceAutopilotsSent = () =>
  (glanceRecorder.inputs.at(-1) as RecordedGlanceInput | undefined)?.autopilots ?? null;

/** The four reads that make up the data glance. */
const dataReads = (mock: ReturnType<typeof client>) =>
  [
    mock.ai.embeddingStatus,
    mock.scrape.listInteractions,
    mock.applications.list,
    mock.autopilot.list,
  ] as ReturnType<typeof vi.fn>[];

describe('useHelpChat', () => {
  beforeEach(() => {
    glanceRecorder.inputs.length = 0;
    vi.mocked(generateHelpAnswer).mockClear();
    vi.mocked(generateHelpAnswer).mockResolvedValue('Open the document and click Export.');
  });

  it('sends the whole active-locale corpus to help:search, then answers from the hits', async () => {
    const { result, mock } = render();

    await act(async () => {
      await result.current.send('  how do i export a pdf  ');
    });

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
    expect(result.current.turns[1]?.content).toBe('Open the document and click Export.');
    expect(result.current.turns[1]?.sources?.map((s) => s.id)).toEqual(['exportDoc']);
    expect(result.current.turns[1]?.mode).toBe('hybrid');
    expect(result.current.streaming).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('asks for fewer entries on a SMALL model (the prompt budget drives the request)', async () => {
    const { result, mock } = render('llama3.2:1b');
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    expect(searchArg(mock).limit).toBe(2);
  });

  it('builds the data glance from the user’s own counts, excluding untracked interactions', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('what have i done so far');
    });

    const glance = generateArg().dataGlance ?? '';
    expect(glance).toContain('Documents imported: 3');
    // Two `viewed`; the `dismissed` row is excluded by the tracked allowlist —
    // a dismissal is the opposite of tracking, so it must not inflate this.
    expect(glance).toContain('viewed 2');
    expect(glance).not.toContain('dismissed');
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).toContain('Autopilots configured: 2');
  });

  it('reads none of the user’s lists until a question is actually asked', async () => {
    const { result, mock } = render();

    // Opening the Help page to read ONE entry must not read the user's
    // documents, interactions, applications or autopilots. Mounting the four
    // queries would have issued all four before a question existed.
    for (const read of dataReads(mock)) expect(read).not.toHaveBeenCalled();

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    for (const read of dataReads(mock)) expect(read).toHaveBeenCalledTimes(1);
  });

  it('withholds the recent-application NAMES unless the question retrieved an applications entry', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    const glance = generateArg().dataGlance ?? '';
    // Counts are always safe to send; the scraped job titles and company names
    // are not, and an export question is not made better by them.
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).not.toContain('Senior Engineer');
    expect(glance).not.toContain('Acme');
  });

  it('includes the recent-application names when an applications entry was retrieved', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [APPLICATIONS_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
    });
    await act(async () => {
      await result.current.send('which jobs have i applied to');
    });

    expect(generateArg().dataGlance ?? '').toContain('Senior Engineer — Acme (applied)');
  });

  it('withholds the recent-application names when an applications entry is only a SECONDARY hit', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        // An export question, with the applications entry as the rank-2 maybe
        // the ranker kept behind it.
        results: [HIT, APPLICATIONS_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
    });
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    const glance = generateArg().dataGlance ?? '';
    // The answer is written from the TOP entry, so a secondary hit must not
    // widen what leaves the machine — the counts still travel.
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).not.toContain('Senior Engineer');
    expect(glance).not.toContain('Acme');
  });

  it('sends the sidebar’s own page names, translated, as the app-pages list', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

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

  it('withholds the autopilot NAMES unless the question retrieved an autopilot entry', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    const glance = generateArg().dataGlance ?? '';
    // The count is always safe; the user-typed names are not, and an export
    // question is not answered any better for having them.
    expect(glance).toContain('Autopilots configured: 2');
    expect(glance).not.toContain('Berlin React roles');
    expect(glance).not.toContain('Remote Rust');
  });

  it('includes the autopilot names when an autopilot entry was retrieved, and nothing else off the record', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [AUTOPILOT_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
    });
    await act(async () => {
      await result.current.send('how do i set up an autopilot');
    });

    const glance = generateArg().dataGlance ?? '';
    expect(glance).toContain('Berlin React roles — active, completed (12 found)');
    // No run yet → no run status, rather than an invented one.
    expect(glance).toContain('Remote Rust — paused (0 found)');
    // Four fields travel, so the résumé text on the same record does not.
    expect(glance).not.toContain('SECRET resume text');
  });

  it('withholds the autopilot names when an autopilot entry is only a SECONDARY hit', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        // The shape observed live: a LinkedIn-import question whose rank-2 hit
        // was `setUpAutopilot`. Gating on "any retrieved entry" sent the user's
        // autopilot names to the provider for a question the top entry answers.
        results: [HIT, AUTOPILOT_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
    });
    await act(async () => {
      await result.current.send('how do i import my linkedin profile');
    });

    const glance = generateArg().dataGlance ?? '';
    expect(glance).toContain('Autopilots configured: 2');
    expect(glance).not.toContain('Berlin React roles');
    expect(glance).not.toContain('Remote Rust');
  });

  it('sends at most the 10 autopilot names the glance renders', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [AUTOPILOT_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
      'autopilot.list': vi.fn().mockResolvedValue(
        Array.from({ length: 12 }, (_, index) => ({
          _id: `ap${index}`,
          name: `Autopilot number ${index + 1}`,
          status: 'active',
          totalFound: 0,
        }))
      ),
    });
    await act(async () => {
      await result.current.send('how do i set up an autopilot');
    });

    // The COUNT covers all 12 — that line is a number, not user-typed text.
    expect(generateArg().dataGlance ?? '').toContain('Autopilots configured: 12');
    // The NAMES stop where the prompt's `Autopilots:` list does. Asserted on
    // what the hook PASSED, not on the rendered glance: the prompt slices to 10
    // as well, so the string is identical either way and would prove nothing.
    const sent = glanceAutopilotsSent() ?? [];
    expect(sent.map((autopilot) => autopilot.name)).toEqual(
      Array.from({ length: 10 }, (_, index) => `Autopilot number ${index + 1}`)
    );
  });

  it('claims nothing about autopilots when that source could not be read', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [AUTOPILOT_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
      'autopilot.list': vi.fn().mockRejectedValue(new Error('database is locked')),
    });
    await act(async () => {
      await result.current.send('how do i set up an autopilot');
    });

    // The answer still lands — the glance is a garnish on a corpus answer.
    expect(result.current.error).toBeNull();
    expect(result.current.turns[1]?.role).toBe('assistant');

    const glance = generateArg().dataGlance ?? '';
    // The glance says NOTHING about autopilots — no count and no names. An
    // unreadable list must omit its lines rather than tell the model this user
    // has none, which the answer would then state as fact. (`null` and `[]`
    // render the same names-wise; the missing COUNT line is what distinguishes
    // an unread source from an empty one here.)
    expect(glance).not.toContain('Autopilots configured');
    expect(glance).not.toContain('Autopilots:');
    expect(glance).not.toContain('Berlin React roles');
    // The sources that DID answer are unaffected.
    expect(glance).toContain('Documents imported: 3');
  });

  it('passes the PRIOR turns as history, never the question being asked', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('first question');
    });
    await act(async () => {
      await result.current.send('second question');
    });

    const second = vi.mocked(generateHelpAnswer).mock.calls[1]?.[0];
    expect(second?.history?.map((turn) => turn.content)).toEqual([
      'first question',
      'Open the document and click Export.',
    ]);
    expect(second?.history?.map((turn) => turn.content)).not.toContain('second question');
  });

  it('still answers when ONE glance source fails, and omits only that source', async () => {
    // The glance is a garnish on an answer grounded in the help corpus, so a
    // single unreadable source must cost its own line and nothing else. Under
    // `Promise.all` this rejection failed the whole question.
    const { result } = render('llama3:70b', {
      'applications.list': vi.fn().mockRejectedValue(new Error('database is locked')),
    });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    // The answer landed: an assistant turn, no error.
    expect(result.current.error).toBeNull();
    expect(result.current.turns[1]?.role).toBe('assistant');
    expect(result.current.turns[1]?.content).toBe('Open the document and click Export.');

    const glance = generateArg().dataGlance ?? '';
    // The failed source is ABSENT — not reported as "Applications tracked: 0",
    // which the model would state as fact about a user who has applications.
    expect(glance).not.toContain('Applications tracked');
    // The three that answered are all still there.
    expect(glance).toContain('Documents imported: 3');
    expect(glance).toContain('viewed 2');
    expect(glance).toContain('Autopilots configured: 2');
  });

  it('flags a keyword-only answer on the turn, WITH the reason the dense arm did not run', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [HIT],
        mode: 'keyword',
        arms: { lexical: 'ran', dense: 'skipped' },
      }),
    });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    expect(result.current.turns[1]?.mode).toBe('keyword');
    // `skipped` (the user's opt-out) and `unavailable` (an embedding failure)
    // both produce `mode: 'keyword'` but need different copy, so the arm status
    // has to survive onto the turn.
    expect(result.current.turns[1]?.dense).toBe('skipped');
  });

  it('carries dense=unavailable through, distinct from the opt-out', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [HIT],
        mode: 'keyword',
        arms: { lexical: 'ran', dense: 'unavailable' },
      }),
    });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    expect(result.current.turns[1]?.dense).toBe('unavailable');
  });

  it('surfaces a retrieval failure and never asks the model to answer from nothing', async () => {
    const { result, mock } = render('llama3:70b', {
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
      .mockResolvedValue({
        results: [HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      });
    const { result } = render('llama3:70b', { 'help.search': search });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
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
    const { result, mock } = render();
    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.retry();
    });
    expect(answered).toBe(false);
    expect(mock.help.search).not.toHaveBeenCalled();
  });

  it('stop() aborts the stream and keeps the partial answer as the assistant turn', async () => {
    let abortSignal: AbortSignal | undefined;
    let release: (() => void) | undefined;
    vi.mocked(generateHelpAnswer).mockImplementationOnce(
      ({ onToken, signal }) =>
        new Promise((resolve) => {
          abortSignal = signal;
          onToken?.('Open the ');
          release = () => resolve('never used');
        })
    );

    const { result } = render();
    let pending: Promise<boolean> | undefined;
    await act(async () => {
      pending = result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.answer).toBe('Open the '));
    });

    act(() => {
      result.current.stop();
    });

    expect(abortSignal?.aborted).toBe(true);
    expect(result.current.streaming).toBe(false);
    expect(result.current.answer).toBe('');
    // The half-written answer is kept, WITH the sources retrieval had already
    // settled — losing it on Stop would throw away what the user asked for.
    expect(result.current.turns[1]?.content).toBe('Open the ');
    expect(result.current.turns[1]?.sources?.map((s) => s.id)).toEqual(['exportDoc']);

    await act(async () => {
      release?.();
      await pending;
    });
    // The resolved-after-abort value must NOT append a second assistant turn.
    expect(result.current.turns).toHaveLength(2);
  });

  it('a Stop during RETRIEVAL keeps the busy state until help_search settles', async () => {
    let settle: ((value: unknown) => void) | undefined;
    const search = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        })
    );
    const { result } = render('llama3:70b', { 'help.search': search });

    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    act(() => {
      result.current.stop();
    });

    // The cancel below makes the backend give up SOONER, not instantly (it is
    // checked between dense candidates), and the invoke promise is not
    // abortable either way. Handing the Ask button back HERE is what lets a
    // second cold search run alongside the first one.
    expect(result.current.streaming).toBe(true);
    expect(search).toHaveBeenCalledTimes(1);

    await act(async () => {
      settle?.({ results: [HIT], mode: 'hybrid', arms: { lexical: 'ran', dense: 'ran' } });
      await waitFor(() => expect(result.current.streaming).toBe(false));
    });

    // …and the abort still did its half of the job: the reply was dropped, so
    // no model call and no assistant turn came out of the stopped question.
    expect(generateHelpAnswer).not.toHaveBeenCalled();
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user']);
  });

  // ── cancellation of the retrieval leg ──────────────────────────────────────
  //
  // `help_search` runs a dense arm that embeds one entry per cache miss, and a
  // Tauri invoke is not abortable from the renderer. `jobs.cancel(queryId)` is
  // the ONLY way to stop that work; the id below is the only thing that names
  // it, which is why the shape of the id is asserted and not just its presence.

  it('mints a distinct help- queryId per question and sends the active UI locale', async () => {
    const { result, mock } = render();
    const search = mock.help.search as ReturnType<typeof vi.fn>;

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    await act(async () => {
      await result.current.send('and how do i track a job');
    });

    const first = searchArgAt(search, 0);
    const second = searchArgAt(search, 1);
    for (const req of [first, second]) {
      // The prefix is what keeps this id from naming a live `job-`/`run-`/
      // `search-` run in the shared cancel registry; the cap is the schema's.
      expect(req.queryId.startsWith('help-')).toBe(true);
      expect(req.queryId.length).toBeLessThanOrEqual(64);
      // A full BCP-47 tag, region and all (`en-US` here) — the Rust side
      // normalises to the primary subtag, so this is deliberately not pinned
      // to a bare `en`.
      expect(req.locale).toMatch(/^en(-|$)/);
    }
    // Per QUESTION, not per session: a reused id would let a cancel for the
    // first question kill the second one's retrieval.
    expect(second.queryId).not.toBe(first.queryId);
  });

  it('follows the UI language rather than sending a fixed locale', async () => {
    // Without this the assertion above passes for a hardcoded 'en': the whole
    // point of the field is that a German corpus is filtered with the German
    // function-word list.
    await act(async () => {
      await i18n.changeLanguage('de');
    });
    try {
      const { result, mock } = render();
      await act(async () => {
        await result.current.send('wie exportiere ich ein pdf');
      });
      expect(searchArg(mock).locale).toMatch(/^de(-|$)/);
    } finally {
      await act(async () => {
        await i18n.changeLanguage('en-US');
      });
    }
  });

  it('pins ONE locale per question, even if the UI language changes mid-retrieval', async () => {
    // The run reads the language TWICE — once for the request that picks the
    // function-word list, once for the answer's output language — either side
    // of an await that lasts as long as the dense arm does. Read live, a switch
    // inside that window filters the question in English and then answers it in
    // German: two halves of one answer in different locales, with nothing in
    // the UI saying so.
    //
    // Run against the LIVE i18n instance — see the `live` mock above, without
    // which the switch below is invisible to the hook and this test passes on
    // the broken code.
    live.i18n = true;
    const asked = i18n.language;
    expect(asked).toMatch(/^en(-|$)/);

    let settle: ((value: unknown) => void) | undefined;
    const search = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        })
    );

    try {
      const { result } = render('llama3:70b', { 'help.search': search });

      await act(async () => {
        void result.current.send('how do i export a pdf');
        await waitFor(() => expect(result.current.streaming).toBe(true));
      });

      // The switch lands while retrieval is still pending — the whole point.
      await act(async () => {
        await i18n.changeLanguage('de');
      });
      expect(i18n.language).toBe('de');

      await act(async () => {
        settle?.({ results: [HIT], mode: 'hybrid', arms: { lexical: 'ran', dense: 'ran' } });
        await waitFor(() => expect(result.current.streaming).toBe(false));
      });

      // Both legs carry the locale the question was ASKED in, not the one the
      // UI drifted to while the dense arm was still embedding.
      expect(searchArgAt(search, 0).locale).toBe(asked);
      expect(generateArg().language).toBe(asked);
    } finally {
      live.i18n = false;
      await act(async () => {
        await i18n.changeLanguage(asked);
      });
    }
  });

  it('a Stop during RETRIEVAL cancels the backend leg by the id it sent', async () => {
    let settle: ((value: unknown) => void) | undefined;
    const search = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        })
    );
    const { result, mock } = render('llama3:70b', { 'help.search': search });

    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });
    const { queryId } = searchArgAt(search, 0);

    act(() => {
      result.current.stop();
    });

    await waitFor(() => expect(mock.jobs.cancel).toHaveBeenCalledWith(queryId));

    // Once the leg settles the id names nothing, so a later Stop must not fire
    // a second cancel — that would be a no-op that still emits a jobs event.
    await act(async () => {
      settle?.({ results: [HIT], mode: 'hybrid', arms: { lexical: 'ran', dense: 'ran' } });
      await waitFor(() => expect(result.current.streaming).toBe(false));
    });
    act(() => {
      result.current.stop();
    });
    expect(mock.jobs.cancel).toHaveBeenCalledTimes(1);
  });

  it('unmounting mid-RETRIEVAL cancels the leg — navigating away is the common case', async () => {
    const search = vi.fn().mockImplementation(() => new Promise(() => {}));
    const { result, mock, unmount } = render('llama3:70b', { 'help.search': search });

    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });
    const { queryId } = searchArgAt(search, 0);

    unmount();

    await waitFor(() => expect(mock.jobs.cancel).toHaveBeenCalledWith(queryId));
  });

  it('unmounting mid-STREAM does not cancel — the retrieval leg is already done', async () => {
    vi.mocked(generateHelpAnswer).mockImplementationOnce(() => new Promise(() => {}));
    const { result, mock, unmount } = render();

    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(generateHelpAnswer).toHaveBeenCalled());
    });

    unmount();
    // A bare assertion straight after `unmount()` would pass for the wrong
    // reason: `mutateAsync` reaches the client several microtasks later, so
    // nothing has been called yet either way. Wait past the point where the
    // mid-RETRIEVAL test's cancel has already landed.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // `jobs.cancel` on a finished id is not free: it emits `job.cancelled` for
    // an id with no job record and invalidates the whole jobs list.
    expect(mock.jobs.cancel).not.toHaveBeenCalled();
  });

  it('a second question cancels the retrieval of the run it replaces', async () => {
    const search = vi
      .fn()
      .mockImplementationOnce(() => new Promise(() => {}))
      .mockResolvedValue({
        results: [HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      });
    const { result, mock } = render('llama3:70b', { 'help.search': search });
    // The handle a component captured BEFORE the first run re-rendered: its
    // `streaming` guard is a closed-over `false`, so both calls get in.
    const staleSend = result.current.send;

    await act(async () => {
      void staleSend('first question');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });
    const first = searchArgAt(search, 0);

    await act(async () => {
      await staleSend('second question');
    });

    expect(mock.jobs.cancel).toHaveBeenCalledWith(first.queryId);
    expect(searchArgAt(search, 1).queryId).not.toBe(first.queryId);
  });

  it('a superseded run settling late leaves the SECOND question cancellable', async () => {
    // The test above leaves the first leg pending forever, so it cannot see
    // this: the superseded run only reaches its post-`await` bookkeeping when
    // it RESOLVES, and that is where an unguarded `queryIdRef.current = null`
    // wipes the id the replacing run had already minted — silently making the
    // question the user is actually waiting on uncancellable.
    let settleFirst: ((value: unknown) => void) | undefined;
    const search = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            settleFirst = resolve;
          })
      )
      // The second leg stays in retrieval so there is something for `stop()`
      // to cancel at the moment the first one settles.
      .mockImplementation(() => new Promise(() => {}));
    const { result, mock } = render('llama3:70b', { 'help.search': search });
    const staleSend = result.current.send;

    await act(async () => {
      void staleSend('first question');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    await act(async () => {
      void staleSend('second question');
      await waitFor(() => expect(search).toHaveBeenCalledTimes(2));
    });
    const second = searchArgAt(search, 1);

    // The replaced run finishes now — after the supersede, not before.
    await act(async () => {
      settleFirst?.({ results: [HIT], mode: 'hybrid', arms: { lexical: 'ran', dense: 'ran' } });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    act(() => {
      result.current.stop();
    });

    await waitFor(() => expect(mock.jobs.cancel).toHaveBeenCalledWith(second.queryId));
  });

  it('a second question aborts the run it replaces — one assistant turn, not two', async () => {
    const streams: AbortSignal[] = [];
    let releaseFirst: ((value: string) => void) | undefined;
    vi.mocked(generateHelpAnswer)
      .mockImplementationOnce(
        ({ signal }) =>
          new Promise((resolve) => {
            if (signal) streams.push(signal);
            releaseFirst = resolve;
          })
      )
      .mockImplementation(() => Promise.resolve('second answer'));

    const { result } = render();
    // The handle as a component captured it BEFORE the first run re-rendered:
    // its `streaming` guard is a closed-over `false`, so both calls get in.
    // That is the only way two runs overlap, and it is why `run` must abort the
    // controller it replaces instead of just overwriting the ref.
    const staleSend = result.current.send;

    await act(async () => {
      void staleSend('first question');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    await act(async () => {
      await staleSend('second question');
    });

    expect(streams[0]?.aborted).toBe(true);
    const assistant = () => result.current.turns.filter((turn) => turn.role === 'assistant');
    expect(assistant().map((turn) => turn.content)).toEqual(['second answer']);

    // The abandoned stream resolving late must not append a turn of its own.
    await act(async () => {
      releaseFirst?.('first answer');
      await Promise.resolve();
    });
    expect(assistant()).toHaveLength(1);
  });

  it('refuses a new question while one is in flight (single-flight)', async () => {
    vi.mocked(generateHelpAnswer).mockImplementationOnce(() => new Promise(() => {}));
    const { result, mock } = render();

    await act(async () => {
      void result.current.send('first question');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    // A FRESH handle, from a render where `streaming` is true: the guard inside
    // `run` is the only thing between the user and a second overlapping
    // question here — the Ask button is not even on screen.
    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.send('second question');
    });

    expect(answered).toBe(false);
    expect(mock.help.search).toHaveBeenCalledTimes(1);
    expect(result.current.turns.map((turn) => turn.content)).toEqual(['first question']);
  });

  it('aborts an in-flight stream on unmount', async () => {
    let abortSignal: AbortSignal | undefined;
    vi.mocked(generateHelpAnswer).mockImplementationOnce(
      ({ signal }) =>
        new Promise(() => {
          abortSignal = signal;
        })
    );

    const { result, unmount } = render();
    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    unmount();
    expect(abortSignal?.aborted).toBe(true);
  });

  it('does nothing for a blank question or while AI is unavailable', async () => {
    const mock = client();
    const { result } = renderHookWithClient(
      () => useHelpChat({ model: 'llama3:70b', canUse: false }),
      { client: mock }
    );

    await act(async () => {
      await result.current.send('a real question');
    });
    expect(mock.help.search).not.toHaveBeenCalled();

    const usable = render();
    await act(async () => {
      await usable.result.current.send('   ');
    });
    expect(usable.mock.help.search).not.toHaveBeenCalled();
    expect(usable.result.current.turns).toHaveLength(0);
  });
});
