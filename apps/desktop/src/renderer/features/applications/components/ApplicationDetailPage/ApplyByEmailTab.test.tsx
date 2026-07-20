/**
 * ApplyByEmailTab — subject-only copy + select-to-rewrite integration tests.
 *
 * Covers:
 *  - Feature #2: the subject copy button writes JUST the subject (no "Subject:"
 *    prefix, no body) and is independent of the whole-email Copy button.
 *  - Feature #5: select-to-rewrite wiring — clicking Rewrite opens RewritePopover
 *    with docType='email' and the selected span (or the whole field when nothing
 *    is selected); accepting splices the replacement back into the mutable draft.
 *
 * Heavy pieces (AI streaming, RewritePopover internals) are stubbed so the test
 * stays fast and deterministic; the real component wiring is exercised directly.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { AiGenerationRecord, Application } from '@ajh/shared';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── ModelSelector hooks ───────────────────────────────────────────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => ({ canUse: true }),
}));

// ── Document / application hooks — no real IPC ───────────────────────────────

vi.mock('@/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => 'doc-1',
}));

// Router — the needsResume CTA calls useNavigate(); a bare hook throws without a
// RouterProvider, so stub it and capture the navigation target.
const navigateMock = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigateMock,
}));

// Mutable service returns (same idiom as generateEmailMock) so each case can
// drive the résumé text, contact profile, and URL-resolved job description
// independently.
const documentTextMock = vi.fn<() => { data: string; isLoading: boolean }>();
const contactProfileMock = vi.fn<() => { data?: { fullName?: string } }>();
const resolveJobUrlMock =
  vi.fn<
    (url: string, enabled?: boolean) => { data?: { description?: string }; isFetching: boolean }
  >();

vi.mock('@/services', () => ({
  useDocuments: () => ({ isLoading: false }),
  useDocumentText: () => documentTextMock(),
  useUpdateApplication: () => ({ mutate: vi.fn() }),
  useContactProfile: () => contactProfileMock(),
  useResolveJobUrl: (url: string, enabled?: boolean) => resolveJobUrlMock(url, enabled),
}));

// Deterministic: no recipient auto-fill from the (empty) job description.
vi.mock('../../lib/extract-recipient', () => ({
  extractRecipient: () => ({ name: '', email: '' }),
}));

// ── generateApplicationEmail — resolves with a fixed draft ───────────────────

const SUBJECT = 'Senior Engineer application';
const BODY = 'Hello, I am interested in the role.';
const EMAIL_RAW = `Subject: ${SUBJECT}\n\n${BODY}`;

const generateEmailMock = vi.fn<(p: { onToken?: (tok: string) => void }) => Promise<string>>();

vi.mock('@/lib/generate', () => ({
  generateApplicationEmail: (p: { onToken?: (tok: string) => void }) => generateEmailMock(p),
}));

// ── RewritePopover stub — drives onAccept / onClose without AI streaming ─────

const REWRITE_RESULT = 'REWRITTEN';

type PopoverProps = {
  target: { selection: string; before: string; after: string };
  docType: string;
  model: string;
  locale?: string;
  onAccept: (text: string) => void;
  onClose: () => void;
};

const RewritePopoverStub = vi.fn(({ target, docType, onAccept, onClose }: PopoverProps) => (
  <div data-testid="rewrite-popover" data-doc-type={docType} data-selection={target.selection}>
    <div
      role="button"
      tabIndex={0}
      onClick={() => onAccept(REWRITE_RESULT)}
      onKeyDown={() => onAccept(REWRITE_RESULT)}
      data-testid="popover-accept"
    >
      accept
    </div>
    <div
      role="button"
      tabIndex={0}
      onClick={onClose}
      onKeyDown={onClose}
      data-testid="popover-close"
    >
      cancel
    </div>
  </div>
));

vi.mock('@/components/generation/EditableOutput/RewritePopover', () => ({
  RewritePopover: (props: PopoverProps) => RewritePopoverStub(props),
}));

// ── component under test (imported AFTER mocks) ──────────────────────────────

import { ApplyByEmailTab } from './ApplyByEmailTab';

// ── fixtures ──────────────────────────────────────────────────────────────────

function makeApp(overrides: Partial<Application> = {}): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: 1000,
    updatedAt: 1000,
    jobUrl: 'https://acme.com/job/1',
    board: 'linkedin',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    jobDescription: 'We need a senior engineer.',
    jobSummary: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

const NO_GENERATIONS: AiGenerationRecord[] = [];

beforeEach(() => {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
  RewritePopoverStub.mockClear();
  generateEmailMock.mockReset();
  generateEmailMock.mockImplementation(async (p) => {
    p.onToken?.(EMAIL_RAW);
    return EMAIL_RAW;
  });
  navigateMock.mockClear();
  documentTextMock.mockReset();
  documentTextMock.mockReturnValue({ data: 'My résumé text.', isLoading: false });
  contactProfileMock.mockReset();
  contactProfileMock.mockReturnValue({ data: { fullName: 'Jane Applicant' } });
  resolveJobUrlMock.mockReset();
  resolveJobUrlMock.mockReturnValue({ data: undefined, isFetching: false });
  window.getSelection()?.removeAllRanges();
});

/** Render the tab and run one generation so the editable draft is present. */
async function renderWithDraft() {
  render(<ApplyByEmailTab application={makeApp()} matchingGenerations={NO_GENERATIONS} />);
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.generate' }));
  });
  // The draft (subject + body) is now rendered.
  await screen.findByText(BODY);
}

