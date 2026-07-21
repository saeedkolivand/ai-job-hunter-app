/**
 * AgencyCompaniesPreferences — add / dedup / remove via the single-column
 * `useSetExtraAgencyCompanies` setter (ADR-029 §i).
 *
 * @ajh/ui + lucide + motion are stubbed; the service hook is a spy so the exact
 * next-list argument is assertable.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, unknown>) => (p ? `${k}:${Object.values(p).join(',')}` : k),
  }),
}));

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: ({ children, ...rest }: React.HTMLAttributes<HTMLDivElement>) => (
      <div {...rest}>{children}</div>
    ),
  },
}));

vi.mock('lucide-react', () => ({
  Building2: () => null,
  Plus: () => null,
  X: () => null,
}));

vi.mock('@ajh/ui', () => ({
  GlassCard: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Button: ({
    children,
    onClick,
    disabled,
    'aria-label': ariaLabel,
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
    'aria-label'?: string;
  }) => React.createElement('button', { onClick, disabled, 'aria-label': ariaLabel }, children),
  Input: ({
    value,
    onChange,
    onKeyDown,
    placeholder,
  }: {
    value?: string;
    onChange?: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
    placeholder?: string;
  }) => React.createElement('input', { value, onChange, onKeyDown, placeholder }),
}));

const prefs: { data: { extraAgencyCompanies?: string[] } | undefined } = { data: undefined };
const mockSetExtra = vi.fn();

vi.mock('@/services', () => ({
  useJobPreferences: () => prefs,
  useSetExtraAgencyCompanies: () => ({ mutate: mockSetExtra, isPending: false }),
}));

import { AgencyCompaniesPreferences } from './index';

beforeEach(() => {
  prefs.data = undefined;
  mockSetExtra.mockClear();
});

describe('AgencyCompaniesPreferences', () => {
  it('appends a trimmed company name on Enter', async () => {
    const user = userEvent.setup();
    prefs.data = { extraAgencyCompanies: ['Hays'] };
    render(<AgencyCompaniesPreferences />);

    await user.type(
      screen.getByPlaceholderText('settings.agencyCompanies.placeholder'),
      '  Randstad  {Enter}'
    );

    expect(mockSetExtra).toHaveBeenCalledWith(['Hays', 'Randstad']);
  });

  it('ignores a case-insensitive duplicate', async () => {
    const user = userEvent.setup();
    prefs.data = { extraAgencyCompanies: ['Hays'] };
    render(<AgencyCompaniesPreferences />);

    await user.type(
      screen.getByPlaceholderText('settings.agencyCompanies.placeholder'),
      'hays{Enter}'
    );

    expect(mockSetExtra).not.toHaveBeenCalled();
  });

  it('removes a company via its labelled remove button', async () => {
    const user = userEvent.setup();
    prefs.data = { extraAgencyCompanies: ['Hays', 'Adecco'] };
    render(<AgencyCompaniesPreferences />);

    await user.click(screen.getByLabelText('settings.agencyCompanies.remove:Hays'));

    expect(mockSetExtra).toHaveBeenCalledWith(['Adecco']);
  });

  it('shows the empty hint when no custom agencies are set', () => {
    render(<AgencyCompaniesPreferences />);
    expect(screen.getByText('settings.agencyCompanies.empty')).toBeInTheDocument();
  });

  it('does not call the setter before job preferences have loaded (pre-load guard)', async () => {
    // prefs.data stays undefined (beforeEach default) — the query hasn't
    // resolved, so adding would replace the saved list with just this entry.
    const user = userEvent.setup();
    render(<AgencyCompaniesPreferences />);

    await user.type(
      screen.getByPlaceholderText('settings.agencyCompanies.placeholder'),
      'Hays{Enter}'
    );

    expect(mockSetExtra).not.toHaveBeenCalled();
  });
});
