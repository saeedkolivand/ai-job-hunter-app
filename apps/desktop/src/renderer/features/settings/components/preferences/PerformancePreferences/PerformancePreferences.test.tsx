/**
 * PerformancePreferences — custom performance mode UI tests.
 *
 * Covers:
 *  1. All 4 mode cards render (Low Memory, Balanced, Performance, Custom).
 *  2. Selecting the Custom card:
 *     - calls setPerformanceMode('custom').
 *     - seeds customPerformance from the balanced preset (setCustomPerformance
 *       called with balanced) when no custom profile exists yet.
 *     - does NOT re-seed if a custom profile already exists.
 *  3. Custom sub-panel appears when mode='custom'.
 *  4. Toggling a visual Switch (role="switch") calls setCustomPerformance with
 *     the updated field.
 *  5. Changing a backend Dropdown calls setCustomPerformance with the updated
 *     field.
 *  6. The 4 dropdowns are labeled via <label htmlFor> → Dropdown id.
 *
 * Accessibility notes:
 *  - Mode cards are `motion.button` → `role="button"` (implicit).
 *  - Switches are `role="switch"` (explicit on the Button inside Switch).
 *  - Dropdowns render a trigger `<button>` — query scoped to the sub-panel
 *    `div` to avoid collisions with mode-card buttons.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import {
  PERFORMANCE_PRESETS,
  type PerformanceMode,
  type PerformanceProfile,
} from '@/store/preferences-schema';

// ── store mock ────────────────────────────────────────────────────────────────

const mockSetPerformanceMode = vi.fn();
const mockSetCustomPerformance = vi.fn();

let mockPerformanceMode: PerformanceMode = 'balanced';
let mockProfile: PerformanceProfile = PERFORMANCE_PRESETS.balanced;
let mockCustomPerformance: PerformanceProfile | undefined = undefined;

vi.mock('@/store/preferences-store', () => ({
  usePerformanceMode: () => mockPerformanceMode,
  useResolvedPerformanceProfile: () => mockProfile,
  usePreferencesStore: (
    selector: (s: {
      setPerformanceMode: typeof mockSetPerformanceMode;
      setCustomPerformance: typeof mockSetCustomPerformance;
      customPerformance: PerformanceProfile | undefined;
    }) => unknown
  ) =>
    selector({
      setPerformanceMode: mockSetPerformanceMode,
      setCustomPerformance: mockSetCustomPerformance,
      customPerformance: mockCustomPerformance,
    }),
}));

// ── import component AFTER mocks ──────────────────────────────────────────────

import { PerformancePreferences } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderPreferences() {
  return render(<PerformancePreferences />);
}

/** Returns the custom sub-panel container when it is rendered. */
function getCustomPanel(container: HTMLElement): HTMLElement {
  // The sub-panel is the div.space-y-5 rendered when performanceMode === 'custom'.
  // It contains both the "Visual" section label and the switch/dropdown controls.
  const panel = container.querySelector<HTMLElement>('.space-y-5');
  if (!panel) throw new Error('custom sub-panel not found — is performanceMode set to "custom"?');
  return panel;
}

// ── reset ──────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockSetPerformanceMode.mockClear();
  mockSetCustomPerformance.mockClear();
  mockPerformanceMode = 'balanced';
  mockProfile = PERFORMANCE_PRESETS.balanced;
  mockCustomPerformance = undefined;
});

// ── tests ─────────────────────────────────────────────────────────────────────

describe('PerformancePreferences — mode card rendering', () => {
  it('renders all four mode cards', () => {
    renderPreferences();
    expect(screen.getByText('Low Memory')).toBeInTheDocument();
    expect(screen.getByText('Balanced')).toBeInTheDocument();
    expect(screen.getByText('Performance')).toBeInTheDocument();
    expect(screen.getByText('Custom')).toBeInTheDocument();
  });
});

