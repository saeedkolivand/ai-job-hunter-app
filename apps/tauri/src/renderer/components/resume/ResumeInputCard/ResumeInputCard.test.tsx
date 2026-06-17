/**
 * ResumeInputCard — resting/expanded flow tests.
 *
 * Seam: vi.mock('./useResumeInput') with a module-level mutable object so each
 * test sets `expanded` / `activeDoc` / `review` BEFORE render. Child components
 * are stubbed to recognizable testids; @ajh/ui (Button / cn) and the real
 * resting/expanded branch logic under test are NOT mocked.
 */
import type React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── child-component stubs (not under test here) ───────────────────────────────

vi.mock('../ProfileUrlInput', () => ({ ProfileUrlInput: () => null }));
vi.mock('../ResumeReviewPanel', () => ({
  ResumeReviewPanel: () => <div data-testid="review" />,
}));
vi.mock('../SavedResumeMenu', () => ({ SavedResumeMenu: () => null }));
vi.mock('../UploadZone', () => ({ UploadZone: () => <div data-testid="upload-zone" /> }));

// ── useResumeInput stub ───────────────────────────────────────────────────────
// Module-level mutable object: set BEFORE each render, never after.

const mockSetExpanded = vi.fn();

type ActiveDoc = { id: string; title: string } | undefined;

interface StubHook {
  expanded: boolean;
  dragging: boolean;
  showSaved: boolean;
  showUrlInput: boolean;
  hasSaved: boolean;
  uploading: boolean;
  scanning: boolean;
  profileUrlValid: boolean;
  profileImportPending: boolean;
  selectedDocId: string | null;
  profileUrl: string;
  docs: unknown[];
  activeDoc: ActiveDoc;
  review: unknown;
  menuPos: { top: number; right: number };
  fileRef: React.RefObject<HTMLInputElement | null>;
  savedBtnRef: React.RefObject<HTMLButtonElement | null>;
  savedMenuRef: React.RefObject<HTMLDivElement | null>;
  setExpanded: typeof mockSetExpanded;
  setDragging: ReturnType<typeof vi.fn>;
  setShowUrlInput: ReturnType<typeof vi.fn>;
  setProfileUrl: ReturnType<typeof vi.fn>;
  openSavedMenu: ReturnType<typeof vi.fn>;
  handleSelectSaved: ReturnType<typeof vi.fn>;
  handleSetDefaultSaved: ReturnType<typeof vi.fn>;
  handleRemove: ReturnType<typeof vi.fn>;
  handleFileChange: ReturnType<typeof vi.fn>;
  handleSavePaste: ReturnType<typeof vi.fn>;
  handleProfileUrlSubmit: ReturnType<typeof vi.fn>;
  toggleUrlInput: ReturnType<typeof vi.fn>;
  clearReview: ReturnType<typeof vi.fn>;
}

const baseHook: StubHook = {
  expanded: false,
  dragging: false,
  showSaved: false,
  showUrlInput: false,
  hasSaved: false,
  uploading: false,
  scanning: false,
  profileUrlValid: false,
  profileImportPending: false,
  selectedDocId: null,
  profileUrl: '',
  docs: [],
  activeDoc: undefined,
  review: undefined,
  menuPos: { top: 0, right: 0 },
  fileRef: { current: null },
  savedBtnRef: { current: null },
  savedMenuRef: { current: null },
  setExpanded: mockSetExpanded,
  setDragging: vi.fn(),
  setShowUrlInput: vi.fn(),
  setProfileUrl: vi.fn(),
  openSavedMenu: vi.fn(),
  handleSelectSaved: vi.fn(),
  handleSetDefaultSaved: vi.fn(),
  handleRemove: vi.fn(),
  handleFileChange: vi.fn(),
  handleSavePaste: vi.fn(),
  handleProfileUrlSubmit: vi.fn(),
  toggleUrlInput: vi.fn(),
  clearReview: vi.fn(),
};

let stubbedHook: StubHook = baseHook;

vi.mock('./useResumeInput', () => ({
  useResumeInput: () => stubbedHook,
}));

// ── component under test ──────────────────────────────────────────────────────

import { ResumeInputCard } from './index';

const defaultProps = {
  value: '',
  onChange: vi.fn(),
} as const;

// ── tests ─────────────────────────────────────────────────────────────────────

describe('ResumeInputCard — resting / expanded flow', () => {
  it('RESTING: renders the active doc title chip and a Change control, no add options', () => {
    stubbedHook = {
      ...baseHook,
      expanded: false,
      activeDoc: { id: 'd1', title: 'My CV' },
    };
    render(<ResumeInputCard {...defaultProps} value="resume text" />);

    expect(screen.getByText('My CV')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /resumeInput\.change/i })).toBeInTheDocument();
    expect(screen.queryByTestId('upload-zone')).not.toBeInTheDocument();
  });

  it('clicking Change expands the card (setExpanded(true))', () => {
    mockSetExpanded.mockClear();
    stubbedHook = {
      ...baseHook,
      expanded: false,
      activeDoc: { id: 'd1', title: 'My CV' },
    };
    render(<ResumeInputCard {...defaultProps} value="resume text" />);

    fireEvent.click(screen.getByRole('button', { name: /resumeInput\.change/i }));

    expect(mockSetExpanded).toHaveBeenCalledWith(true);
  });

  it('EXPANDED: shows the upload zone (add options)', () => {
    stubbedHook = { ...baseHook, expanded: true };
    render(<ResumeInputCard {...defaultProps} />);

    expect(screen.getByTestId('upload-zone')).toBeInTheDocument();
  });

  it('renders the review panel only when review is present', () => {
    stubbedHook = { ...baseHook, expanded: true, review: undefined };
    const { rerender } = render(<ResumeInputCard {...defaultProps} />);
    expect(screen.queryByTestId('review')).not.toBeInTheDocument();

    stubbedHook = { ...baseHook, expanded: true, review: { reviewRequired: true } };
    rerender(<ResumeInputCard {...defaultProps} value="x" />);
    expect(screen.getByTestId('review')).toBeInTheDocument();
  });
});
