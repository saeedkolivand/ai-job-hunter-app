import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { hashText, type QualityReport } from '@/lib/generate';

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

function report(overrides: Partial<QualityReport>): QualityReport {
  return { schemaVersion: 1, pipeline: 'fast', generatedAt: 1, ...overrides };
}

describe('QualityBadge', () => {
  it('renders nothing when this document has no report yet', () => {
    const { container } = render(<QualityBadge report={null} docKind="resume" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing for a docKind the report never validated (e.g. cover-only run)', () => {
    const { container } = render(
      <QualityBadge report={report({ coverLetter: CLEAN })} docKind="resume" />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the clean state for a report with zero issues', () => {
    render(<QualityBadge report={report({ resume: CLEAN })} docKind="resume" />);
    expect(screen.getByRole('button', { name: /checked/i })).toBeInTheDocument();
  });

  it('shows the issue count + critical count for a report with issues', () => {
    render(<QualityBadge report={report({ resume: WITH_ISSUES })} docKind="resume" />);
    const button = screen.getByRole('button');
    expect(button.textContent).toMatch(/2/);
    expect(button.textContent).toMatch(/1/);
  });

  it('opens the quality report panel on click, scoped to the right document', async () => {
    const user = userEvent.setup();
    render(<QualityBadge report={report({ resume: WITH_ISSUES })} docKind="resume" />);

    expect(screen.queryByRole('dialog')).toBeNull();
    await user.click(screen.getByRole('button'));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });
});

describe('QualityBadge — staleness', () => {
  it('stays in the clean state when currentText matches the validated hash', () => {
    render(
      <QualityBadge
        report={report({ resume: CLEAN, sourceTextHash: { resume: hashText('same text') } })}
        docKind="resume"
        currentText="same text"
      />
    );
    expect(screen.getByRole('button', { name: /checked/i })).toBeInTheDocument();
  });

  it('switches to the stale state — never the green clean state — once the text diverges', () => {
    render(
      <QualityBadge
        report={report({ resume: CLEAN, sourceTextHash: { resume: hashText('original text') } })}
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
        report={report({
          resume: WITH_ISSUES,
          sourceTextHash: { resume: hashText('original text') },
        })}
        docKind="resume"
        currentText="edited text"
      />
    );
    expect(screen.getByRole('button').textContent).toMatch(/checked before your edits/i);
  });

  it('never flags staleness for a legacy report with no sourceTextHash', () => {
    render(
      <QualityBadge report={report({ resume: CLEAN })} docKind="resume" currentText="anything" />
    );
    expect(screen.getByRole('button', { name: /checked/i })).toBeInTheDocument();
  });

  it("forwards onRecheck/rechecking to the panel's Re-check action", async () => {
    const user = userEvent.setup();
    const onRecheck = vi.fn();
    render(
      <QualityBadge
        report={report({ resume: CLEAN, sourceTextHash: { resume: hashText('original') } })}
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
