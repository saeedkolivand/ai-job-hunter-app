/**
 * The agent-CLI card. Nothing between the component and the backend is
 * stubbed: the real `useAgentCliInfo` runs against a mock `AppClient` and a
 * real `QueryClient` (the `DeveloperPreferences.test` pattern), so the pending
 * and `null` states are the hook's real states rather than literals a stub was
 * told to return.
 *
 * `@ajh/translations` is deliberately NOT mocked either — the assertions are on
 * the shipped English copy, which is also what makes a missing key visible.
 *
 * What lives here rather than in `agent-cli-snippets.test.ts`: that the card
 * puts the RIGHT snippet on the clipboard for the CURRENT tier. The builders'
 * own quoting is pinned there.
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';
import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

const { navigate } = vi.hoisted(() => ({ navigate: vi.fn() }));

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  useRouter: () => ({ navigate }),
}));

// Component import deferred until after the mock above is hoisted.
import { AgentCliSection } from './index';

/** A real install path: it has a space, so every snippet has to quote it. */
const EXE = 'C:\\Program Files\\AI Job Hunter\\ajh-tauri.exe';

function renderCard(agentCliInfo = vi.fn().mockResolvedValue({ exePath: EXE })) {
  const client = createMockClient({ 'system.agentCliInfo': agentCliInfo });
  const queryClient = makeQueryClient();

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <AppClientProvider client={client}>
          <NotificationProvider>{children}</NotificationProvider>
        </AppClientProvider>
      </QueryClientProvider>
    );
  }

  return render(<AgentCliSection />, { wrapper: Wrapper });
}

const claudeSnippet = () => screen.getByTestId(TEST_IDS.settings.agentCliClaudeSnippet).textContent;
const codexSnippet = () => screen.getByTestId(TEST_IDS.settings.agentCliCodexSnippet).textContent;

const writeText = () => vi.mocked(navigator.clipboard.writeText);

