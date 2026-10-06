/** Fixtures + render helpers shared by the QualityReportPanel / QualityBadge pipeline tests. */
import { vi } from 'vitest';
import { render } from '@testing-library/react';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { buildSectionVerdicts, type Fabrication } from '@/lib/generate';

import { type QualityPipelineReview, QualityReportPanel } from './QualityReportPanel';

export const METRICS: ContentReportPayload['metrics'] = {
  keywordCoverage: 60,
  topRequirementHits: 1,
  topRequirementsMeasured: 2,
  duplicateRatio: 0,
  rolesSource: 1,
  rolesOutput: 1,
};

export const CLEAN_REPORT: ContentReportPayload = { ok: true, issues: [], metrics: METRICS };

export const FLAGGED_LINE = 'Cut latency by 40% across the fleet.';
export const DOCUMENT = ['Summary', FLAGGED_LINE, '', 'Experience', 'Acme'].join('\n');

export const PENDING: Fabrication = {
  issueKey: 'factual.unsourced_metric#0',
  code: 'factual.unsourced_metric',
  evidence: 'Cut latency by 40%',
  // The line the span sat on — the ONLY thing a removal may be anchored to.
  line: FLAGGED_LINE,
};

export function pipeline(overrides: Partial<QualityPipelineReview> = {}): QualityPipelineReview {
  return {
    documentText: DOCUMENT,
    sections: buildSectionVerdicts(CLEAN_REPORT, DOCUMENT),
    fabrications: [PENDING],
    ...overrides,
  };
}

export function panelElement(review: QualityPipelineReview, report = CLEAN_REPORT) {
  return (
    <QualityReportPanel open onClose={vi.fn()} report={report} docKind="resume" pipeline={review} />
  );
}

export function renderPanel(review: QualityPipelineReview, report = CLEAN_REPORT) {
  return render(panelElement(review, report));
}
