/**
 * Shared mocks + fixtures for the ApplyByEmailTab suites.
 *
 * Heavy pieces (AI streaming, RewritePopover internals) are stubbed so the suites
 * stay fast and deterministic; the real component wiring is exercised directly.
 * This module registers every `vi.mock` (hoisted above its own imports), so a
 * suite must import the tab FROM HERE, never from `../ApplyByEmailTab`.
 */
import { beforeEach, type Mock, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { AiGenerationRecord, AiGenerationSaveResult, Application } from '@ajh/shared';

import { ApplyByEmailTab as TabUnderTest } from '../ApplyByEmailTab';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── ModelSelector hooks ───────────────────────────────────────────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => ({ canUse: true }),
  useSelectedProvider: () => 'ollama',
}));

// ── Document / application hooks — no real IPC ───────────────────────────────

vi.mock('@/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => 'doc-1',
}));

// Router — the needsResume CTA calls useNavigate(); a bare hook throws without a
// RouterProvider, so stub it and capture the navigation target.
export const navigateMock: Mock = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigateMock,
}));

// Mutable service returns (same idiom as generateEmailMock) so each case can
// drive the résumé text, contact profile, and URL-resolved job description
// independently.
export const documentTextMock = vi.fn<() => { data: string; isLoading: boolean }>();
const contactProfileMock = vi.fn<() => { data?: { fullName?: string } }>();
export const resolveJobUrlMock =
  vi.fn<
    (url: string, enabled?: boolean) => { data?: { description?: string }; isFetching: boolean }
  >();

// Controlled so the canonical-contact binding tests can assert the exact patch
// the recipient fields persist (`contactName`/`contactEmail`, never the
// deprecated `recipientName`/`recipientEmail` aliases).
export const updateApplicationMutate: Mock = vi.fn();

vi.mock('@/services', () => ({
  useDocuments: () => ({ isLoading: false }),
  useDocumentText: () => documentTextMock(),
  useUpdateApplication: () => ({ mutate: updateApplicationMutate }),
  useContactProfile: () => contactProfileMock(),
  useResolveJobUrl: (url: string, enabled?: boolean) => resolveJobUrlMock(url, enabled),
}));

// Persistence — the save mutation onto the per-job aiGenerations aggregate.
// Mirrors React Query's `mutate(vars, { onSuccess })` so a test can drive the
// in-band `{ error }` payload the Rust command actually returns.
type SaveResult = AiGenerationSaveResult;
export type SaveCallbacks = { onSuccess?: (d: SaveResult) => void; onError?: () => void };
export const saveResultMock = vi.fn<() => SaveResult>();
export const saveGenerationMock = vi.fn((_req: unknown, cbs?: SaveCallbacks) => {
  cbs?.onSuccess?.(saveResultMock());
});

vi.mock('@/services/use-ai-generations', () => ({
  useSaveAiGeneration: () => ({ mutate: saveGenerationMock }),
}));

// Deterministic: no recipient auto-fill from the (empty) job description.
vi.mock('../../../lib/extract-recipient', () => ({
  extractRecipient: () => ({ name: '', email: '' }),
}));

// ── generateApplicationEmail — resolves with a fixed draft ───────────────────

export const SUBJECT = 'Senior Engineer application';
export const BODY = 'Hello, I am interested in the role.';
export const EMAIL_RAW = `Subject: ${SUBJECT}\n\n${BODY}`;

export const generateEmailMock =
  vi.fn<(p: { onToken?: (tok: string) => void }) => Promise<string>>();

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

export const RewritePopoverStub = vi.fn(({ target, docType, onAccept, onClose }: PopoverProps) => (
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

import { makeApplication } from '@/features/applications/lib/test-fixtures';

export const makeApp = (overrides: Partial<Application> = {}) =>
  makeApplication({ jobDescription: 'We need a senior engineer.', ...overrides });

// ── fixtures ──────────────────────────────────────────────────────────────────

/** The tab under test — a wrapper, so the (mock-hoisted) import is read at render time. */
export const ApplyByEmailTab = (props: React.ComponentProps<typeof TabUnderTest>) => (
  <TabUnderTest {...props} />
);

export const NO_GENERATIONS: AiGenerationRecord[] = [];

/** Render the tab for `makeApp(overrides)` with the given saved generations (default none). */
export function renderTab(
  overrides: Partial<Application> = {},
  generations: AiGenerationRecord[] = NO_GENERATIONS
) {
  return render(
    <ApplyByEmailTab application={makeApp(overrides)} matchingGenerations={generations} />
  );
}

/** Click the (re)generate button and let the stream settle. */
export async function clickGenerate(name: 'generate' | 'regenerate' = 'generate') {
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: `applications.detail.email.${name}` }));
  });
}

beforeEach(() => {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
  RewritePopoverStub.mockClear();
  // `mockReset`, not `mockClear`: the rejection tests install implementations.
  updateApplicationMutate.mockReset();
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
  saveGenerationMock.mockClear();
  saveResultMock.mockReset();
  saveResultMock.mockReturnValue({ id: 'gen-1', success: true });
  window.getSelection()?.removeAllRanges();
});

/** A saved aggregate carrying a persisted email draft, as the store returns it. */
export function makeSavedGeneration(
  overrides: Partial<AiGenerationRecord> = {}
): AiGenerationRecord {
  return {
    id: 'gen-1',
    createdAt: 1000,
    candidateName: 'Jane',
    jobTitle: 'Engineer',
    companyName: 'Acme',
    resumeLanguage: 'de',
    jobAdLanguage: 'en',
    targetLanguage: 'en',
    mismatch: true,
    topRequirements: ['rust'],
    mode: 'ats',
    resumeText: 'SAVED RESUME',
    coverLetterText: 'SAVED COVER',
    jobAd: 'JD',
    jobUrl: 'https://acme.com/job/1',
    board: 'linkedin',
    applicationAnswers: [],
    companyBrief: '',
    interviewQuestions: [],
    applicationId: 'app-1',
    ...overrides,
  };
}

/** Render the tab and run one generation so the editable draft is present. */
export async function renderWithDraft() {
  renderTab();
  await clickGenerate();
  // The draft (subject + body) is now rendered.
  await screen.findByText(BODY);
}

/** Selects `substring` (occurs once in `el`'s text) inside `el`, mirroring a drag. */
export function selectSubstring(el: HTMLElement, text: string, substring: string) {
  const textNode = el.firstChild as Text;
  const start = text.indexOf(substring);
  const range = document.createRange();
  range.setStart(textNode, start);
  range.setEnd(textNode, start + substring.length);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
}

/** Open the body Rewrite popover and accept its (stubbed) replacement. */
export async function rewriteBodyAndAccept() {
  fireEvent.click(
    screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
  );
  await act(async () => {
    fireEvent.click(screen.getByTestId('popover-accept'));
  });
}
