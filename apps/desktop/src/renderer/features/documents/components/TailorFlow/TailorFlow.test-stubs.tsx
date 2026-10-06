/**
 * Stub child components for the TailorFlow tests, wired in by `TailorFlow.test-support.tsx`.
 * The stubs are purposely @ajh/ui-free (div[role=button]) to stay inside the no-raw-button
 * ESLint rule for test files. They must NEVER import the component under test.
 */
import { TEST_IDS } from '@ajh/test-ids';

// TailorWizard stub exposes:
//   - a "next-step" button → calls setStep(step + 1), exercising handleStep →
//     persistForm → persistence.setWizardForm + persistence.setWizardStep.
//   - a "generate" button → calls onGenerate({ resume, outputType, researchCompany }),
//     exercising startGeneration → persistForm → persistence.setWizardForm.
// The stub is purposely @ajh/ui-free (uses div[role=button]) to stay inside
// the no-raw-button ESLint rule for test files.
export const wizardModule = {
  TailorWizard: ({
    step,
    setStep,
    onGenerate,
    jobDesc,
    onJobDescChange,
    methods,
    resumeId,
  }: {
    step: number;
    setStep: (n: number) => void;
    onGenerate: (v: { resume: string; outputType: 'resume'; researchCompany: boolean }) => void;
    jobDesc?: string;
    onJobDescChange?: (v: string) => void;
    // The RHF form — the stub reads the research toggle so the capability-driven
    // default is observable via a data attribute.
    methods: { watch: (name: 'researchCompany') => boolean };
    /** What the Score tab will actually score — see TailorFlow's `resumeId`. */
    resumeId?: string;
  }) => (
    <div
      data-testid={TEST_IDS.documents.tailorWizard}
      data-step={step}
      data-jobdesc={jobDesc}
      data-research={String(methods.watch('researchCompany'))}
      data-resumeid={resumeId ?? ''}
    >
      <div
        role="button"
        tabIndex={0}
        data-testid={TEST_IDS.documents.wizardNext}
        onClick={() => setStep(step + 1)}
      >
        next-step
      </div>
      <div
        role="button"
        tabIndex={0}
        data-testid={TEST_IDS.documents.wizardGenerate}
        onClick={() =>
          onGenerate({ resume: 'my-resume', outputType: 'resume', researchCompany: false })
        }
      >
        generate
      </div>
      <div
        role="button"
        tabIndex={0}
        data-testid="wizard-edit-jobdesc"
        onClick={() => onJobDescChange?.('edited-job-ad')}
      >
        edit-jobdesc
      </div>
    </div>
  ),
};

export const generatingPanelModule = {
  // `streamingTarget` is surfaced as a data attribute: the panel's own
  // rendering of it is `GeneratingPanel.test.tsx`'s job — what belongs HERE is
  // which value TailorFlow hands it, which is a decision this component makes.
  GeneratingPanel: ({ streamingTarget }: { streamingTarget: 'resume' | 'cover' }) => (
    <div data-testid={TEST_IDS.documents.generatingPanel} data-streaming={streamingTarget} />
  ),
};

export const resultsPanelModule = {
  // div[role=button] avoids the no-raw-button ESLint rule while remaining
  // clickable via userEvent.click — stubs in test files only, no production code.
  ResultsPanel: ({
    onEditSettings,
    onTemplateChange,
    onAtsModeChange,
    templateId,
    atsMode,
    market,
  }: {
    onEditSettings?: () => void;
    onTemplateChange?: (v: string) => void;
    onAtsModeChange?: (v: boolean) => void;
    templateId?: string;
    atsMode?: boolean;
    market?: string;
  }) => (
    <div
      data-testid={TEST_IDS.documents.resultsPanel}
      data-templateid={templateId}
      data-atsmode={String(atsMode)}
      data-market={market ?? ''}
    >
      <div role="button" tabIndex={0} onClick={onEditSettings}>
        edit-settings
      </div>
      <div role="button" tabIndex={0} onClick={() => onTemplateChange?.('classic')}>
        change-template
      </div>
      <div role="button" tabIndex={0} onClick={() => onAtsModeChange?.(true)}>
        toggle-ats
      </div>
    </div>
  ),
};

/** A modal stub: a marker plus a "close-<name>" button wired to `onClose`. */
const modalStub = (testId: string, name: string) =>
  function ModalStub({ onClose }: { onClose: () => void }) {
    return (
      <div data-testid={testId}>
        <div role="button" tabIndex={0} onClick={onClose}>
          close-{name}
        </div>
      </div>
    );
  };

export const questionsModalModule = {
  ApplicationQuestionsModal: modalStub(TEST_IDS.documents.questionsModal, 'questions'),
};
export const interviewModalModule = {
  InterviewQuestionsModal: modalStub(TEST_IDS.documents.interviewModal, 'interview'),
};
export const referralModalModule = {
  ReferralModal: modalStub(TEST_IDS.documents.referralModal, 'referral'),
};