beforeEach(() => {
  navigate.mockClear();
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

describe('AgentCliSection', () => {
  it('renders the resolved path and both registration snippets', async () => {
    renderCard();

    // The card's own id — what an e2e selector anchors on.
    expect(screen.getByTestId(TEST_IDS.settings.agentCliSection)).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    expect(claudeSnippet()).toBe(`claude mcp add --scope user ai-job-hunter -- "${EXE}" agent mcp`);
    expect(codexSnippet()).toBe(
      `[mcp_servers.ai-job-hunter]\ncommand = '${EXE}'\nargs = ["agent", "mcp"]`
    );
  });

  it('shows a skeleton, not an empty command, while the path is still being read', () => {
    // A query that never settles is the real pending state.
    renderCard(vi.fn().mockImplementation(() => new Promise(() => {})));

    expect(screen.queryByTestId(TEST_IDS.settings.agentCliPath)).toBeNull();
    expect(screen.queryByTestId(TEST_IDS.settings.agentCliClaudeSnippet)).toBeNull();
    expect(screen.queryByTestId(TEST_IDS.settings.agentCliCodexSnippet)).toBeNull();
  });

  it('says where to find the path when the shell could not resolve it, and shows no snippets', async () => {
    renderCard(vi.fn().mockResolvedValue({ exePath: null }));

    await waitFor(() => expect(screen.getByText(/could not be resolved/i)).toBeInTheDocument());
    // A half-built command is worse than none — a user would copy it and
    // register a server that never starts.
    expect(screen.queryByTestId(TEST_IDS.settings.agentCliClaudeSnippet)).toBeNull();
    expect(screen.queryByTestId(TEST_IDS.settings.agentCliCodexSnippet)).toBeNull();
    expect(screen.queryByTestId(TEST_IDS.settings.agentCliCopyPath)).toBeNull();
  });

  it('copies the path, the Claude command and the Codex block VERBATIM', async () => {
    renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    // `fireEvent`, not `userEvent`: `userEvent.setup()` installs its OWN
    // clipboard stub over `navigator.clipboard`, so the spy asserted on here
    // would never be the one the component called.
    fireEvent.click(screen.getByTestId(TEST_IDS.settings.agentCliCopyPath));
    await waitFor(() => expect(writeText()).toHaveBeenCalledWith(EXE));

    fireEvent.click(screen.getByRole('button', { name: 'Copy command' }));
    await waitFor(() => expect(writeText()).toHaveBeenCalledWith(claudeSnippet()));

    fireEvent.click(screen.getByRole('button', { name: 'Copy config' }));
    await waitFor(() => expect(writeText()).toHaveBeenCalledWith(codexSnippet()));

    expect(await screen.findAllByText('Copied to the clipboard.')).not.toHaveLength(0);
  });

  it('surfaces a clipboard failure instead of claiming a copy that did not happen', async () => {
    renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    writeText().mockRejectedValueOnce(new Error('denied'));

    fireEvent.click(screen.getByTestId(TEST_IDS.settings.agentCliCopyPath));

    expect(await screen.findByText('Could not copy to the clipboard.')).toBeInTheDocument();
    expect(screen.queryByText('Copied to the clipboard.')).toBeNull();
  });

  it('switching the tier rewrites BOTH snippets and the tier description', async () => {
    renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    const tier = within(screen.getByTestId(TEST_IDS.settings.agentCliTier));

    fireEvent.click(tier.getByRole('radio', { name: 'Irreversible' }));

    // Claude gets a distinct SERVER NAME per tier; Codex keeps one table name
    // and moves the flag into args. Both change — a card that only rewrote one
    // would hand out a mismatched pair.
    expect(claudeSnippet()).toBe(
      `claude mcp add --scope user ai-job-hunter-unrestricted -- "${EXE}" agent mcp --allow-irreversible`
    );
    expect(codexSnippet()).toBe(
      `[mcp_servers.ai-job-hunter]\ncommand = '${EXE}'\nargs = ["agent", "mcp", "--allow-irreversible"]`
    );
    expect(screen.getByText(/spend AI budget and delete data/i)).toBeInTheDocument();

    fireEvent.click(tier.getByRole('radio', { name: 'Reversible' }));
    expect(claudeSnippet()).toBe(
      `claude mcp add --scope user ai-job-hunter-write -- "${EXE}" agent mcp --allow-reversible`
    );
    expect(codexSnippet()).toContain('"--allow-reversible"');
  });

  it('links to Help & Support in-app rather than out to a browser', async () => {
    renderCard();

    fireEvent.click(screen.getByRole('button', { name: /Help & Support/i }));

    expect(navigate).toHaveBeenCalledWith({ to: '/support' });
  });

  it('renders the house error state with a retry when the read FAILED', async () => {
    const agentCliInfo = vi
      .fn()
      .mockRejectedValueOnce(new Error('ipc failed'))
      .mockResolvedValue({ exePath: EXE });
    renderCard(agentCliInfo);

    // A failed read and a path that resolved to `null` are different
    // situations: only one of them is worth a retry, and the "look in
    // agent.json" note is a lie about the other.
    const alert = await screen.findByRole('alert');
    expect(within(alert).getByText(/Could not read the binary path/i)).toBeInTheDocument();
    expect(screen.queryByText(/could not be resolved/i)).toBeNull();

    fireEvent.click(within(alert).getByRole('button'));

    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    expect(agentCliInfo).toHaveBeenCalledTimes(2);
  });

  it('hides the tier picker when there is no path — it would rewrite nothing', async () => {
    renderCard(vi.fn().mockResolvedValue({ exePath: null }));

    await waitFor(() => expect(screen.getByText(/could not be resolved/i)).toBeInTheDocument());
    expect(screen.queryByTestId(TEST_IDS.settings.agentCliTier)).toBeNull();
  });

  it('says the tier choice only rewrites the commands, and never promises a prompt', async () => {
    renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    expect(screen.getByText(/only rewrites the commands below/i)).toBeInTheDocument();

    fireEvent.click(
      within(screen.getByTestId(TEST_IDS.settings.agentCliTier)).getByRole('radio', {
        name: 'Irreversible',
      })
    );

    // The app has no confirmation prompt for these calls — the gate is a
    // proof-of-read step the AGENT performs. Copy promising a dialog that
    // never appears is worse than no copy at all.
    expect(screen.getByText(/proof-of-read step/i)).toBeInTheDocument();
    expect(screen.queryByText(/asks you to confirm/i)).toBeNull();
  });

  it('labels the path input with a real <label> only when that input exists', async () => {
    const { unmount } = renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );
    expect(screen.getByLabelText('Path to the binary')).toBe(
      screen.getByTestId(TEST_IDS.settings.agentCliPath)
    );
    unmount();

    // With no input rendered, a `<label htmlFor>` would point at an id that is
    // not in the document — a broken reference rather than a hidden one.
    renderCard(vi.fn().mockResolvedValue({ exePath: null }));
    await waitFor(() => expect(screen.getByText(/could not be resolved/i)).toBeInTheDocument());
    expect(screen.getByText('Path to the binary').tagName).toBe('SPAN');
  });

  it('wraps each snippet instead of hiding the privilege flag in a scroll region', async () => {
    renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );

    const blocks = [
      [TEST_IDS.settings.agentCliClaudeSnippet, 'Claude Code'],
      [TEST_IDS.settings.agentCliCodexSnippet, 'Codex (~/.codex/config.toml)'],
    ] as const;

    for (const [testId, name] of blocks) {
      const block = screen.getByTestId(testId);
      // jsdom lays nothing out, so what is pinned here are the properties that
      // DECIDE the outcome: a horizontally-scrolling `<pre>` cannot be reached
      // by keyboard on WebKit and clipped `--allow-irreversible` — the word
      // that says what is being granted — off the right edge at every width.
      expect(block.className).toContain('whitespace-pre-wrap');
      expect(block.className).toContain('break-all');
      expect(block.className).not.toContain('overflow-x-auto');
      // By ROLE and name, not `toHaveAccessibleName`: the name calculation
      // runs happily on a roleless element, but assistive tech never exposes
      // it there — so only the role query can tell the two apart.
      expect(screen.getByRole('group', { name })).toBe(block);
    }
  });

  it('lets the path row and the tier control wrap at the narrow settings column', async () => {
    renderCard();
    await waitFor(() =>
      expect(screen.getByTestId(TEST_IDS.settings.agentCliPath)).toHaveValue(EXE)
    );

    // The failing case is the 900px minimum window on the `large` text scale,
    // where this column is ~316px. Again unmeasurable in jsdom, so the
    // assertions are on the two properties that decide it: a flex Input
    // refuses to shrink below its intrinsic width without `min-w-0`, and
    // beside a `shrink-0` button that is what pushed the row past the card.
    const input = screen.getByTestId(TEST_IDS.settings.agentCliPath);
    expect(input.className).toContain('min-w-0');
    expect(input.parentElement?.className).toContain('flex-wrap');

    // Same for the tier control: its segments are `whitespace-nowrap`, so the
    // group itself has to be allowed to break onto a second row.
    const group = within(screen.getByTestId(TEST_IDS.settings.agentCliTier)).getByRole(
      'radiogroup'
    );
    expect(group.className).toContain('flex-wrap');
    expect(group.className).toContain('max-w-full');
  });
});