describe('PerformancePreferences — selecting the Custom card', () => {
  it('calls setPerformanceMode with "custom" when the Custom card is clicked', async () => {
    const user = userEvent.setup();
    renderPreferences();
    await user.click(screen.getByText('Custom'));
    expect(mockSetPerformanceMode).toHaveBeenCalledWith('custom');
  });

  it('seeds customPerformance from balanced preset on first Custom selection (no existing custom)', async () => {
    const user = userEvent.setup();
    mockCustomPerformance = undefined;
    renderPreferences();
    await user.click(screen.getByText('Custom'));
    expect(mockSetCustomPerformance).toHaveBeenCalledTimes(1);
    const call = mockSetCustomPerformance.mock.calls.at(0);
    if (!call) throw new Error('expected setCustomPerformance to have been called');
    const calledWith = call[0] as PerformanceProfile;
    expect(calledWith).toEqual(PERFORMANCE_PRESETS.balanced);
  });

  it('does NOT call setCustomPerformance when a custom profile already exists', async () => {
    const user = userEvent.setup();
    mockCustomPerformance = { ...PERFORMANCE_PRESETS.balanced };
    renderPreferences();
    await user.click(screen.getByText('Custom'));
    expect(mockSetCustomPerformance).not.toHaveBeenCalled();
  });
});

