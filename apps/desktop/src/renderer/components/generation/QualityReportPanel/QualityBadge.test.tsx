import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import type { QualityReport } from '@/lib/generate';

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
