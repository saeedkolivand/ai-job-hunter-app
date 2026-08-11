import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { buildSectionVerdicts, type Fabrication } from '@/lib/generate';

import { QualityBadge } from './QualityBadge';
import { type QualityPipelineReview, QualityReportPanel } from './QualityReportPanel';

const METRICS: ContentReportPayload['metrics'] = {
  keywordCoverage: 60,
  topRequirementHits: 1,
  topRequirementsMeasured: 2,
  duplicateRatio: 0,
  rolesSource: 1,
  rolesOutput: 1,
};

const CLEAN_REPORT: ContentReportPayload = { ok: true, issues: [], metrics: METRICS };

const DOCUMENT = ['Summary', 'Cut latency by 40% across the fleet.', '', 'Experience', 'Acme'].join(
  '\n'
);

const PENDING: Fabrication = {
  issueKey: 'factual.unsourced_metric#0',
  code: 'factual.unsourced_metric',
  evidence: 'Cut latency by 40%',
};

function pipeline(overrides: Partial<QualityPipelineReview> = {}): QualityPipelineReview {
  return {
    documentText: DOCUMENT,
    sections: buildSectionVerdicts(CLEAN_REPORT, DOCUMENT),
    fabrications: [PENDING],
    ...overrides,
  };
}

function renderPanel(review: QualityPipelineReview, report = CLEAN_REPORT) {
  return render(
    <QualityReportPanel open onClose={vi.fn()} report={report} docKind="resume" pipeline={review} />
  );
}

describe('QualityReportPanel — staged run extras', () => {
  it('renders a verdict chip per section, clean ones included', () => {
    renderPanel(pipeline());
    expect(screen.getByText('Section verdicts')).toBeInTheDocument();
    expect(screen.getAllByText('No changes needed').length).toBeGreaterThan(0);
  });

  it('offers Fix this section only for a section it can address', async () => {
    const onFixSection = vi.fn();
    renderPanel(
      pipeline({
        sections: [
          { label: 'Experience', sectionKey: 'experience:0', issues: 2, criticals: 1 },
          { label: 'Volunteering', sectionKey: null, issues: 1, criticals: 0 },
        ],
        onFixSection,
      })
    );

    const fixButtons = screen.getAllByRole('button', { name: /fix this section/i });
    expect(fixButtons).toHaveLength(1);
    expect(screen.getByText(/can't re-generate this section/i)).toBeInTheDocument();

    const [fix] = fixButtons;
    if (!fix) throw new Error('expected exactly one Fix button');
    await userEvent.click(fix);
    await userEvent.type(screen.getByLabelText(/what should change/i), 'lead with the migration');
    await userEvent.click(screen.getByRole('button', { name: /regenerate this section/i }));
    expect(onFixSection).toHaveBeenCalledWith('experience:0', 'lead with the migration');
  });

  it('surfaces a refusal (e.g. a non-latest run) instead of swallowing it', () => {
    renderPanel(
      pipeline({
        sections: [{ label: 'Summary', sectionKey: 'summary', issues: 1, criticals: 0 }],
        onFixSection: vi.fn(),
        fixError: 'Only the newest run for this job can be changed.',
      })
    );
    expect(screen.getByRole('alert')).toHaveTextContent(/only the newest run/i);
  });

  it('reports repair rounds for the RUN and says they are not per-section', () => {
    renderPanel(pipeline({ repairRounds: 2, repairReverted: true }));
    expect(screen.getByText(/2 repair rounds ran automatically/i)).toBeInTheDocument();
    expect(screen.getByText(/made things worse and was reverted/i)).toBeInTheDocument();
    expect(screen.getByText(/doesn't record which section/i)).toBeInTheDocument();
  });

  describe('terminal per-bullet review', () => {
    it('lists each flagged claim with Remove and Keep, and removes nothing itself', async () => {
      const onResolveFabrication = vi.fn();
      renderPanel(pipeline({ onResolveFabrication }));

      expect(screen.getByText('“Cut latency by 40%”')).toBeInTheDocument();
      expect(screen.getByText(/1 claim still needs a decision/i)).toBeInTheDocument();

      await userEvent.click(screen.getByRole('button', { name: /remove/i }));
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'remove');
    });

    it('records Keep through the same command', async () => {
      const onResolveFabrication = vi.fn();
      renderPanel(pipeline({ onResolveFabrication }));
      await userEvent.click(screen.getByRole('button', { name: /keep/i }));
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'keep');
    });

    // A preserved entry can outlive the line it describes (a hand-edit, or a
    // Re-check carrying it across a newer document). Asking the user to judge
    // text they cannot find is the failure this state prevents — and it stays
    // decidable, because deciding it is what clears needs-review.
    it('flags an entry whose evidence is gone rather than prompting blindly', async () => {
      const onResolveFabrication = vi.fn();
      renderPanel(
        pipeline({
          documentText: 'Summary\nLed the platform migration.',
          onResolveFabrication,
        })
      );
      expect(screen.getByText('No longer in the document')).toBeInTheDocument();
      expect(screen.getByText(/decide it anyway to clear the review/i)).toBeInTheDocument();
      await userEvent.click(screen.getByRole('button', { name: /keep/i }));
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'keep');
    });

    it('shows a decided entry as decided, with no second prompt', () => {
      renderPanel(pipeline({ fabrications: [{ ...PENDING, decision: 'remove' }] }));
      expect(screen.getByText('Marked for removal')).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /^keep$/i })).not.toBeInTheDocument();
      expect(screen.getByText(/every flagged claim has a decision/i)).toBeInTheDocument();
    });

    // `needsReview` is not a failure — but it is emphatically not clean either.
    it('never renders the "passed every check" empty state while claims are open', () => {
      renderPanel(pipeline());
      expect(screen.queryByText('No issues found')).not.toBeInTheDocument();
    });

    it('does show the clean empty state once the run has no findings at all', () => {
      renderPanel(pipeline({ fabrications: [] }));
      expect(screen.getByText('No issues found')).toBeInTheDocument();
    });
  });
});

describe('QualityBadge — a needsReview run is never green', () => {
  const slot = { report: CLEAN_REPORT, sourceTextHash: 0 };
  const wrapper = {
    schemaVersion: 2 as const,
    pipeline: 'quality' as const,
    generatedAt: 0,
    resume: { ...slot, sourceTextHash: hashOf(DOCUMENT) },
  };

  function hashOf(text: string): number {
    let hash = 5381;
    for (let i = 0; i < text.length; i++) hash = (hash * 33) ^ text.charCodeAt(i);
    return hash >>> 0;
  }

  it('counts undecided claims as open issues even on an otherwise clean report', () => {
    render(
      <QualityBadge
        report={wrapper}
        docKind="resume"
        currentText={DOCUMENT}
        pipeline={pipeline()}
      />
    );
    expect(screen.queryByText('Checked — no issues')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1 issue/i })).toBeInTheDocument();
  });

  it('goes green only once every claim carries a verdict', () => {
    render(
      <QualityBadge
        report={wrapper}
        docKind="resume"
        currentText={DOCUMENT}
        pipeline={pipeline({ fabrications: [{ ...PENDING, decision: 'keep' }] })}
      />
    );
    expect(screen.getByText('Checked — no issues')).toBeInTheDocument();
  });
});