describe('PerformancePreferences — custom sub-panel', () => {
  beforeEach(() => {
    mockPerformanceMode = 'custom';
    mockProfile = { ...PERFORMANCE_PRESETS.balanced };
    mockCustomPerformance = { ...PERFORMANCE_PRESETS.balanced };
  });

  it('shows the custom sub-panel when mode=custom', () => {
    renderPreferences();
    expect(screen.getByText('Visual')).toBeInTheDocument();
    expect(screen.getByText('Backend')).toBeInTheDocument();
  });

  it('does NOT show the custom sub-panel when mode=balanced', () => {
    mockPerformanceMode = 'balanced';
    mockProfile = PERFORMANCE_PRESETS.balanced;
    renderPreferences();
    // "Visual" section label only appears in the custom sub-panel.
    expect(screen.queryByText('Visual')).not.toBeInTheDocument();
  });

  describe('visual switches (role="switch")', () => {
    it('calls setCustomPerformance with aurora=false when the Aurora switch is toggled off', async () => {
      // Profile has aurora=true (balanced).
      mockProfile = { ...PERFORMANCE_PRESETS.balanced };
      const user = userEvent.setup();
      const { container } = renderPreferences();

      // The Switch component renders role="switch"; its accessible name comes
      // from the <label htmlFor> ("Aurora ribbons").
      const panel = getCustomPanel(container);
      const auroraSwitch = within(panel).getByRole('switch', { name: /aurora ribbons/i });
      await user.click(auroraSwitch);

      expect(mockSetCustomPerformance).toHaveBeenCalledTimes(1);
      const call = mockSetCustomPerformance.mock.calls.at(0);
      if (!call) throw new Error('expected setCustomPerformance to have been called');
      const updated = call[0] as PerformanceProfile;
      expect(updated.visual.aurora).toBe(false);
    });

    it('calls setCustomPerformance with cursorGlow=false when the cursor glow switch is toggled off', async () => {
      mockProfile = { ...PERFORMANCE_PRESETS.balanced };
      const user = userEvent.setup();
      const { container } = renderPreferences();

      const panel = getCustomPanel(container);
      const glowSwitch = within(panel).getByRole('switch', { name: /cursor glow/i });
      await user.click(glowSwitch);

      const call = mockSetCustomPerformance.mock.calls.at(0);
      if (!call) throw new Error('expected setCustomPerformance to have been called');
      const updated = call[0] as PerformanceProfile;
      expect(updated.visual.cursorGlow).toBe(false);
    });

    it('calls setCustomPerformance with nebula=false when the Nebulae switch is toggled off', async () => {
      mockProfile = { ...PERFORMANCE_PRESETS.balanced };
      const user = userEvent.setup();
      const { container } = renderPreferences();

      const panel = getCustomPanel(container);
      const nebulaSwitch = within(panel).getByRole('switch', { name: /nebulae/i });
      await user.click(nebulaSwitch);

      const call = mockSetCustomPerformance.mock.calls.at(0);
      if (!call) throw new Error('expected setCustomPerformance to have been called');
      const updated = call[0] as PerformanceProfile;
      expect(updated.visual.nebula).toBe(false);
    });

    it('calls setCustomPerformance with animations=false when the Rich animations switch is toggled off', async () => {
      // Balanced preset has animations=true; toggling the switch should flip it.
      mockProfile = { ...PERFORMANCE_PRESETS.balanced };
      const user = userEvent.setup();
      const { container } = renderPreferences();

      const panel = getCustomPanel(container);
      const animationsSwitch = within(panel).getByRole('switch', { name: /rich animations/i });
      await user.click(animationsSwitch);

      expect(mockSetCustomPerformance).toHaveBeenCalledTimes(1);
      const call = mockSetCustomPerformance.mock.calls.at(0);
      if (!call) throw new Error('expected setCustomPerformance to have been called');
      const updated = call[0] as PerformanceProfile;
      expect(updated.visual.animations).toBe(false);
    });
  });

  describe('backend dropdowns', () => {
    it('calls setCustomPerformance with concurrency="low" when Concurrency dropdown changes to Low', async () => {
      mockProfile = { ...PERFORMANCE_PRESETS.balanced };
      const user = userEvent.setup();
      const { container } = renderPreferences();
      const panel = getCustomPanel(container);

      // The Concurrency dropdown trigger button is linked by id="perf-concurrency"
      // and labeled by a <label htmlFor="perf-concurrency">. Query the trigger
      // button by its id to avoid ambiguity with mode-card buttons.
      const concurrencyTrigger = panel.querySelector<HTMLButtonElement>('#perf-concurrency');
      if (!concurrencyTrigger)
        throw new Error('expected Concurrency dropdown trigger #perf-concurrency');
      await user.click(concurrencyTrigger);

      // "Low" option appears in the open panel.
      await user.click(screen.getByRole('option', { name: 'Low' }));

      const call = mockSetCustomPerformance.mock.calls.at(0);
      if (!call) throw new Error('expected setCustomPerformance to have been called');
      const updated = call[0] as PerformanceProfile;
      expect(updated.backend.concurrency).toBe('low');
    });

    it('calls setCustomPerformance with cache="high" when Cache dropdown changes to Generous', async () => {
      mockProfile = { ...PERFORMANCE_PRESETS.balanced };
      const user = userEvent.setup();
      const { container } = renderPreferences();
      const panel = getCustomPanel(container);

      const cacheTrigger = panel.querySelector<HTMLButtonElement>('#perf-cache');
      if (!cacheTrigger) throw new Error('expected Cache dropdown trigger #perf-cache');
      await user.click(cacheTrigger);

      await user.click(screen.getByRole('option', { name: 'Generous' }));

      const call = mockSetCustomPerformance.mock.calls.at(0);
      if (!call) throw new Error('expected setCustomPerformance to have been called');
      const updated = call[0] as PerformanceProfile;
      expect(updated.backend.cache).toBe('high');
    });
  });

  describe('accessible label wiring (<label htmlFor> → Dropdown id)', () => {
    it('each dropdown control has a <label htmlFor> referencing its id', () => {
      const { container } = renderPreferences();
      const labelIds = ['perf-blur', 'perf-concurrency', 'perf-keep-alive', 'perf-cache'];
      for (const id of labelIds) {
        const label = container.querySelector<HTMLLabelElement>(`label[for="${id}"]`);
        if (!label) throw new Error(`expected a <label for="${id}">`);
        expect(label).toBeInTheDocument();
        const trigger = document.getElementById(id);
        if (!trigger) throw new Error(`expected an element with id="${id}"`);
        expect(trigger).toBeInTheDocument();
      }
    });
  });
});