/** Selects `substring` (occurs once in `el`'s text) inside `el`, mirroring a drag. */
function selectSubstring(el: HTMLElement, text: string, substring: string) {
  const textNode = el.firstChild as Text;
  const start = text.indexOf(substring);
  const range = document.createRange();
  range.setStart(textNode, start);
  range.setEnd(textNode, start + substring.length);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
}

// ── Feature #2 — subject-only copy ────────────────────────────────────────────

describe('ApplyByEmailTab — subject-only copy', () => {
  it('writes JUST the subject to the clipboard (no "Subject:" prefix, no body)', async () => {
    await renderWithDraft();

    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copySubject' }));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(SUBJECT);
    });
    // It must NOT have written the whole "Subject: …\n\n<body>" blob.
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(EMAIL_RAW);
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(
      `Subject: ${SUBJECT}\n\n${BODY}`
    );
  });

  it('the whole-email Copy button still writes the "Subject: …" + body blob', async () => {
    await renderWithDraft();

    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copy' }));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(`Subject: ${SUBJECT}\n\n${BODY}`);
    });
  });
});

// ── Feature #5 — select-to-rewrite ────────────────────────────────────────────

describe('ApplyByEmailTab — select-to-rewrite', () => {
  it('opens the popover with docType="email" and the full body when nothing is selected', async () => {
    await renderWithDraft();

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );

    const popover = screen.getByTestId('rewrite-popover');
    expect(popover.getAttribute('data-doc-type')).toBe('email');
    expect(popover.getAttribute('data-selection')).toBe(BODY);
  });

  it('targets only the selected substring of the body', async () => {
    await renderWithDraft();
    selectSubstring(screen.getByText(BODY), BODY, 'interested');

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );

    expect(screen.getByTestId('rewrite-popover').getAttribute('data-selection')).toBe('interested');
  });

  it('accepting a rewrite of a selected substring splices it back into the body', async () => {
    await renderWithDraft();
    selectSubstring(screen.getByText(BODY), BODY, 'interested');

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept')); // emits REWRITE_RESULT
    });

    // 'interested' → 'REWRITTEN', rest of the body untouched.
    expect(screen.getByText('Hello, I am REWRITTEN in the role.')).toBeTruthy();
    // Popover closes after accept.
    expect(screen.queryByTestId('rewrite-popover')).toBeNull();
  });

  it('accepting a subject rewrite splices into the subject, leaving the body untouched', async () => {
    await renderWithDraft();
    selectSubstring(screen.getByText(SUBJECT), SUBJECT, 'Senior');

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteSubjectAriaLabel' })
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });

    // 'Senior' → 'REWRITTEN' in the subject; body unchanged.
    expect(screen.getByText('REWRITTEN Engineer application')).toBeTruthy();
    expect(screen.getByText(BODY)).toBeTruthy();
  });

  it('closing the popover leaves the draft unchanged', async () => {
    await renderWithDraft();

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );
    expect(screen.getByTestId('rewrite-popover')).toBeTruthy();

    fireEvent.click(screen.getByTestId('popover-close'));

    expect(screen.queryByTestId('rewrite-popover')).toBeNull();
    expect(screen.getByText(BODY)).toBeTruthy();
  });

  it('passes the document model + locale to the popover', async () => {
    await renderWithDraft();

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );

    expect(RewritePopoverStub).toHaveBeenCalledWith(
      expect.objectContaining({ model: 'test-model', docType: 'email', locale: 'en' })
    );
  });

  // Both outputs read the mutable post-rewrite draft — not the frozen generation.
  it('the whole-email Copy and mailto reflect a rewrite of the draft', async () => {
    const openSpy = vi.spyOn(window, 'open').mockReturnValue(null);
    render(
      <ApplyByEmailTab
        application={makeApp({ recipientEmail: 'hr@acme.com' })}
        matchingGenerations={NO_GENERATIONS}
      />
    );
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.generate' }));
    });
    await screen.findByText(BODY);

    // Rewrite 'interested' → 'REWRITTEN', mutating the draft body.
    selectSubstring(screen.getByText(BODY), BODY, 'interested');
    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });

    const rewrittenBody = 'Hello, I am REWRITTEN in the role.';

    // Whole-email Copy writes the post-rewrite blob (not the original BODY).
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copy' }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        `Subject: ${SUBJECT}\n\n${rewrittenBody}`
      );
    });

    // mailto encodes the post-rewrite body too.
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.openMailto' }));
    expect(openSpy).toHaveBeenCalledWith(
      expect.stringContaining(encodeURIComponent(rewrittenBody)),
      '_blank'
    );
    openSpy.mockRestore();
  });
});

