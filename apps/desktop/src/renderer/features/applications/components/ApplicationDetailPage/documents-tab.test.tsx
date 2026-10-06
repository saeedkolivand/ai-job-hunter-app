/**
 * ApplicationDetailPage — Documents tab: generation matching, apply-run isolation, toolbar, résumé seeding
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { ApplicationDetailPage, renderLoaded } from './test-render';
import {
  makeApp,
  makeGen,
  mockSessionState,
  mockUseAiGenerations,
  setLoaded,
  state,
} from './test-support';

beforeEach(() => {
  state.tab = 'documents';
});

describe('ApplicationDetailPage — generation matching (Documents tab)', () => {
  it('does NOT render a saved-generations list even when a generation matches (list removed)', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    setLoaded(
      app,
      [],
      [
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1', applicationId: 'app-1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x', applicationId: 'app-2' }),
      ]
    );

    render(<ApplicationDetailPage />);

    // The Documents tab is now a full-height host for TailorFlow (mirrors the
    // autopilot apply flow); the previously-saved generations list was removed.
    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toBeInTheDocument();
  });

  it('shows no saved GenerationCard when no generation matches', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    setLoaded(
      app,
      [],
      [
        makeGen({
          id: 'gen-x',
          jobUrl: 'https://different.com/job/99',
          applicationId: 'app-other',
        }),
      ]
    );

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
    // TailorFlow still mounts so the user can generate inline.
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toBeInTheDocument();
  });

  it('passes the matching generation to TailorFlow as the seedGeneration (cold-entry source)', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    setLoaded(
      app,
      [],
      [
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1', applicationId: 'app-1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x', applicationId: 'app-2' }),
      ]
    );

    render(<ApplicationDetailPage />);

    // Only gen-1 carries this application's FK → it seeds the flow.
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute(
      'data-seedgenid',
      'gen-1'
    );
  });

  it('passes no seedGeneration to TailorFlow when nothing matches', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    setLoaded(
      app,
      [],
      [
        makeGen({
          id: 'gen-x',
          jobUrl: 'https://different.com/job/99',
          applicationId: 'app-other',
        }),
      ]
    );

    render(<ApplicationDetailPage />);

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute('data-seedgenid', '');
  });

  it('does NOT match a generation whose applicationId differs from this application', () => {
    // Docs join by the `applicationId` FK, not by url — a generation linked to a
    // DIFFERENT Application (or unlinked) must not surface here, even if its url
    // happens to match.
    const app = makeApp({ id: 'app-1' });

    setLoaded(
      app,
      [],
      [makeGen({ id: 'gen-z', jobUrl: 'https://acme.com/job/1', applicationId: 'app-other' })]
    );

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// F2 regression — a stale `applyRun` from another application must never
// reach TailorFlow as a reconnect target (cross-application run leak).
//
// Both apply entry points (`usePostingActions.handleTailor`,
// `AutopilotPage.handleApply`) set `applyForId` to the NEW application's id
// BEFORE navigating, with no compensating clear of the reconnect target — so
// the reset effect's `applyForId !== application.id` guard can already be
// FALSE by the time this application's DocumentsTab first renders (and even
// where it isn't, this tab is the wizard's DEFAULT tab, so it mounts in the
// SAME commit as the reset effect that would otherwise clear it — a classic
// `useState` lazy-initializer race). Exercised here directly at the render
// level: `applicationApply.applyRun.forId` disagreeing with the CURRENT
// application id, regardless of how that came to be, must read as "no run".
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — F2: applyRun is read only for the CURRENT application', () => {
  afterEach(() => {
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyRun: null,
    };
  });

  it('does NOT hand another application’s run id to TailorFlow (A→B leak)', () => {
    // Simulates the exact hazard: `applyForId` already matches THIS
    // application (set atomically by an apply entry point, or simply because
    // the reset effect already ran) while `applyRun` still points at a
    // DIFFERENT application's run — the shape a naive `applyRunId` field
    // could leak, and the self-describing `forId` gate must reject.
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyForId: 'app-1',
      applyRun: { forId: 'app-OTHER', runId: 'run-OTHER', jobId: 'job-OTHER' },
    };
    const app = makeApp({ id: 'app-1' });
    setLoaded(app);

    render(<ApplicationDetailPage />);

    const flow = screen.getByTestId(TEST_IDS.documents.tailorFlow);
    expect(flow).toHaveAttribute('data-runid', '');
    expect(flow).toHaveAttribute('data-runjobid', '');
  });

  it('DOES hand this application’s own run id to TailorFlow (happy path)', () => {
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyForId: 'app-1',
      applyRun: { forId: 'app-1', runId: 'run-1', jobId: 'job-1' },
    };
    const app = makeApp({ id: 'app-1' });
    setLoaded(app);

    render(<ApplicationDetailPage />);

    const flow = screen.getByTestId(TEST_IDS.documents.tailorFlow);
    expect(flow).toHaveAttribute('data-runid', 'run-1');
    expect(flow).toHaveAttribute('data-runjobid', 'job-1');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — Documents tab toolbar
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — Documents tab toolbar', () => {
  it('renders the TailorFlow stub on the Documents tab', () => {
    renderLoaded();
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toBeInTheDocument();
  });

  it('does NOT render the Questions button when controller stage is not "done"', () => {
    // onController is never called because TailorFlow is stubbed (never fires).
    // controller stays null → Questions button is hidden.
    renderLoaded();
    // The referral button IS always visible; questions button only shows on done.
    expect(
      screen.queryByRole('button', { name: /autopilot\.apply\.questions\.title/i })
    ).not.toBeInTheDocument();
    // Referral button always present.
    expect(screen.getByRole('button', { name: /autopilot\.referral\.open/i })).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — seedResumeDocId (Score tab résumé identity)
//
// `seedResumeText` has THREE fallbacks (autopilot one-shot → default résumé
// text → previous generation's output); only the middle one has a saved-
// document backing. The id must be seeded ONLY when the visible text IS that
// document's text — an id that doesn't match the seeded text is the exact
// drift `useResumeInput`'s `selectDoc` contract exists to prevent (see
// index.tsx's `seedResumeDocId` comment).
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — seedResumeDocId (Score tab résumé identity)', () => {
  it("seeds the default résumé id when the seeded text IS that résumé's text (matched branch)", () => {
    state.docsData = [{ _id: 'doc-1', name: 'Resume.pdf', isDefault: true }];
    state.documentText = 'Default resume text';
    mockSessionState.applicationApply.applySeedResume = null; // no autopilot one-shot
    mockUseAiGenerations.mockReturnValue({ data: [] }); // no generation fallback needed

    renderLoaded({ id: 'app-1' });

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute(
      'data-resumedocid',
      'doc-1'
    );
  });

  it('does NOT seed a résumé id when the text came from the autopilot one-shot seed', () => {
    state.docsData = [{ _id: 'doc-1', name: 'Resume.pdf', isDefault: true }];
    // Even though a default résumé (with its OWN text) exists, the one-shot
    // seed wins priority — an id backing DIFFERENT text must never be seeded.
    state.documentText = 'Default resume text';
    mockSessionState.applicationApply.applySeedResume = 'Autopilot one-shot resume text';
    mockUseAiGenerations.mockReturnValue({ data: [] });

    renderLoaded({ id: 'app-1' });

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute(
      'data-resumedocid',
      ''
    );
  });

  it('strips a persisted résumé id whose document no longer exists', () => {
    // `resume_source` is ID-WINS with NO fallback: a stale id fails the whole
    // run with "resume not found" while the résumé text sits visible on screen.
    state.docsData = [{ _id: 'doc-alive', name: 'Resume.pdf', isDefault: true }];
    state.documentText = 'Default resume text';
    mockSessionState.applicationApply.applySeedResume = null;
    mockSessionState.applicationApply.applyWizardForm = {
      resume: 'Some resume text',
      outputType: 'both',
      researchCompany: false,
      resumeDocId: 'doc-deleted',
    };
    mockUseAiGenerations.mockReturnValue({ data: [] });

    renderLoaded({ id: 'app-1' });

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute('data-formdocid', '');
  });

  it('keeps a persisted résumé id whose document still exists', () => {
    state.docsData = [{ _id: 'doc-alive', name: 'Resume.pdf', isDefault: true }];
    state.documentText = 'Default resume text';
    mockSessionState.applicationApply.applySeedResume = null;
    mockSessionState.applicationApply.applyWizardForm = {
      resume: 'Some resume text',
      outputType: 'both',
      researchCompany: false,
      resumeDocId: 'doc-alive',
    };
    mockUseAiGenerations.mockReturnValue({ data: [] });

    renderLoaded({ id: 'app-1' });

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute(
      'data-formdocid',
      'doc-alive'
    );
  });

  it("does NOT seed a résumé id when the text came from a previous generation's output", () => {
    // A default résumé DOES exist, so `defaultResumeId` is a real id and the
    // `?? undefined` fallback cannot carry this test on its own — the equality
    // guard is the only thing preventing the seed. Its text is EMPTY, which is
    // what lets the `||` chain fall through to the generation's text: a
    // non-empty default text would win the chain and legitimately seed the id.
    state.docsData = [{ _id: 'doc-1', name: 'Resume.pdf', isDefault: true }];
    state.documentText = '';
    mockSessionState.applicationApply.applySeedResume = null;
    // `renderLoaded` always overwrites the generations mock to `{ data: [] }`
    // (it's the "no matching generation" default for every OTHER test in this
    // file) — call the two mocks + render directly instead, mirroring the
    // "generation matching" describe block above.
    const app = makeApp({ id: 'app-1', jobUrl: 'https://acme.com/job/1' });
    setLoaded(
      app,
      [],
      [
        {
          ...makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1', applicationId: 'app-1' }),
          resumeText: 'Previous generation resume text',
        },
      ]
    );

    render(<ApplicationDetailPage />);

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute(
      'data-resumedocid',
      ''
    );
  });
});
