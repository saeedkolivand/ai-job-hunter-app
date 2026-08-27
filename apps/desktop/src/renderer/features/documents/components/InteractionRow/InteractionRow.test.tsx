/**
 * InteractionRow — `dismissed` badge (regression: a union member with no
 * `INTERACTION_TYPES` entry silently rendered the raw English word via the
 * forward-compat fallback, in every locale).
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  GlassCard: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Button: ({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) => (
    <button type="button" onClick={onClick}>
      {children}
    </button>
  ),
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

vi.mock('lucide-react', () => ({
  Activity: () => null,
  Bookmark: () => null,
  Building2: () => null,
  Clock: () => null,
  Eye: () => null,
  ExternalLink: () => null,
  FileText: () => null,
  Mail: () => null,
  MapPin: () => null,
  Send: () => null,
  Tag: () => null,
  X: () => null,
}));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '3 min ago',
}));

vi.mock('@/services/use-system', () => ({
  useOpenExternal: () => ({ mutate: vi.fn() }),
}));

import type { Interaction } from '@/features/documents/constants';

import { InteractionRow } from './index';

function makeRow(overrides: Partial<Interaction> = {}): Interaction {
  return {
    jobId: 'job-1',
    interactionType: 'dismissed',
    timestamp: 0,
    title: 'Engineer',
    company: 'Acme',
    url: 'https://example.com/job/1',
    source: 'autopilot',
    location: '',
    ...overrides,
  };
}

describe('InteractionRow — dismissed badge', () => {
  it('renders the localized "resumes.activity.dismissed" label, not the raw type string', () => {
    render(<InteractionRow row={makeRow()} />);

    expect(screen.getByText('resumes.activity.dismissed')).toBeInTheDocument();
    expect(screen.queryByText('dismissed')).not.toBeInTheDocument();
  });

  it('still falls back to the raw type string for a genuinely unknown/future type', () => {
    render(<InteractionRow row={makeRow({ interactionType: 'someFutureType' })} />);

    expect(screen.getByText('someFutureType')).toBeInTheDocument();
  });
});