// ── standalone generation (no prior document generation) ─────────────────────

describe('ApplyByEmailTab — standalone generation', () => {
  /** Render, click Generate, and wait for the streamed draft to settle. */
  async function generate(application: Application) {
    render(<ApplyByEmailTab application={application} matchingGenerations={NO_GENERATIONS} />);
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.generate' }));
    });
    await screen.findByText(BODY);
  }

  it('builds meta from the contact profile + application when there is no saved generation', async () => {
    await generate(makeApp());

    expect(generateEmailMock).toHaveBeenCalledWith(
      expect.objectContaining({
        meta: expect.objectContaining({
          candidateName: 'Jane Applicant',
          companyName: 'Acme',
          jobTitle: 'Engineer',
        }),
      })
    );
    // The persisted JD is present, so the URL resolver is disabled.
    expect(resolveJobUrlMock).toHaveBeenCalledWith('https://acme.com/job/1', false);
  });

  it('resolves the job description from the URL when the application has none', async () => {
    const RESOLVED = 'Resolved job description fetched from the posting URL.';
    resolveJobUrlMock.mockReturnValue({ data: { description: RESOLVED }, isFetching: false });

    render(
      <ApplyByEmailTab
        application={makeApp({ jobDescription: '' })}
        matchingGenerations={NO_GENERATIONS}
      />
    );

    // The resolver is enabled ONLY because the JD is empty.
    expect(resolveJobUrlMock).toHaveBeenCalledWith('https://acme.com/job/1', true);

    const generateBtn = screen.getByRole('button', {
      name: 'applications.detail.email.generate',
    });
    expect(generateBtn).toBeEnabled();

    await act(async () => {
      fireEvent.click(generateBtn);
    });
    await screen.findByText(BODY);

    expect(generateEmailMock).toHaveBeenCalledWith(expect.objectContaining({ jobAd: RESOLVED }));
  });

  it('detects the target language from a German job description (real detectLanguage)', async () => {
    const germanJd =
      'Wir suchen einen erfahrenen Softwareentwickler für unser Team in München. ' +
      'Sie arbeiten an spannenden Projekten und stimmen sich eng mit dem Produktteam ab.';

    await generate(makeApp({ jobDescription: germanJd }));

    expect(generateEmailMock).toHaveBeenCalledWith(
      expect.objectContaining({ meta: expect.objectContaining({ targetLanguage: 'de' }) })
    );
  });

  it('disables Generate and shows the needsResume empty state + CTA when no résumé exists', () => {
    documentTextMock.mockReturnValue({ data: '', isLoading: false });

    render(<ApplyByEmailTab application={makeApp()} matchingGenerations={NO_GENERATIONS} />);

    const generateBtn = screen.getByRole('button', {
      name: 'applications.detail.email.generate',
    });
    expect(generateBtn).toBeDisabled();

    expect(screen.getByText('applications.detail.email.needsResume')).toBeTruthy();

    const cta = screen.getByRole('button', { name: 'applications.detail.email.addResume' });
    fireEvent.click(cta);
    expect(navigateMock).toHaveBeenCalledWith({ to: '/documents' });
  });
});
