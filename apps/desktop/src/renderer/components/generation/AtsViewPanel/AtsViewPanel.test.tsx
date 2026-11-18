import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { AtsView } from '@ajh/shared/ipc';
import i18n from '@ajh/translations';

import { AtsViewPanel } from './index';

let mockResult: { data?: AtsView; isError?: boolean } = {};

vi.mock('@/services/use-ats-view', () => ({
  useAtsView: () => mockResult,
}));

const VIEW: AtsView = {
  contact: {
    name: { status: 'found', value: 'Jane Doe' },
    email: { status: 'found', value: 'jane@example.com' },
    phone: { status: 'suspicious', value: '+49 30', reason: 'wrapped' },
    location: { status: 'missing', reason: 'notFound' },
  },
  sections: [
    { kind: 'experience', heading: 'WORK EXPERIENCE', status: 'found' },
    { kind: 'skills', status: 'missing', reason: 'notFound' },
  ],
  roles: [
    {
      title: { status: 'found', value: 'Engineer' },
      company: { status: 'found', value: 'Acme' },
      start: { status: 'suspicious', value: "Jan '21", reason: 'unparseable' },
      end: { status: 'found', value: 'present' },
    },
  ],
  readingOrder: { status: 'found' },
  summary: { found: 7, suspicious: 2, missing: 2, total: 11 },
};

describe('AtsViewPanel', () => {
  beforeEach(() => {
    mockResult = {};
  });

  it('renders nothing until the view has loaded (a failed read stays silent)', () => {
    const { container } = render(<AtsViewPanel path="/cv.pdf" />);
    expect(container).toBeEmptyDOMElement();
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('renders nothing when the re-read failed, even though the query still holds the old view', () => {
    mockResult = { data: VIEW, isError: true };
    const { container } = render(<AtsViewPanel path="/cv.pdf" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('localizes the open-ended end date (present -> heute in German)', async () => {
    await i18n.changeLanguage('de');
    mockResult = { data: VIEW };
    render(<AtsViewPanel path="/cv.pdf" />);
    fireEvent.click(screen.getByRole('button', { name: /Das sieht ein ATS/ }));
    expect(screen.getByText('heute')).toBeInTheDocument();
    expect(screen.queryByText('present')).not.toBeInTheDocument();
  });

  it('collapses to the summary line and expands to the per-field readout', () => {
    mockResult = { data: VIEW };
    render(<AtsViewPanel path="/cv.pdf" />);
    const toggle = screen.getByRole('button', {
      name: 'What an ATS sees: 7 of 11 fields found',
    });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('Jane Doe')).not.toBeInTheDocument();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Jane Doe')).toBeInTheDocument();
    expect(screen.getByText('WORK EXPERIENCE')).toBeInTheDocument();
    expect(screen.getByText('present')).toBeInTheDocument();
  });

  it('says "In order" for a found reading order', () => {
    mockResult = { data: VIEW };
    render(<AtsViewPanel path="/cv.pdf" />);
    fireEvent.click(screen.getByRole('button', { name: /What an ATS sees/ }));
    expect(screen.getByText('In order')).toBeInTheDocument();
  });

  it('explains a reading order that could not be checked (no standard heading)', () => {
    mockResult = { data: { ...VIEW, readingOrder: { status: 'missing', reason: 'notFound' } } };
    render(<AtsViewPanel path="/cv.pdf" />);
    fireEvent.click(screen.getByRole('button', { name: /What an ATS sees/ }));
    expect(screen.queryByText('In order')).not.toBeInTheDocument();
    expect(screen.getByText(/could not be checked/)).toBeInTheDocument();
  });

  it('renders two sections with the same heading without a duplicate-key warning', () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const dup = {
      kind: 'other' as const,
      heading: 'HOBBIES',
      status: 'suspicious' as const,
      reason: 'nonstandardHeading' as const,
    };
    mockResult = { data: { ...VIEW, sections: [dup, dup] } };
    render(<AtsViewPanel path="/cv.pdf" />);
    fireEvent.click(screen.getByRole('button', { name: /What an ATS sees/ }));
    expect(screen.getAllByText('HOBBIES')).toHaveLength(2);
    expect(err).not.toHaveBeenCalled();
    err.mockRestore();
  });

  it('explains a suspicious field with its reason and a text status', () => {
    mockResult = { data: VIEW };
    render(<AtsViewPanel path="/cv.pdf" />);
    fireEvent.click(screen.getByRole('button', { name: /What an ATS sees/ }));

    expect(screen.getByText("Jan '21")).toBeInTheDocument();
    expect(screen.getByText(/format an ATS may not read/)).toBeInTheDocument();
    expect(screen.getByText('The phone number was split across two lines.')).toBeInTheDocument();
    // status is conveyed in text, not by the icon alone
    expect(screen.getAllByText('Unclear:').length).toBe(2);
    expect(screen.getAllByText('Missing:').length).toBe(2);
  });
});
