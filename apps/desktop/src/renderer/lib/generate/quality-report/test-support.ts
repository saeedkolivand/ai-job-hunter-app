import type { vi } from 'vitest';

import type { ContentReportPayload, PipelineQualityReport } from '@ajh/shared/ipc';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';
import { hashText } from '../quality-report';

export const OK_REPORT: ContentReportPayload = {
  ok: true,
  issues: [],
  metrics: {
    keywordCoverage: 80,
    topRequirementHits: 2,
    topRequirementsMeasured: 4,
    duplicateRatio: 0,
    rolesSource: 3,
    rolesOutput: 3,
  },
};

export const CRITICAL_REPORT: ContentReportPayload = {
  ok: false,
  issues: [
    {
      severity: 'critical',
      code: 'factual.dropped_role',
      section: 'Experience',
      message: 'A role from the source résumé is missing.',
      evidence: 'Acme Corp',
    },
  ],
  metrics: {
    keywordCoverage: 40,
    topRequirementHits: 0,
    topRequirementsMeasured: 0,
    duplicateRatio: 0,
    rolesSource: 3,
    rolesOutput: 2,
  },
};

/**
 * What the staged Rust pipeline persists into the SAME `ai_generations` row the
 * fast path reads and rewrites: depth `'quality'`, plus a slot carrying the
 * per-bullet fabrication review — one bullet the user already ruled on, one
 * still awaiting a verdict (which is what holds the run at `needsReview`).
 */
export const QUALITY_WRAPPER: PipelineQualityReport = {
  schemaVersion: 2,
  pipeline: 'quality',
  generatedAt: 1700,
  resume: {
    report: CRITICAL_REPORT,
    sourceTextHash: hashText('generated resume'),
    fabrications: [
      {
        issueKey: 'factual.unsupported_metric#0',
        code: 'factual.unsupported_metric',
        evidence: 'Cut p99 latency by 60%',
        decision: 'keep',
      },
      {
        issueKey: 'factual.unsupported_metric#1',
        code: 'factual.unsupported_metric',
        evidence: 'Led a team of 12',
      },
    ],
  },
  coverLetter: { report: OK_REPORT, sourceTextHash: hashText('generated cover') },
};

export function register(validateContent: ReturnType<typeof vi.fn>) {
  _registerClient(createMockClient({ resume: { validateContent } }));
}
