/**
 * ResumeInputCard — segmented-control active-state tests.
 *
 * Seam: vi.mock('./useResumeInput') with a module-level mutable object so each
 * test sets inputMode BEFORE render. Child components are stubbed to nulls;
 * @ajh/ui (Button / cn) and the real className logic under test are NOT mocked.
 *
 * Three assertion groups:
 *  1. Upload active  → Upload button has active classes; Paste does not.
 *  2. Click Paste    → setInputMode vi.fn() called with 'paste'.
 *  3. Paste active   → Upload button has inactive classes; Paste has active classes.
 */
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── child-component stubs (not under test here) ───────────────────────────────

vi.mock('../ProfileUrlInput', () => ({ ProfileUrlInput: () => null }));
vi.mock('../ResumeReviewPanel', () => ({ ResumeReviewPanel: () => null }));
vi.mock('../SaveActions', () => ({ SaveActions: () => null }));
vi.mock('../SavedResumeMenu', () => ({ SavedResumeMenu: () => null }));
vi.mock('../UploadZone', () => ({ UploadZone: () => null }));

// ── useResumeInput stub ───────────────────────────────────────────────────────
// Module-level mutable object: set BEFORE each render, never after.

const mockSetInputMode = vi.fn();

let stubbedHook: {
  inputMode: 'upload' | 'paste';
  expanded: boolean;
  dragging: boolean;
  showSaved: boolean;
  showUrlInput: boolean;
  saving: boolean;
  hasSaved: boolean;
  profileUrlValid: boolean;
  profileImportPending: boolean;
  lastUploadedFile: File | null;
  selectedDocId: string | null;
  profileUrl: string;
  docs: unknown[];
  triggerDoc: undefined;
  review: undefined;
  menuPos: { top: number; right: number };
  fileRef: React.RefObject<HTMLInputElement | null>;
  savedBtnRef: React.RefObject<HTMLButtonElement | null>;
  savedMenuRef: React.RefObject<HTMLDivElement | null>;
  setInputMode: typeof mockSetInputMode;
  setExpanded: ReturnType<typeof vi.fn>;
  setDragging: ReturnType<typeof vi.fn>;
  setShowUrlInput: ReturnType<typeof vi.fn>;
  setProfileUrl: ReturnType<typeof vi.fn>;
  openSavedMenu: ReturnType<typeof vi.fn>;
  handleSelectSaved: ReturnType<typeof vi.fn>;
  handleSetDefaultSaved: ReturnType<typeof vi.fn>;
  handleSaveToLibrary: ReturnType<typeof vi.fn>;
  handleFileChange: ReturnType<typeof vi.fn>;
  handleProfileUrlSubmit: ReturnType<typeof vi.fn>;
  toggleUrlInput: ReturnType<typeof vi.fn>;
  clearReview: ReturnType<typeof vi.fn>;
} = {
  inputMode: 'upload',
  expanded: true,
  dragging: false,
  showSaved: false,
  showUrlInput: false,
  saving: false,
  hasSaved: false,
  profileUrlValid: false,
  profileImportPending: false,
  lastUploadedFile: null,
  selectedDocId: null,
  profileUrl: '',
  docs: [],
  triggerDoc: undefined,
  review: undefined,
  menuPos: { top: 0, right: 0 },
  fileRef: { current: null },
  savedBtnRef: { current: null },
  savedMenuRef: { current: null },
  setInputMode: mockSetInputMode,
  setExpanded: vi.fn(),
  setDragging: vi.fn(),
  setShowUrlInput: vi.fn(),
  setProfileUrl: vi.fn(),
  openSavedMenu: vi.fn(),
  handleSelectSaved: vi.fn(),
  handleSetDefaultSaved: vi.fn(),
  handleSaveToLibrary: vi.fn(),
  handleFileChange: vi.fn(),
  handleProfileUrlSubmit: vi.fn(),
  toggleUrlInput: vi.fn(),
  clearReview: vi.fn(),
};

vi.mock('./useResumeInput', () => ({
  useResumeInput: () => stubbedHook,
}));

// ── component under test ──────────────────────────────────────────────────────

import { ResumeInputCard } from './index';

// ── default props ─────────────────────────────────────────────────────────────

const defaultProps = {
  value: '',
  onChange: vi.fn(),
  onUpload: vi.fn(() => Promise.resolve()),
  uploading: false,
} as const;

// ── tests ─────────────────────────────────────────────────────────────────────

describe('ResumeInputCard — segmented control active state', () => {
  it('Upload button carries active classes when inputMode is upload', () => {
    stubbedHook = { ...stubbedHook, inputMode: 'upload' };
    render(<ResumeInputCard {...defaultProps} />);

    const upload = screen.getByRole('button', { name: /resumeInput\.modeUpload/i });
    const paste = screen.getByRole('button', { name: /resumeInput\.modePaste/i });

    expect(upload).toHaveClass('bg-brand-gradient');
    expect(upload).toHaveClass('text-brand-foreground');
    expect(paste).not.toHaveClass('bg-brand-gradient');
  });

  it('clicking the Paste button calls setInputMode with paste', () => {
    mockSetInputMode.mockClear();
    stubbedHook = { ...stubbedHook, inputMode: 'upload' };
    render(<ResumeInputCard {...defaultProps} />);

    const paste = screen.getByRole('button', { name: /resumeInput\.modePaste/i });
    fireEvent.click(paste);

    expect(mockSetInputMode).toHaveBeenCalledWith('paste');
  });

  it('Upload button carries inactive classes and Paste button carries active classes when inputMode is paste', () => {
    stubbedHook = { ...stubbedHook, inputMode: 'paste' };
    render(<ResumeInputCard {...defaultProps} />);

    const upload = screen.getByRole('button', { name: /resumeInput\.modeUpload/i });
    const paste = screen.getByRole('button', { name: /resumeInput\.modePaste/i });

    expect(upload).toHaveClass('text-brand-soft/60');
    expect(upload).not.toHaveClass('bg-brand-gradient');
    expect(paste).toHaveClass('bg-white/[0.10]');
    expect(paste).toHaveClass('text-foreground/90');
  });
});
