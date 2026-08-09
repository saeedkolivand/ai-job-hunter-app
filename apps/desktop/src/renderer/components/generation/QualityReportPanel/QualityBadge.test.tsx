import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { hashText, parseQualityReport, type QualityReport } from '@/lib/generate';

import { QualityBadge } from './QualityBadge';

const METRICS = {
  keywordCoverage: 50,
  topRequirementHits: 1,
  duplicateRatio: 0,
  rolesSource: 1,
  rolesOutput: 1,
};

const CLEAN: ContentReportPayload = { ok: true, issues: [], metrics: METRICS };

const WITH_ISSUES: ContentReportPayload = {
  ok: false,
  issues: [
    {
      severity: 'critical',
      code: 'factual.dropped_role',
      section: null,
      message: 'x',
      evidence: null,
    },
    {
      severity: 'warning',
      code: 'duplicate.bullet',
      section: 'Experience',
      message: 'y',
      evidence: null,
    },
  ],
  metrics: METRICS,
};

const TEXT = 'the validated document text';

/** A slot for `TEXT` — the badge is NOT stale as long as `currentText` matches. */
function slot(payload: ContentReportPayload, text = TEXT) {
  return { report: payload, sourceTextHash: hashText(text) };
}

function report(overrides: Partial<QualityReport>): QualityReport {
  return { schemaVersion: 2, pipeline: 'fast', generatedAt: 1, ...overrides };
}

describe('QualityBadge', () => {
  it('renders nothing when this document has no report yet', () => {
    const { container } = render(
      <QualityBadge report={null} docKind="resume" currentText={TEXT} />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing for a docKind the report never validated (e.g. cover-only run)', () => {
    const { container } = render(
      <QualityBadge report={report({ coverLetter: slot(CLEAN) })} docKind="resume" currentText="" />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the clean state for a report with zero issues', () => {
    render(
      <QualityBadge report={report({ resume: slot(CLEAN) })} docKind="resume" currentText={TEXT} />
    );
    expect(screen.getByRole('button', { name: /checked/i })).toBeInTheDocument();
  });

  it('shows the issue count + critical count for a report with issues', () => {
    render(
      <QualityBadge
        report={report({ resume: slot(WITH_ISSUES) })}
        docKind="resume"
        currentText={TEXT}
      />
    );
    const button = screen.getByRole('button');
    expect(button.textContent).toMatch(/2/);
    expect(button.textContent).toMatch(/1/);
  });

  it('opens the quality report panel on click, scoped to the right document', async () => {
    const user = userEvent.setup();
    render(
      <QualityBadge
        report={report({ resume: slot(WITH_ISSUES) })}
        docKind="resume"
        currentText={TEXT}
      />
    );

    expect(screen.queryByRole('dialog')).toBeNull();
    await user.click(screen.getByRole('button'));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  // Security finding M-1: a malformed persisted `quality_report` column value
  // must never crash the renderer past `parseQualityReport`'s shape guard —
  // this exercises the real hydration path end to end (persisted string →
  // parse → render) rather than asserting on `parseQualityReport` alone.
  it('renders without throwing when hydrated from a malformed persisted report', async () => {
    const user = userEvent.setup();
    const malformed = parseQualityReport(
      '{"schemaVersion":2,"resume":{"report":{"issues":42},"sourceTextHash":1}}'
    );

    expect(() =>
      render(<QualityBadge report={malformed} docKind="resume" currentText={TEXT} />)
    ).not.toThrow();
    // No report survived validation for this docKind — the badge renders nothing.
    expect(screen.queryByRole('button')).toBeNull();

    const { container } = render(
      <QualityBadge
        report={parseQualityReport(
          JSON.stringify({
            schemaVersion: 2,
            resume: slot(CLEAN),
            coverLetter: { report: { issues: 42 }, sourceTextHash: 1 },
          })
        )}
        docKind="resume"
        currentText={TEXT}
      />
    );
    expect(container).not.toBeEmptyDOMElement();
    await user.click(screen.getByRole('button'));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });
});

describe('QualityBadge — staleness', () => {
  it('stays in the clean state when currentText matches the slot hash', () => {
    render(
      <QualityBadge
        report={report({ resume: slot(CLEAN, 'same text') })}
        docKind="resume"
        currentText="same text"
      />
    );
    expect(screen.getByRole('button', { name: /checked/i })).toBeInTheDocument();
  });

  it('switches to the stale state — never the green clean state — once the text diverges', () => {
    render(
      <QualityBadge
        report={report({ resume: slot(CLEAN, 'original text') })}
        docKind="resume"
        currentText="edited text"
      />
    );
    const button = screen.getByRole('button');
    expect(button.textContent).toMatch(/checked before your edits/i);
    expect(button.textContent).not.toMatch(/no issues/i);
  });

  it('also switches to the stale state for a report WITH issues on diverged text', () => {
    render(
      <QualityBadge
        report={report({ resume: slot(WITH_ISSUES, 'original text') })}
        docKind="resume"
        currentText="edited text"
      />
    );
    expect(screen.getByRole('button').textContent).toMatch(/checked before your edits/i);
  });

  // The staleness anchor travels INSIDE the slot, so a résumé-only re-validation
  // (the shape a résumé-only regeneration persists) can never leave the cover
  // letter holding a report with no hash — the state that used to render green
  // "no issues" over hand-edited text. Each doc is judged by its own anchor.
  it('judges each document by its OWN slot hash — a fresh résumé slot cannot un-stale the letter', () => {
    const wrapper = report({
      resume: slot(CLEAN, 'fresh resume'),
      coverLetter: slot(CLEAN, 'original letter'),
    });

    const { unmount } = render(
      <QualityBadge report={wrapper} docKind="resume" currentText="fresh resume" />
    );
    expect(screen.getByRole('button', { name: /checked/i })).toBeInTheDocument();
    unmount();

    render(<QualityBadge report={wrapper} docKind="coverLetter" currentText="edited letter" />);
    const button = screen.getByRole('button');
    expect(button.textContent).toMatch(/checked before your edits/i);
    expect(button.textContent).not.toMatch(/no issues/i);
  });

  it("forwards onRecheck/rechecking to the panel's Re-check action", async () => {
    const user = userEvent.setup();
    const onRecheck = vi.fn();
    render(
      <QualityBadge
        report={report({ resume: slot(CLEAN, 'original') })}
        docKind="resume"
        currentText="edited"
        onRecheck={onRecheck}
      />
    );
    await user.click(screen.getByRole('button'));
    await user.click(screen.getByRole('button', { name: /re-check/i }));
    expect(onRecheck).toHaveBeenCalledTimes(1);
  });
});
