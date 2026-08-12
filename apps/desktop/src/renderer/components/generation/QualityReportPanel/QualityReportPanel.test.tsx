import { describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { QualityReportPanel } from './QualityReportPanel';

const METRICS = {
  keywordCoverage: 62.4,
  topRequirementHits: 2,
  topRequirementsMeasured: 4,
  duplicateRatio: 0.25,
  rolesSource: 3,
  rolesOutput: 2,
};

const REPORT: ContentReportPayload = {
  ok: false,
  issues: [
    {
      severity: 'critical',
      code: 'factual.dropped_role',
      section: 'Experience',
      message: 'raw rust message — never rendered, the UI localizes off `code`',
      evidence: 'Acme Corp — Senior Engineer',
    },
    {
      severity: 'warning',
      code: 'ats.keyword_density',
      section: 'Experience',
      message: 'x',
      evidence: null,
    },
    {
      severity: 'warning',
      code: 'content.language_mismatch',
      section: null,
      message: 'x',
      evidence: null,
    },
  ],
  metrics: METRICS,
};

describe('QualityReportPanel', () => {
  it('renders an empty state for a clean report', () => {
    render(
      <QualityReportPanel
        open
        onClose={vi.fn()}
        report={{ ok: true, issues: [], metrics: METRICS }}
        docKind="resume"
      />
    );
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText(/no issues found/i)).toBeInTheDocument();
  });

  it('treats a null report the same as a clean one', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={null} docKind="resume" />);
    expect(screen.getByText(/no issues found/i)).toBeInTheDocument();
  });

  it('groups issues by section, with the document-wide bucket last', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
    const headings = screen.getAllByRole('heading', { level: 3 }).map((h) => h.textContent);
    // "Experience" (named section) must appear before the document-wide group.
    const experienceIndex = headings.findIndex((h) => h === 'Experience');
    const wideIndex = headings.findIndex(
      (h) => h === 'quality.panel.documentWide' || /document-wide/i.test(h ?? '')
    );
    expect(experienceIndex).toBeGreaterThanOrEqual(0);
    expect(wideIndex).toBeGreaterThan(experienceIndex);
  });

  it('renders a severity chip per issue (critical vs warning)', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
    expect(screen.getAllByText(/critical/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/warning/i).length).toBeGreaterThan(0);
  });

  it('renders known-code guidance text, never the raw Rust message', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
    expect(screen.queryByText(/raw rust message/i)).toBeNull();
  });

  it('renders the evidence span as quoted text when present', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
    expect(screen.getByText(/Acme Corp — Senior Engineer/)).toBeInTheDocument();
  });

  it('falls back to its own Rust-authored message for an unknown/future issue code that has one', () => {
    const withUnknownCode: ContentReportPayload = {
      ok: false,
      issues: [
        {
          severity: 'warning',
          code: 'future.not_yet_translated',
          section: null,
          message: 'a genuinely useful explanation the Rust validator wrote',
          evidence: null,
        },
      ],
      metrics: METRICS,
    };
    render(<QualityReportPanel open onClose={vi.fn()} report={withUnknownCode} docKind="resume" />);
    // Never a raw i18n key leaking onto the screen.
    expect(screen.queryByText('quality.issue.future.not_yet_translated')).toBeNull();
    expect(
      screen.getByText(/a genuinely useful explanation the rust validator wrote/i)
    ).toBeInTheDocument();
    expect(screen.queryByText(/deterministic check flagged this/i)).toBeNull();
  });

  it('falls back to the generic message for a message-less unknown/future issue code', () => {
    const withUnknownCode: ContentReportPayload = {
      ok: false,
      issues: [
        {
          severity: 'warning',
          code: 'future.not_yet_translated',
          section: null,
          message: '',
          evidence: null,
        },
      ],
      metrics: METRICS,
    };
    render(<QualityReportPanel open onClose={vi.fn()} report={withUnknownCode} docKind="resume" />);
    expect(screen.queryByText('quality.issue.future.not_yet_translated')).toBeNull();
    expect(screen.getByText(/deterministic check flagged this/i)).toBeInTheDocument();
  });

  it('renders the Rust-authored message, numbers intact, for a lossy-number code (ats.bullet_count)', () => {
    const withCount: ContentReportPayload = {
      ok: false,
      issues: [
        {
          severity: 'warning',
          code: 'ats.bullet_count',
          section: 'Experience',
          message: '"Backend Engineer, Acme" has 9 bullets — keep the 6 strongest.',
          evidence: 'Backend Engineer, Acme',
        },
      ],
      metrics: METRICS,
    };
    render(<QualityReportPanel open onClose={vi.fn()} report={withCount} docKind="resume" />);
    // The interpolated counts (9 bullets, keep the 6 strongest) survive verbatim.
    expect(screen.getByText(/has 9 bullets — keep the 6 strongest/i)).toBeInTheDocument();
    // The static translation (which has no numbers at all) must NOT also render.
    expect(screen.queryByText(/unusual number of bullets/i)).toBeNull();
  });

  // ── The max-depth judge ─────────────────────────────────────────────────────
  //
  // A `judge.*` issue is the ONLY finding in this panel whose message is
  // free prose a model wrote about this specific document (already in the run's
  // target language). Two things follow, and both are guarded here.
  describe('a judge issue', () => {
    const JUDGE_NOTE =
      'The second bullet under Acme repeats the platform migration already claimed in the summary.';
    const judgeReport = (code = 'judge.clarity'): ContentReportPayload => ({
      ok: false,
      issues: [
        {
          severity: 'warning',
          code,
          section: 'Experience',
          message: JUDGE_NOTE,
          evidence: 'Led the platform migration end to end',
        },
      ],
      metrics: METRICS,
    });

    // THE guard. `messageFor` returns the translation and discards `message`
    // for any code with a `quality.issue.<code>` key — so a catalog entry for a
    // judge code would silently replace the whole remark with a canned
    // sentence. The prefix short-circuits BEFORE that lookup: add
    // `quality.issue.judge.clarity` to the catalog and this still passes, but
    // delete the `isJudgeCode` branch from `messageFor` and add the key and it
    // fails. (Both mutations were run.)
    it('renders the model’s own note, never a catalog string', () => {
      render(<QualityReportPanel open onClose={vi.fn()} report={judgeReport()} docKind="resume" />);
      expect(screen.getByText(JUDGE_NOTE)).toBeInTheDocument();
      expect(screen.queryByText(/deterministic check flagged this/i)).toBeNull();
      expect(screen.queryByText('quality.issue.judge.clarity')).toBeNull();
    });

    // The note is model-authored prose derived from an untrusted job posting.
    // It stays a TEXT node: no markdown, no linkification, no HTML.
    it('renders the note as inert text, not as markup', () => {
      const injected: ContentReportPayload = {
        ok: false,
        issues: [
          {
            severity: 'warning',
            code: 'judge.note',
            section: null,
            message: '<img src=x onerror="alert(1)"> **bold** [link](https://evil.test)',
            evidence: null,
          },
        ],
        metrics: METRICS,
      };
      const { container } = render(
        <QualityReportPanel open onClose={vi.fn()} report={injected} docKind="resume" />
      );
      expect(
        screen.getByText(/<img src=x onerror="alert\(1\)"> \*\*bold\*\* \[link\]/)
      ).toBeInTheDocument();
      expect(container.querySelector('img')).toBeNull();
      expect(container.querySelector('a[href*="evil.test"]')).toBeNull();
    });

    it('labels its provenance so it does not read as a validator finding', () => {
      render(<QualityReportPanel open onClose={vi.fn()} report={judgeReport()} docKind="resume" />);
      const badge = screen.getByText(/AI reviewer/);
      expect(badge).toBeInTheDocument();
      expect(badge).toHaveTextContent('Clarity');
      expect(badge).toHaveAttribute('title', expect.stringMatching(/a suggestion, not a rule/i));
    });

    it('keeps the badge — minus the kind — for a judge kind this build predates', () => {
      render(
        <QualityReportPanel
          open
          onClose={vi.fn()}
          report={judgeReport('judge.some_future_kind')}
          docKind="resume"
        />
      );
      expect(screen.getByText(/AI reviewer/)).toBeInTheDocument();
      // Never a raw i18n key on screen.
      expect(screen.queryByText(/quality\.panel\.judge/)).toBeNull();
    });

    it('leaves a deterministic finding unbadged', () => {
      render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
      expect(screen.queryByText(/AI reviewer/)).toBeNull();
    });
  });

  it('still renders the static translation for a code outside the lossy set, even though its Rust message has numbers (alignment.low_coverage)', () => {
    const withCoverageNumbers: ContentReportPayload = {
      ok: false,
      issues: [
        {
          severity: 'warning',
          code: 'alignment.low_coverage',
          section: null,
          message:
            "The generated document covers 40% of this posting's keywords where your source résumé already covered 55%. Something relevant was dropped — compare the two before sending.",
          evidence: '40% vs 55%',
        },
      ],
      metrics: METRICS,
    };
    render(
      <QualityReportPanel open onClose={vi.fn()} report={withCoverageNumbers} docKind="resume" />
    );
    // alignment.low_coverage's two percentages are already duplicated into
    // `evidence`, so the translation (guidance, no numbers) stays preferred.
    expect(screen.getByText(/covers little of the job ad's vocabulary/i)).toBeInTheDocument();
    expect(screen.queryByText(/covers 40% of this posting/i)).toBeNull();
  });

  it('maps the truncated-report marker to its own i18n key, not the raw Rust message', () => {
    const truncated: ContentReportPayload = {
      ok: true,
      issues: [
        {
          severity: 'warning',
          code: 'report.truncated',
          section: null,
          message:
            '49 more issues found but not shown here — this document has an unusually large number of findings.',
          evidence: '49',
        },
      ],
      metrics: METRICS,
    };
    render(<QualityReportPanel open onClose={vi.fn()} report={truncated} docKind="resume" />);
    expect(screen.getByText(/more than fit in this report/i)).toBeInTheDocument();
    expect(screen.queryByText(/49 more issues found/i)).toBeNull();
  });

  it('renders a metrics footer with keyword coverage, requirement hits, duplicates, and roles', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
    const footer = screen
      .getByRole('heading', { level: 3, name: /metrics/i })
      .closest('div') as HTMLElement;
    expect(within(footer).getByText('62%')).toBeInTheDocument();
    // Hits render as a ratio against the measured denominator, never a bare
    // uninterpretable count (round-9 finding).
    expect(within(footer).getByText('2 / 4')).toBeInTheDocument();
    expect(within(footer).getByText('25%')).toBeInTheDocument();
    expect(within(footer).getByText(/3.*2/)).toBeInTheDocument();
  });

  it('falls back to the bare count for a pre-denominator persisted report', () => {
    const legacy: ContentReportPayload = {
      ok: true,
      issues: [],
      metrics: { ...METRICS, topRequirementsMeasured: null },
    };
    render(<QualityReportPanel open onClose={vi.fn()} report={legacy} docKind="resume" />);
    const footer = screen
      .getByRole('heading', { level: 3, name: /metrics/i })
      .closest('div') as HTMLElement;
    expect(within(footer).getByText('2')).toBeInTheDocument();
    expect(within(footer).queryByText(/2 \/ /)).toBeNull();
  });

  it('renders an em dash when requirement hits were not measured (uncomparable posting)', () => {
    const unmeasured: ContentReportPayload = {
      ok: true,
      issues: [],
      metrics: { ...METRICS, topRequirementHits: null },
    };
    render(<QualityReportPanel open onClose={vi.fn()} report={unmeasured} docKind="resume" />);
    const footer = screen
      .getByRole('heading', { level: 3, name: /metrics/i })
      .closest('div') as HTMLElement;
    // null = the validator could not measure (language mismatch / no
    // requirements) — never a literal 0 presented as fact.
    expect(within(footer).getAllByText('—').length).toBeGreaterThan(0);
    expect(within(footer).queryByText('2')).toBeNull();
  });

  it('shows only keyword coverage for a cover letter — the other metrics are hard constants, not measurements', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="coverLetter" />);
    const footer = screen
      .getByRole('heading', { level: 3, name: /metrics/i })
      .closest('div') as HTMLElement;
    // keywordCoverage is genuinely computed for letters — stays.
    expect(within(footer).getByText('62%')).toBeInTheDocument();
    // rolesSource→rolesOutput, topRequirementHits, duplicateRatio are the
    // CoverLetter arm's (0, 0.0, 0, 0) constants — rendering them would state
    // "0 requirements covered" as fact. All three rows hidden.
    expect(within(footer).queryByText(/roles/i)).toBeNull();
    expect(within(footer).queryByText(/3.*2/)).toBeNull();
    expect(within(footer).queryByText('2')).toBeNull(); // topRequirementHits value
    expect(within(footer).queryByText('25%')).toBeNull(); // duplicateRatio value
  });

  it('titles the dialog per docKind', () => {
    const { rerender } = render(
      <QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />
    );
    expect(screen.getByRole('heading', { level: 2 }).textContent).toMatch(/résumé/i);

    rerender(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="coverLetter" />);
    expect(screen.getByRole('heading', { level: 2 }).textContent).toMatch(/cover letter/i);
  });
});

