import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { OutputPanelDone } from './index';

// OutputPanelDone always mounts ExportModal (gated by its own `open` prop, not a
// conditional render), and ExportModal now calls useNotification() to surface a
// rejected export — stub it so this panel-wiring suite doesn't need a real
// NotificationProvider tree.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => ({
      open: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

// Stub the real-PDF preview (#24) — it renders the export via IPC, out of scope
// for this panel's preview/edit wiring test (covered in PdfPreview's own suite).
// The received `letterLayoutId` is surfaced as a data-* attribute so tests can
// prove the preview reads the SAME value threaded to export (ADR-012 parity).
vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: ({ letterLayoutId }: { letterLayoutId?: string }) => (
    <div data-testid={TEST_IDS.documents.pdfPreview} data-letter-layout-id={letterLayoutId ?? ''}>
      PDF
    </div>
  ),
}));

// EditableOutput (rendered inside OutputPanelDone) calls useContactProfile() which
// reaches for AppClientProvider.  Return a stub so no provider tree is needed.
vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => ({ data: undefined }),
}));

// EditableOutput also reads useSelectedModel, now backed by the `useActiveConfig`
// query (task #16) which reaches for AppClientProvider — stub it to a plain model.
vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'llama3',
}));

// The quality panel's "Re-check" action calls useValidateContent(), which also
// reaches for AppClientProvider — stub it. Hoisted + mutable so the dedicated
// "Re-check" describe block below can control what mutateAsync resolves.
const validateContentMock = vi.hoisted(() => ({ mutateAsync: vi.fn(), isPending: false }));
vi.mock('@/services/use-resume-validation', () => ({
  useValidateContent: () => validateContentMock,
}));

// Persisting a re-checked report goes through the aiGenerations save mutation
// (the only write path that reaches `quality_report`) — stub it and capture the
// request so the "doesn't clobber the record" assertions can inspect its shape.
const saveGenerationMock = vi.hoisted(() => ({ mutate: vi.fn() }));
vi.mock('@/services/use-ai-generations', () => ({
  useSaveAiGeneration: () => saveGenerationMock,
}));

// The Re-check tests below pass a `jobAd`, which also mounts TrimPanel — it
// calls useTrimSuggestions() (AppClientProvider) and is out of scope here
// (its own suite covers it).
vi.mock('../TrimPanel', () => ({
  TrimPanel: () => null,
}));

// Stub useDebouncedCommit so tests don't depend on fake timers.
// scheduleCommit immediately calls onCommit with the (out, value) pair —
// simulates instant commit in tests. flush() with no argument is also a no-op
// (the pair was just committed by scheduleCommit already).
vi.mock('@/hooks/use-debounced-commit', () => ({
  useDebouncedCommit: (onCommit: (out: string, v: string) => void) => ({
    scheduleCommit: (out: string, v: string) => onCommit(out, v),
    flush: () => undefined,
    cancel: () => undefined,
  }),
}));

const RAW = 'Led **payments** migration at scale.';

function renderPanel(overrides: Partial<React.ComponentProps<typeof OutputPanelDone>> = {}) {
  const onOutputChange = vi.fn();
  const onExport = vi.fn();
  const onCopy = vi.fn();
  const onLetterLayoutChange = vi.fn();
  render(
    <OutputPanelDone
      resumeOut={RAW}
      coverOut=""
      activeOut="resume"
      meta={null}
      mode="ats"
      templateId="classic"
      atsMode={false}
      onActiveOutChange={vi.fn()}
      onLetterLayoutChange={onLetterLayoutChange}
      onCopy={onCopy}
      onExport={onExport}
      onOutputChange={onOutputChange}
      onRegenerate={vi.fn()}
      copied={false}
      {...overrides}
    />
  );
  return { onOutputChange, onExport, onCopy, onLetterLayoutChange };
}

