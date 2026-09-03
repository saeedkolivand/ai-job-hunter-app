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
});