describe('QualityReportPanel — staleness notice + re-check', () => {
  it('shows no notice when not stale', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" />);
    expect(screen.queryByText(/edited since this report/i)).toBeNull();
  });

  it('shows a small notice when stale', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" stale />);
    expect(screen.getByText(/edited since this report/i)).toBeInTheDocument();
  });

  it('renders no Re-check button when onRecheck is omitted, even while stale', () => {
    render(<QualityReportPanel open onClose={vi.fn()} report={REPORT} docKind="resume" stale />);
    expect(screen.queryByRole('button', { name: /re-check/i })).toBeNull();
  });

  it('calls onRecheck when the Re-check button is clicked', async () => {
    const user = userEvent.setup();
    const onRecheck = vi.fn();
    render(
      <QualityReportPanel
        open
        onClose={vi.fn()}
        report={REPORT}
        docKind="resume"
        stale
        onRecheck={onRecheck}
      />
    );
    await user.click(screen.getByRole('button', { name: /re-check/i }));
    expect(onRecheck).toHaveBeenCalledTimes(1);
  });

  it('disables the Re-check button and shows the checking label while rechecking', () => {
    render(
      <QualityReportPanel
        open
        onClose={vi.fn()}
        report={REPORT}
        docKind="resume"
        stale
        onRecheck={vi.fn()}
        rechecking
      />
    );
    const button = screen.getByRole('button', { name: /checking/i });
    expect(button).toBeDisabled();
  });
});
