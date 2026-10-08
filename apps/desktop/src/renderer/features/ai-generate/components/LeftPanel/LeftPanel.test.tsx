import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { LeftPanel } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/components/job/JobAdField', () => ({
  JobAdField: ({ placeholder }: { placeholder: string }) => (
    <div data-testid="job-ad-field">{placeholder}</div>
  ),
}));
vi.mock('@/components/resume/ResumeInputCard', () => ({ ResumeInputCard: () => null }));
vi.mock('@/components/ui/AiSetupHint', () => ({ AiSetupHint: () => null }));
vi.mock('@/components/ui/ModelSelector', () => ({ ModelSelector: () => null }));
vi.mock('@/features/ai-generate/components/GenerationMetadata', () => ({
  GenerationMetadata: () => null,
}));
vi.mock('@/features/ai-generate/components/TemplateRecommendation', () => ({
  TemplateRecommendation: () => null,
}));

describe('LeftPanel', () => {
  it('gives the job-ad field its own placeholder, not the generated-output one', () => {
    render(
      <LeftPanel
        resume=""
        jobAd=""
        stage="idle"
        meta={null}
        templateId="classic"
        uploading={null}
        uploadError={null}
        canGenerate={false}
        canUseAI
        aiReason=""
        canProceed={false}
        setResume={vi.fn()}
        setJobAd={vi.fn()}
        onJobAdImport={vi.fn()}
        setTemplateId={vi.fn()}
        setAtsMode={vi.fn()}
        setLocale={vi.fn()}
        onUpload={vi.fn()}
        onReset={vi.fn()}
        onAnalyze={vi.fn()}
      />
    );

    expect(screen.getByTestId('job-ad-field')).toHaveTextContent('aiGenerate.jobAdPlaceholder');
  });
});