describe('OutputPanelDone — preview/edit', () => {
  it('shows the real-PDF preview by default (#24), not markdown or a textarea', () => {
    renderPanel();
    // The default Preview tab renders the real-PDF view, not the markdown fallback.
    expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toBeInTheDocument();
    expect(screen.queryByText(/\*\*payments\*\*/)).toBeNull();
    // No editable textarea while previewing.
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('switches to a raw textarea with markers intact (export source untouched)', () => {
    const { onOutputChange } = renderPanel();
    // The Preview/Edit/Source switch is a SegmentedControl radio group. The raw
    // markdown textarea lives in the **Source** tab (Edit is now the WYSIWYG surface).
    // t('aiGenerate.source') resolves to "Source" via the real en locale.
    fireEvent.click(screen.getByRole('radio', { name: /source/i }));

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
    // Raw text — including the **payments** markers the export pipeline reads.
    expect(textarea.value).toBe(RAW);
    // Switching views must not mutate the canonical output.
    expect(onOutputChange).not.toHaveBeenCalled();
  });

  it('no Save button is rendered (auto-debounce replaced manual save)', () => {
    renderPanel();
    // Switch to Source so the full edit toolbar is visible.
    fireEvent.click(screen.getByRole('radio', { name: /source/i }));
    expect(screen.queryByRole('button', { name: /save/i })).toBeNull();
  });
});

// The primary AI-Generate flow defaults `target: 'both'`, producing BOTH docs —
// the letter-layout picker must be reachable there (regression: it was only
// wired via StepTemplate's cover-only gate, locking `target: 'both'` users to
// Classic since OutputPanelDone never rendered it).
describe('OutputPanelDone — letter-layout picker (cover tab only)', () => {
  const letterOption = (id: string) => `${TEST_IDS.generation.letterLayoutOption}-${id}`;

  it('is absent on the résumé tab', () => {
    renderPanel({ activeOut: 'resume', coverOut: 'Dear Team, ...' });
    expect(screen.queryByTestId(letterOption('classic'))).not.toBeInTheDocument();
  });

  it('renders on the cover tab, defaulting to classic selected', () => {
    renderPanel({ activeOut: 'cover', resumeOut: '', coverOut: 'Dear Team, ...' });
    expect(screen.getByTestId(letterOption('classic'))).toHaveAttribute('aria-checked', 'true');
  });

  it('forwards a layout pick to onLetterLayoutChange, which drives the live preview + export', async () => {
    const user = userEvent.setup();
    const { onLetterLayoutChange } = renderPanel({
      activeOut: 'cover',
      resumeOut: '',
      coverOut: 'Dear Team, ...',
    });

    await user.click(screen.getByTestId(letterOption('refined')));
    expect(onLetterLayoutChange).toHaveBeenCalledWith('refined');
  });

  it('reflects an already-chosen layout as checked (preview + picker agree)', () => {
    renderPanel({
      activeOut: 'cover',
      resumeOut: '',
      coverOut: 'Dear Team, ...',
      letterLayoutId: 'banded',
    });
    expect(screen.getByTestId(letterOption('banded'))).toHaveAttribute('aria-checked', 'true');
  });

  it('a chosen layout reaches PdfPreview with the SAME value as the picker — export reads the same state', () => {
    // Mirrors the primary `target: 'both'` flow: the host re-renders with the
    // session-store value the picker just set, threaded straight to preview
    // (and to exportPDF/exportDOCX via the same host state — see export.test.ts).
    renderPanel({
      activeOut: 'cover',
      resumeOut: '',
      coverOut: 'Dear Team, ...',
      letterLayoutId: 'refined',
    });
    expect(screen.getByTestId(letterOption('refined'))).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveAttribute(
      'data-letter-layout-id',
      'refined'
    );
  });
});

describe('OutputPanelDone — quality panel Re-check wiring', () => {
  const META = {
    candidateName: 'Ada',
    jobTitle: 'Engineer',
    companyName: 'Acme',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: ['rust'],
  };
  const STALE_REPORT = {
    schemaVersion: 2 as const,
    pipeline: 'fast' as const,
    generatedAt: 1,
    resume: {
      report: {
        ok: true,
        issues: [],
        metrics: {
          keywordCoverage: null,
          topRequirementHits: 0,
          duplicateRatio: 0,
          rolesSource: 0,
          rolesOutput: 0,
        },
      },
      // Mismatches RAW's hash on purpose — the badge renders stale, so the
      // panel's Re-check button is reachable.
      sourceTextHash: -1,
    },
  };

  beforeEach(() => saveGenerationMock.mutate.mockClear());

  it('re-validates the current text and hands the merged report to onReportChange', async () => {
    const freshPayload = {
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: 90,
        topRequirementHits: 1,
        duplicateRatio: 0,
        rolesSource: 1,
        rolesOutput: 1,
      },
    };
    validateContentMock.mutateAsync = vi.fn().mockResolvedValue(freshPayload);
    validateContentMock.isPending = false;
    const onReportChange = vi.fn();
    const user = userEvent.setup();

    renderPanel({
      report: STALE_REPORT,
      onReportChange,
      meta: META,
      sourceResume: 'source resume text',
      jobAd: 'the job ad',
    });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    await user.click(screen.getByRole('button', { name: /re-check/i }));

    expect(validateContentMock.mutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        generated: RAW,
        source: 'source resume text',
        jobAd: 'the job ad',
        topRequirements: ['rust'],
        targetLanguage: 'en',
        docKind: 'resume',
      })
    );
    expect(onReportChange).toHaveBeenCalledWith(
      expect.objectContaining({
        resume: { report: freshPayload, sourceTextHash: expect.any(Number) },
      })
    );
  });

  // A re-check that only lived in React state was discarded on reopen — the
  // stale badge came straight back. It must reach the record, and it must NOT
  // take anything else with it: every other field is sent blank because Rust's
  // `merge_application` keeps the STORED value for a blank incoming field
  // (`pick`), while the two texts are sent at their live values so the stored
  // text stays the text the fresh hash describes.
  it('persists the merged report onto the record without emptying its texts', async () => {
    const freshPayload = {
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: 90,
        topRequirementHits: 1,
        duplicateRatio: 0,
        rolesSource: 1,
        rolesOutput: 1,
      },
    };
    validateContentMock.mutateAsync = vi.fn().mockResolvedValue(freshPayload);
    validateContentMock.isPending = false;
    const user = userEvent.setup();

    renderPanel({
      report: STALE_REPORT,
      onReportChange: vi.fn(),
      meta: META,
      sourceResume: 'source resume text',
      jobAd: 'the job ad',
      coverOut: 'Dear Team, ...',
      jobUrl: 'https://acme.com/job/42',
      board: 'linkedin',
    });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    await user.click(screen.getByRole('button', { name: /re-check/i }));

    expect(saveGenerationMock.mutate).toHaveBeenCalledTimes(1);
    const request = saveGenerationMock.mutate.mock.calls[0]?.[0];
    // The merged wrapper travels as the serialized wrapper, résumé slot fresh.
    expect(JSON.parse(String(request.qualityReport))).toEqual(
      expect.objectContaining({
        schemaVersion: 2,
        resume: { report: freshPayload, sourceTextHash: expect.any(Number) },
      })
    );
    // Texts carried at their live values — never emptied.
    expect(request.resumeText).toBe(RAW);
    expect(request.coverLetterText).toBe('Dear Team, ...');
    // Routing key present (without it the save would insert a duplicate row).
    expect(request.jobUrl).toBe('https://acme.com/job/42');
    // Everything else deliberately blank so the merge keeps the stored value.
    expect(request.candidateName).toBe('');
    expect(request.jobAd).toBe('');
    expect(request.mode).toBe('');
    expect(request.topRequirements).toEqual([]);
  });

  it('keeps the re-check session-only when the record has no url to merge onto', async () => {
    validateContentMock.mutateAsync = vi.fn().mockResolvedValue({
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: 90,
        topRequirementHits: 1,
        duplicateRatio: 0,
        rolesSource: 1,
        rolesOutput: 1,
      },
    });
    validateContentMock.isPending = false;
    const onReportChange = vi.fn();
    const user = userEvent.setup();

    renderPanel({
      report: STALE_REPORT,
      onReportChange,
      meta: META,
      sourceResume: 'source resume text',
      jobAd: 'the job ad',
    });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    await user.click(screen.getByRole('button', { name: /re-check/i }));

    // The session still updates — only the write is skipped, because a
    // url-less save would fork the history into a second row.
    expect(onReportChange).toHaveBeenCalled();
    expect(saveGenerationMock.mutate).not.toHaveBeenCalled();
  });

  it('leaves the report untouched when validation fails (best-effort)', async () => {
    validateContentMock.mutateAsync = vi.fn().mockRejectedValue(new Error('boom'));
    validateContentMock.isPending = false;
    const onReportChange = vi.fn();
    const user = userEvent.setup();

    renderPanel({
      report: STALE_REPORT,
      onReportChange,
      meta: META,
      sourceResume: 'source resume text',
      jobAd: 'the job ad',
    });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    await user.click(screen.getByRole('button', { name: /re-check/i }));

    expect(onReportChange).not.toHaveBeenCalled();
  });

  it('hides the Re-check action when onReportChange is not wired', async () => {
    const user = userEvent.setup();
    renderPanel({
      report: STALE_REPORT,
      meta: META,
      sourceResume: 'source resume text',
      jobAd: 'the job ad',
    });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    expect(screen.queryByRole('button', { name: /re-check/i })).toBeNull();
  });

  it('a session reset while Re-check is still in flight never resurrects a report into the cleared session', async () => {
    let resolveValidate!: (v: unknown) => void;
    validateContentMock.mutateAsync = vi.fn(
      () => new Promise((resolve) => (resolveValidate = resolve))
    );
    validateContentMock.isPending = false;
    const onReportChange = vi.fn();
    const user = userEvent.setup();

    const props = {
      resumeOut: RAW,
      coverOut: '',
      activeOut: 'resume' as const,
      mode: 'ats',
      templateId: 'classic' as const,
      atsMode: false,
      onActiveOutChange: vi.fn(),
      onCopy: vi.fn(),
      onExport: vi.fn(),
      onOutputChange: vi.fn(),
      onRegenerate: vi.fn(),
      copied: false,
    };

    const { rerender } = render(
      <OutputPanelDone
        {...props}
        report={STALE_REPORT}
        onReportChange={onReportChange}
        meta={META}
        sourceResume="source resume text"
        jobAd="the job ad"
      />
    );

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    await user.click(screen.getByRole('button', { name: /re-check/i }));

    // A session Reset happens while the validate call is still in flight —
    // the parent re-renders this panel with its cleared/default props
    // (mirrors what AIGeneratePage's reset() produces).
    rerender(
      <OutputPanelDone
        {...props}
        resumeOut=""
        report={null}
        onReportChange={onReportChange}
        meta={null}
        sourceResume=""
        jobAd=""
      />
    );

    resolveValidate({
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: 90,
        topRequirementHits: 1,
        duplicateRatio: 0,
        rolesSource: 1,
        rolesOutput: 1,
      },
    });
    // Flush the resolved promise's continuation (the post-await half of
    // handleRecheck) so a would-be call to onReportChange has had its chance.
    await new Promise((r) => setTimeout(r, 0));

    expect(onReportChange).not.toHaveBeenCalled();
  });
});
