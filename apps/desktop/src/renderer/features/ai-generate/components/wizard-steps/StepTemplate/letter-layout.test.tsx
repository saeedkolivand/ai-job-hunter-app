// Cover-letter layout picker and the shared-atsMode round-trips.
import { useState } from 'react';
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { LetterLayoutId } from '@/lib/generate';

import { StepTemplate } from './index';
import { letterOption, renderStepWith } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./stubs')).translationsMock);
vi.mock('../../../samples', async () => (await import('./stubs')).samplesMock());

describe('StepTemplate', () => {
  let onTemplateChange: Mock;
  let onAtsModeChange: Mock;
  const renderStep = (props: Parameters<typeof renderStepWith>[2] = {}) =>
    renderStepWith(onTemplateChange, onAtsModeChange, props);

  beforeEach(() => {
    onTemplateChange = vi.fn();
    onAtsModeChange = vi.fn();
  });

  // ── letter layout picker (cover-only) ───────────────────────────────────────

  it('shows the letter layout picker in cover mode but not in résumé mode', () => {
    const { rerender } = renderStep({
      templateId: 'classic',
      atsMode: false,
      target: 'cover',
      onLetterLayoutChange: vi.fn(),
    });
    expect(screen.getByTestId(letterOption('classic'))).toBeInTheDocument();

    rerender(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="resume"
        onLetterLayoutChange={vi.fn()}
      />
    );
    expect(screen.queryByTestId(letterOption('classic'))).not.toBeInTheDocument();
  });

  it("also shows the letter layout picker for target='both' — the primary flow produces a cover letter too", () => {
    renderStep({
      templateId: 'classic',
      atsMode: false,
      target: 'both',
      onLetterLayoutChange: vi.fn(),
    });
    expect(screen.getByTestId(letterOption('classic'))).toBeInTheDocument();
  });

  it('forwards a layout pick to onLetterLayoutChange in cover mode', async () => {
    const user = userEvent.setup();
    const onLetterLayoutChange = vi.fn();
    renderStep({
      templateId: 'classic',
      atsMode: false,
      target: 'cover',
      onLetterLayoutChange,
    });
    await user.click(screen.getByTestId(letterOption('banded')));
    expect(onLetterLayoutChange).toHaveBeenCalledWith('banded');
  });

  // ── layout change releases the shared flag (round-trip) ──────────────────────

  it('clears atsMode when the letter drops to an undecorated layout and nothing else reads it', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'classic', // ATS-tier → the flag is a no-op for the résumé
      target: 'both',
      letterLayoutId: 'monogram',
      atsMode: true,
      onLetterLayoutChange: vi.fn(),
    });
    await user.click(screen.getByTestId(letterOption('classic')));
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  it('KEEPS atsMode on the same change when a design-tier résumé template still reads it', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'atelier', // design-tier → the résumé genuinely uses the flag
      target: 'both',
      letterLayoutId: 'monogram',
      atsMode: true,
      onLetterLayoutChange: vi.fn(),
    });
    await user.click(screen.getByTestId(letterOption('classic')));
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });

  // The full round-trip against real host state — the bug was that step 3 came
  // back ON, so a freshly-picked Monogram exported with no monogram.
  function StatefulStep({ templateId }: { templateId: 'classic' | 'atelier' }) {
    const [atsMode, setAtsMode] = useState(false);
    const [letterLayoutId, setLetterLayoutId] = useState<LetterLayoutId | undefined>(undefined);
    return (
      <StepTemplate
        templateId={templateId}
        atsMode={atsMode}
        onTemplateChange={vi.fn()}
        onAtsModeChange={setAtsMode}
        target="both"
        letterLayoutId={letterLayoutId}
        onLetterLayoutChange={setLetterLayoutId}
      />
    );
  }

  it('monogram → ATS on → classic → monogram: the toggle comes back OFF', async () => {
    const user = userEvent.setup();
    render(<StatefulStep templateId="classic" />);

    await user.click(screen.getByTestId(letterOption('monogram')));
    await user.click(screen.getByRole('switch'));
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');

    await user.click(screen.getByTestId(letterOption('classic')));
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();

    await user.click(screen.getByTestId(letterOption('monogram')));
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  });

  // target='cover': the picked template only supplies the letter's palette — no
  // résumé is exported from it, so a design-tier id must NOT hold the flag open.
  // This was the stranded case: Atelier + Monogram + ATS on → Classic → stuck.
  function StatefulCoverStep() {
    const [atsMode, setAtsMode] = useState(false);
    const [letterLayoutId, setLetterLayoutId] = useState<LetterLayoutId | undefined>(undefined);
    return (
      <StepTemplate
        templateId="atelier"
        atsMode={atsMode}
        onTemplateChange={vi.fn()}
        onAtsModeChange={setAtsMode}
        target="cover"
        letterLayoutId={letterLayoutId}
        onLetterLayoutChange={setLetterLayoutId}
      />
    );
  }

  it("target='cover': monogram → ATS on → classic → monogram comes back OFF, design-tier template notwithstanding", async () => {
    const user = userEvent.setup();
    render(<StatefulCoverStep />);

    await user.click(screen.getByTestId(letterOption('monogram')));
    await user.click(screen.getByRole('switch'));
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');

    await user.click(screen.getByTestId(letterOption('classic')));
    // No résumé in this run, so nothing is left to read the flag: the switch is
    // gone (Atelier is irrelevant here) and the flag went with it.
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();

    await user.click(screen.getByTestId(letterOption('monogram')));
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  });

  it('same round-trip under a design-tier template KEEPS the flag on (the résumé owns it)', async () => {
    const user = userEvent.setup();
    render(<StatefulStep templateId="atelier" />);

    await user.click(screen.getByTestId(letterOption('monogram')));
    await user.click(screen.getByRole('switch'));

    await user.click(screen.getByTestId(letterOption('classic')));
    // The switch stays — the résumé still linearizes under it — and stays ON.
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');

    await user.click(screen.getByTestId(letterOption('monogram')));
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  });
});
