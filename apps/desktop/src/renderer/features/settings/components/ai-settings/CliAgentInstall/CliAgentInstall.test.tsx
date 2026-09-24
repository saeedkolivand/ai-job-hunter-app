/**
 * CliAgentInstall — the one-click install is offered only when there is a
 * command to run. An agent that isn't on npm (Cursor) gets the guide link only:
 * showing the button would send an empty command name and args to the
 * install mutation.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import { CliAgentInstall } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
  };
});

const agents = [
  {
    id: 'opencode',
    installCommandName: 'install-opencode',
    installArgs: ['install', '-g', '@opencode/cli'],
  },
  { id: 'cursor', installCommandName: '', installArgs: [] },
];
let npmAvailable = true;

vi.mock('@/services', () => ({
  useCliAgents: () => ({ data: { agents, npmAvailable } }),
  useInstallCliAgent: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

const renderFor = (provider: 'opencode' | 'cursor') =>
  render(
    <CliAgentInstall provider={provider} label={provider} onGuide={vi.fn()} onRecheck={vi.fn()} />
  );

describe('CliAgentInstall', () => {
  it('offers one-click install for an agent with an install command', () => {
    npmAvailable = true;
    renderFor('opencode');
    expect(screen.getByText('settings.cliInstall.install')).toBeInTheDocument();
  });

  it('shows only the guide for an agent without an install command', () => {
    npmAvailable = true;
    renderFor('cursor');
    expect(screen.queryByText('settings.cliInstall.install')).not.toBeInTheDocument();
    expect(screen.getByText('settings.cliInstall.guide')).toBeInTheDocument();
  });

  it('does not warn about npm for an agent that is not installed through npm', () => {
    npmAvailable = false;
    renderFor('cursor');
    expect(screen.queryByText('settings.cliInstall.npmMissing')).not.toBeInTheDocument();
  });
});
