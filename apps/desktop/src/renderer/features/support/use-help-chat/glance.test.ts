/**
 * useHelpChat — the data glance: what of the user's own data leaves the machine
 * with a question, and when it is withheld.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  APPLICATIONS_HIT,
  ask,
  AUTOPILOT_HIT,
  dataReads,
  firstGlance,
  glanceAutopilotsSent,
  HIT,
  hybridSearch,
  renderChat,
  resetChatMocks,
} from './test-support';

vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('@ajh/prompts/generate', async (importOriginal) =>
  (await import('./test-mocks')).promptsMock(await importOriginal())
);

describe('useHelpChat — data glance', () => {
  beforeEach(resetChatMocks);

  it('builds the data glance from the user’s own counts, excluding untracked interactions', async () => {
    const { result } = renderChat();
    await ask(result, 'what have i done so far');

    const glance = firstGlance();
    expect(glance).toContain('Documents imported: 3');
    // Two `viewed`; the `dismissed` row is excluded by the tracked allowlist —
    // a dismissal is the opposite of tracking, so it must not inflate this.
    expect(glance).toContain('viewed 2');
    expect(glance).not.toContain('dismissed');
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).toContain('Autopilots configured: 2');
  });

  it('reads none of the user’s lists until a question is actually asked', async () => {
    const { result, mock } = renderChat();

    // Opening the Help page to read ONE entry must not read the user's
    // documents, interactions, applications or autopilots. Mounting the four
    // queries would have issued all four before a question existed.
    for (const read of dataReads(mock)) expect(read).not.toHaveBeenCalled();

    await ask(result, 'how do i export a pdf');
    for (const read of dataReads(mock)) expect(read).toHaveBeenCalledTimes(1);
  });

  it('withholds the recent-application NAMES unless the question retrieved an applications entry', async () => {
    const { result } = renderChat();
    await ask(result, 'how do i export a pdf');

    const glance = firstGlance();
    // Counts are always safe to send; the scraped job titles and company names
    // are not, and an export question is not made better by them.
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).not.toContain('Senior Engineer');
    expect(glance).not.toContain('Acme');
  });

  it('includes the recent-application names when an applications entry was retrieved', async () => {
    const { result } = renderChat('llama3:70b', { 'help.search': hybridSearch(APPLICATIONS_HIT) });
    await ask(result, 'which jobs have i applied to');

    expect(firstGlance()).toContain('Senior Engineer — Acme (applied)');
  });

  it('withholds the recent-application names when an applications entry is only a SECONDARY hit', async () => {
    // An export question, with the applications entry as the rank-2 maybe the
    // ranker kept behind it.
    const { result } = renderChat('llama3:70b', {
      'help.search': hybridSearch(HIT, APPLICATIONS_HIT),
    });
    await ask(result, 'how do i export a pdf');

    const glance = firstGlance();
    // The answer is written from the TOP entry, so a secondary hit must not
    // widen what leaves the machine — the counts still travel.
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).not.toContain('Senior Engineer');
    expect(glance).not.toContain('Acme');
  });

  it('withholds the autopilot NAMES unless the question retrieved an autopilot entry', async () => {
    const { result } = renderChat();
    await ask(result, 'how do i export a pdf');

    const glance = firstGlance();
    // The count is always safe; the user-typed names are not, and an export
    // question is not answered any better for having them.
    expect(glance).toContain('Autopilots configured: 2');
    expect(glance).not.toContain('Berlin React roles');
    expect(glance).not.toContain('Remote Rust');
  });

  it('includes the autopilot names when an autopilot entry was retrieved, and nothing else off the record', async () => {
    const { result } = renderChat('llama3:70b', { 'help.search': hybridSearch(AUTOPILOT_HIT) });
    await ask(result, 'how do i set up an autopilot');

    const glance = firstGlance();
    expect(glance).toContain('Berlin React roles — active, completed (12 found)');
    // No run yet → no run status, rather than an invented one.
    expect(glance).toContain('Remote Rust — paused (0 found)');
    // Four fields travel, so the résumé text on the same record does not.
    expect(glance).not.toContain('SECRET resume text');
  });

  it('withholds the autopilot names when an autopilot entry is only a SECONDARY hit', async () => {
    // The shape observed live: a LinkedIn-import question whose rank-2 hit was
    // `setUpAutopilot`. Gating on "any retrieved entry" sent the user's
    // autopilot names to the provider for a question the top entry answers.
    const { result } = renderChat('llama3:70b', {
      'help.search': hybridSearch(HIT, AUTOPILOT_HIT),
    });
    await ask(result, 'how do i import my linkedin profile');

    const glance = firstGlance();
    expect(glance).toContain('Autopilots configured: 2');
    expect(glance).not.toContain('Berlin React roles');
    expect(glance).not.toContain('Remote Rust');
  });

  it('sends at most the 10 autopilot names the glance renders', async () => {
    const { result } = renderChat('llama3:70b', {
      'help.search': hybridSearch(AUTOPILOT_HIT),
      'autopilot.list': vi.fn().mockResolvedValue(
        Array.from({ length: 12 }, (_, index) => ({
          _id: `ap${index}`,
          name: `Autopilot number ${index + 1}`,
          status: 'active',
          totalFound: 0,
        }))
      ),
    });
    await ask(result, 'how do i set up an autopilot');

    // The COUNT covers all 12 — that line is a number, not user-typed text.
    expect(firstGlance()).toContain('Autopilots configured: 12');
    // The NAMES stop where the prompt's `Autopilots:` list does. Asserted on
    // what the hook PASSED, not on the rendered glance: the prompt slices to 10
    // as well, so the string is identical either way and would prove nothing.
    const sent = glanceAutopilotsSent() ?? [];
    expect(sent.map((autopilot) => autopilot.name)).toEqual(
      Array.from({ length: 10 }, (_, index) => `Autopilot number ${index + 1}`)
    );
  });

  it('claims nothing about autopilots when that source could not be read', async () => {
    const { result } = renderChat('llama3:70b', {
      'help.search': hybridSearch(AUTOPILOT_HIT),
      'autopilot.list': vi.fn().mockRejectedValue(new Error('database is locked')),
    });
    await ask(result, 'how do i set up an autopilot');

    // The answer still lands — the glance is a garnish on a corpus answer.
    expect(result.current.error).toBeNull();
    expect(result.current.turns[1]?.role).toBe('assistant');

    const glance = firstGlance();
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

  it('still answers when ONE glance source fails, and omits only that source', async () => {
    // The glance is a garnish on an answer grounded in the help corpus, so a
    // single unreadable source must cost its own line and nothing else. Under
    // `Promise.all` this rejection failed the whole question.
    const { result } = renderChat('llama3:70b', {
      'applications.list': vi.fn().mockRejectedValue(new Error('database is locked')),
    });
    await ask(result, 'how do i export a pdf');

    // The answer landed: an assistant turn, no error.
    expect(result.current.error).toBeNull();
    expect(result.current.turns[1]?.role).toBe('assistant');
    expect(result.current.turns[1]?.content).toBe('Open the document and click Export.');

    const glance = firstGlance();
    // The failed source is ABSENT — not reported as "Applications tracked: 0",
    // which the model would state as fact about a user who has applications.
    expect(glance).not.toContain('Applications tracked');
    // The three that answered are all still there.
    expect(glance).toContain('Documents imported: 3');
    expect(glance).toContain('viewed 2');
    expect(glance).toContain('Autopilots configured: 2');
  });
});
