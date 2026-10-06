/**
 * GenerationOutput — edit → debounce → committed-preview flow.
 * Mocks, props builder and helpers live in `harness.tsx` (see its header).
 */
import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { ControlledWrapper, GenerationOutput, makeProps, resetHarness } from './harness';

beforeEach(resetHarness);

describe('GenerationOutput', () => {
  // ── 8. Edit → save → preview committed-text logic ────────────────────────────
  // The component is controlled: onEdit informs the parent which passes the new
  // output back down. ControlledWrapper simulates that round-trip.
  // PdfPreview (inside previewSlot) always renders the COMMITTED text, which now
  // auto-commits via a ~700 ms debounce — no manual Save button.

  describe('Edit → debounce → preview flow', () => {
    const renderControlled = (output: string) =>
      render(<ControlledWrapper {...makeProps({ activeOut: 'resume', output, editable: true })} />);
    /** Types into the (mock) editor — a synchronous `input` event inside `act`. */
    const typeText = (text: string) => {
      const editBox = screen.getByTestId(TEST_IDS.documents.editableInput);
      void act(() => {
        editBox.textContent = text;
        editBox.dispatchEvent(new Event('input', { bubbles: true }));
      });
    };
    const expectPreview = (text: string) =>
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent(text);

    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('preview text matches the initial output on first render', () => {
      renderControlled('Initial content');
      expectPreview('Initial content');
    });

    it('a parent-driven output change (no local edit) refreshes the preview immediately', () => {
      const props = makeProps({ activeOut: 'resume', output: 'Version 1', editable: true });
      const { rerender } = render(<GenerationOutput {...props} />);

      // No local edit — external change must update committed immediately.
      rerender(<GenerationOutput {...props} output="Version 2" />);

      expectPreview('Version 2');
    });

    it('before 700 ms the preview still shows the last committed text', () => {
      renderControlled('Committed text');

      expectPreview('Committed text');

      typeText('Edited text');

      void act(() => vi.advanceTimersByTime(699));

      // Preview must still show old committed text — debounce not yet fired.
      expectPreview('Committed text');
    });

    it('after 700 ms the debounce auto-commits and the preview updates', () => {
      renderControlled('Old text');

      typeText('New text');

      void act(() => vi.advanceTimersByTime(700));

      expectPreview('New text');
    });

    it('blur flushes the debounce immediately without waiting 700 ms', async () => {
      // Switch to real timers for this test — the blur flush is synchronous and
      // fake-timer + async-act interaction can hide the state update.
      vi.useRealTimers();

      renderControlled('Before blur');

      const editBox = screen.getByTestId(TEST_IDS.documents.editableInput);
      await act(async () => {
        editBox.textContent = 'After blur';
        editBox.dispatchEvent(new Event('input', { bubbles: true }));
      });
      await act(async () => {
        editBox.dispatchEvent(new Event('blur', { bubbles: true }));
      });

      // Blur flushes the commit synchronously; waitFor handles React's async render.
      await waitFor(() => {
        expectPreview('After blur');
      });
    });

    it('no Save button is rendered', () => {
      renderControlled('Some text');
      expect(screen.queryByTestId(TEST_IDS.documents.saveBtn)).not.toBeInTheDocument();
    });

    // ── BUG 2 regression: tab-switch commit must route to the correct doc ─────────
    // Uses the REAL useDebouncedCommit hook (not mocked) + fake timers.
    // Scenario: type on resume tab → switch to cover before 700 ms → the flush
    // triggered by the switch must commit the typed value to RESUME (not cover);
    // cover's preview must remain unchanged.
    it('edit resume → switch tab before 700 ms → resume commits typed value, cover is untouched', () => {
      // Stateful wrapper that mirrors the real parent: each doc owns its own output
      // string and the active doc's output is passed down. Switching tabs passes
      // the COVER's content as output, so the external-change detection does not
      // accidentally overwrite committed.cover with the resume text.
      function TabSwitchWrapper() {
        const [activeOut, setActiveOut] = React.useState<'resume' | 'cover'>('resume');
        const [resumeText, setResumeText] = React.useState('Original resume');
        const coverText = 'Cover content';
        const output = activeOut === 'resume' ? resumeText : coverText;
        const handleEdit = (text: string) => {
          if (activeOut === 'resume') setResumeText(text);
        };
        return (
          <GenerationOutput
            {...makeProps({
              target: 'both',
              activeOut,
              setActiveOut,
              output,
              onEdit: handleEdit,
              editable: true,
            })}
          />
        );
      }

      render(<TabSwitchWrapper />);

      // 1. Type on the resume tab — scheduleCommit('resume', 'Typed resume') fires.
      typeText('Typed resume');

      // 2. Advance only 300 ms — debounce has NOT fired yet.
      void act(() => vi.advanceTimersByTime(300));

      // Preview still shows old committed text (resume).
      expectPreview('Original resume');

      // 3. Switch to cover tab before the 700 ms window — triggers flush().
      //    flush() must commit ('resume', 'Typed resume') — not ('cover', anything).
      void act(() => {
        screen.getByRole('tab', { name: 'autopilot.apply.target.cover' }).click();
      });

      // 4. Advance past the original debounce window; the timer was cancelled by flush.
      void act(() => vi.advanceTimersByTime(700));

      // Cover tab is now active — its committed text comes from coverText ('Cover content').
      expectPreview('Cover content');

      // 5. Switch BACK to resume to verify its committed value.
      void act(() => {
        screen.getByRole('tab', { name: 'autopilot.apply.target.resume' }).click();
      });

      // Resume must show the typed value — committed by the flush at tab-switch time.
      expectPreview('Typed resume');
    });
  });
});
