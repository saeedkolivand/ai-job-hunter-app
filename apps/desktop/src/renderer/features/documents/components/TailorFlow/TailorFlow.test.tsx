/**
 * TailorFlow — extraction seams: stage derivation, persistence injection, capability-driven research default.
 * Shared mocks, state and helpers live in `test-support.tsx` (see its header).
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import {
  genMock,
  makePersistence,
  modelCapsState,
  renderFlow,
  rerenderFlow,
  resetState,
} from './TailorFlow.test-support';

beforeEach(resetState);

const { tailorWizard, generatingPanel, resultsPanel } = TEST_IDS.documents;
const wizard = () => screen.getByTestId(tailorWizard);
const expectPresent = (shown: string, ...hidden: string[]) => {
  expect(screen.getByTestId(shown)).toBeInTheDocument();
  for (const id of hidden) expect(screen.queryByTestId(id)).not.toBeInTheDocument();
};

// ─────────────────────────────────────────────────────────────────────────────
// 1. Stage derivation
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — stage derivation', () => {
  it('renders the wizard (configuring) when not busy and no output', () => {
    renderFlow({});
    expectPresent(tailorWizard, generatingPanel, resultsPanel);
  });

  // A cover-only run skips the `draft` stage entirely, so there is no résumé
  // stream to precede the letter's first token — `letterDraft ? 'cover' :
  // 'resume'` alone labelled the pane "Resume" for the whole analyze → strategy
  // warm-up of a run that produces no résumé at all.
  it.each([
    [
      'labels the streaming pane Cover letter for a cover-only run before the first token',
      'cover',
      'cover',
    ],
    ['still labels it Resume for a run that produces one', 'both', 'resume'],
  ] as const)('%s', (_name, outputType, label) => {
    genMock.busy = true;
    genMock.letterDraft = '';
    genMock.draft = '';
    renderFlow({
      persistence: makePersistence({
        wizardForm: { resume: 'r', outputType, researchCompany: false },
      }),
    });
    expect(screen.getByTestId(generatingPanel)).toHaveAttribute('data-streaming', label);
  });

  it('renders the generating panel when busy=true (no output)', () => {
    genMock.busy = true;
    renderFlow({});
    expectPresent(generatingPanel, tailorWizard, resultsPanel);
  });

  it('renders the results panel when hasOutput is true and not busy', () => {
    genMock.hasOutput = true;
    renderFlow({});
    expectPresent(resultsPanel, tailorWizard, generatingPanel);
  });

  it('busy=true WINS over existing output (generating stage takes priority)', () => {
    genMock.busy = true;
    genMock.hasOutput = true;
    renderFlow({});
    expectPresent(generatingPanel, resultsPanel);
  });

  it('clicking "edit-settings" from done stage reverts to the wizard (forceConfiguring)', async () => {
    genMock.hasOutput = true;
    const user = userEvent.setup();
    renderFlow({});

    // We are in done stage — results panel visible.
    expect(screen.getByTestId(resultsPanel)).toBeInTheDocument();

    // The stubbed ResultsPanel exposes an edit-settings button that calls onEditSettings.
    await user.click(screen.getByRole('button', { name: 'edit-settings' }));

    // After clicking, TailorFlow sets forceConfiguring → wizard shown.
    expectPresent(tailorWizard, resultsPanel);

    // Output is preserved under forceConfiguring — the mock's own hasOutput stays.
    expect(genMock.hasOutput).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. Persistence injection — host-agnostic contract
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — persistence injection', () => {
  it('reads wizardStep from the injected persistence and forwards it to TailorWizard', () => {
    // The stub renders `data-step={step}` so we can assert the value was forwarded.
    renderFlow({ persistence: makePersistence({ wizardStep: 2 }) });
    expect(wizard()).toHaveAttribute('data-step', '2');
  });

  it('reads wizardForm from persistence to seed the RHF defaultValues (non-null form)', () => {
    // When wizardForm is set, TailorFlow uses it as the one-shot seed.
    // Deeper RHF seed verification requires un-stubbing TailorWizard (overkill here;
    // covered by ApplyPage.test.tsx persistence round-trip tests).
    const persistence = makePersistence({
      wizardForm: { resume: 'Seeded resume', outputType: 'resume', researchCompany: false },
    });
    renderFlow({ persistence });
    expect(wizard()).toBeInTheDocument();
  });

  it('calls persistence.setWizardForm AND persistence.setWizardStep when advancing a step', async () => {
    // GAP 1 FIX: TailorWizard stub exposes a "next-step" button that calls
    // setStep(step + 1). TailorFlow's handleStep() calls persistForm() first
    // (→ persistence.setWizardForm) then setStep() (→ persistence.setWizardStep).
    // Clicking "next-step" exercises the full write-back path.
    const user = userEvent.setup();
    const persistence = makePersistence({ wizardStep: 0 });
    renderFlow({ persistence });

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardNext));

    // setWizardForm is called with the current RHF values (persistForm snapshot).
    expect(persistence.setWizardForm).toHaveBeenCalledTimes(1);
    // setWizardStep is called with the next step number.
    expect(persistence.setWizardStep).toHaveBeenCalledTimes(1);
    expect(persistence.setWizardStep).toHaveBeenCalledWith(1);
  });

  it('calls persistence.setWizardForm and gen.start when the user clicks generate', async () => {
    // startGeneration calls persistForm() before launching gen.start.
    // The "generate" button in the TailorWizard stub calls onGenerate({ ... }).
    const user = userEvent.setup();
    const persistence = makePersistence();
    renderFlow({ persistence });

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

    // persistForm is always called before generation starts.
    expect(persistence.setWizardForm).toHaveBeenCalledTimes(1);
    // setWizardStep is NOT called by startGeneration (only by handleStep).
    expect(persistence.setWizardStep).not.toHaveBeenCalled();
    // gen.start is invoked.
    expect(genMock.start).toHaveBeenCalledTimes(1);
  });

  it('reads templateId and atsMode from persistence and passes them to ResultsPanel when done', () => {
    // GAP 2 FIX: drive the "done" stage so ResultsPanel renders, then assert the
    // persistence values were forwarded as props (rendered as data-* attributes by
    // the stub). This proves TailorFlow reads them from persistence, not constants.
    genMock.hasOutput = true;
    renderFlow({ persistence: makePersistence({ templateId: 'classic', atsMode: true }) });

    const panel = screen.getByTestId(resultsPanel);
    expect(panel).toHaveAttribute('data-templateid', 'classic');
    expect(panel).toHaveAttribute('data-atsmode', 'true');
  });

  // Regression guard: `gen.market` (resolved by `useTailorPipeline` from the job
  // description's detected language) used to never reach the live preview — the
  // Rust exporter falls back to market "intl" on an unset value, so a German
  // posting showed an English salutation on screen but a German one in the
  // downloaded PDF/DOCX. Asserts the FORWARDED value, not just that a render
  // happened, so dropping this prop again fails the test.
  it('forwards gen.market to ResultsPanel as `market` (not undefined) when done', () => {
    genMock.hasOutput = true;
    genMock.market = 'de';
    renderFlow({});

    expect(screen.getByTestId(resultsPanel)).toHaveAttribute('data-market', 'de');
  });

  // GAP 2 FIX: the ResultsPanel stub exposes "change-template" / "toggle-ats"
  // buttons that call onTemplateChange('classic') / onAtsModeChange(true).
  it.each([
    [
      'calls persistence.setTemplateId when ResultsPanel fires onTemplateChange',
      'change-template',
      { templateId: 'swiss-minimal' },
      'setTemplateId',
      'classic',
    ],
    [
      'calls persistence.setAtsMode when ResultsPanel fires onAtsModeChange',
      'toggle-ats',
      { atsMode: false },
      'setAtsMode',
      true,
    ],
  ] as const)('%s', async (_name, button, overrides, setter, value) => {
    const user = userEvent.setup();
    genMock.hasOutput = true;
    const persistence = makePersistence(overrides);
    renderFlow({ persistence });

    await user.click(screen.getByRole('button', { name: button }));

    expect(persistence[setter]).toHaveBeenCalledTimes(1);
    expect(persistence[setter]).toHaveBeenCalledWith(value);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2b. Capability-driven "search company" default
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — capability-driven research default', () => {
  const expectResearch = (value: string) =>
    expect(wizard()).toHaveAttribute('data-research', value);

  it.each([
    ['defaults the research toggle ON for a web-search-capable model (fresh form)', true, 'true'],
    [
      'defaults the research toggle OFF for a model without web search (fresh form)',
      false,
      'false',
    ],
  ])('%s', (_name, supportsWebSearch, expected) => {
    modelCapsState.data = { supportsWebSearch };
    renderFlow({ persistence: makePersistence({ wizardForm: null }) });
    expectResearch(expected);
  });

  it('does NOT override a restored form — the saved choice wins over the capability default', () => {
    // Capability says ON, but the persisted form saved OFF → the restore wins.
    modelCapsState.data = { supportsWebSearch: true };
    const persistence = makePersistence({
      wizardForm: { resume: 'Seeded', outputType: 'both', researchCompany: false },
    });
    renderFlow({ persistence });
    expectResearch('false');
  });

  it('re-seeds the toggle when the model changes mid-session (fresh form, untouched)', () => {
    modelCapsState.data = { supportsWebSearch: false };
    const persistence = makePersistence({ wizardForm: null });
    // A fresh element per render so React reconciles (identical element refs bail).
    const { rerender } = render(rerenderFlow(persistence));
    expectResearch('false');

    // User switches to a web-search-capable model without touching the toggle.
    modelCapsState.data = { supportsWebSearch: true };
    rerender(rerenderFlow(persistence));
    expectResearch('true');
  });
});
