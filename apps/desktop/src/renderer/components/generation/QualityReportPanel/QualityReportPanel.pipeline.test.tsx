import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { type Fabrication, removeEvidenceLines } from '@/lib/generate';

import { panelElement, PENDING, pipeline, renderPanel } from './test-support';

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
    it('lists each flagged claim with Remove and Keep, and removes nothing unasked', async () => {
      const onResolveFabrication = vi.fn();
      renderPanel(pipeline({ onResolveFabrication }));

      expect(screen.getByText('“Cut latency by 40%”')).toBeInTheDocument();
      expect(screen.getByText(/1 claim still needs a decision/i)).toBeInTheDocument();

      await userEvent.click(screen.getByRole('button', { name: /remove/i }));
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'remove');
    });

    it('APPLIES the removal to the document before recording the verdict', async () => {
      const order: string[] = [];
      const onRemoveEvidence = vi.fn(() => {
        order.push('apply');
      });
      const onResolveFabrication = vi.fn(() => {
        order.push('record');
      });
      renderPanel(pipeline({ onRemoveEvidence, onResolveFabrication }));

      await userEvent.click(screen.getByRole('button', { name: /remove/i }));
      await waitFor(() => expect(onResolveFabrication).toHaveBeenCalled());
      // The whole ENTRY, not its span: the apply anchors on `line`, and a bare
      // span is not enough to identify a line with.
      expect(onRemoveEvidence).toHaveBeenCalledWith(PENDING);
      // Order matters: recording first would briefly claim the entry is settled
      // while the line is still in the document.
      expect(order).toEqual(['apply', 'record']);
    });

    it('keeps the verdict — and says the line is still there — when the apply fails', async () => {
      const onResolveFabrication = vi.fn();
      const onRemoveEvidence = vi.fn(() => Promise.reject(new Error('document is read-only')));
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const { rerender } = renderPanel(pipeline({ onRemoveEvidence, onResolveFabrication }));

      await userEvent.click(screen.getByRole('button', { name: /remove/i }));
      // The user's decision is never thrown away…
      await waitFor(() =>
        expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'remove')
      );
      consoleError.mockRestore();

      // …and once it comes back on the record with the line STILL in the
      // document, the row says exactly that instead of "Marked for removal".
      rerender(panelElement(pipeline({ fabrications: [{ ...PENDING, decision: 'remove' }] })));
      expect(screen.getByText(/still in the document/i)).toBeInTheDocument();
      expect(screen.getByText(/edit the line out there to finish/i)).toBeInTheDocument();
      expect(screen.getByText(/1 claim still needs a decision/i)).toBeInTheDocument();
    });

    it('records Keep without touching the document', async () => {
      const onRemoveEvidence = vi.fn();
      const onResolveFabrication = vi.fn();
      renderPanel(pipeline({ onRemoveEvidence, onResolveFabrication }));

      await userEvent.click(screen.getByRole('button', { name: /keep/i }));
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'keep');
      expect(onRemoveEvidence).not.toHaveBeenCalled();
    });

    it('surfaces a failed resolve write in an alert, like the Fix twin does', () => {
      renderPanel(
        pipeline({
          onResolveFabrication: vi.fn(),
          resolveError: "Couldn't record that decision. Try again.",
        })
      );
      expect(screen.getByRole('alert')).toHaveTextContent(/couldn't record that decision/i);
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
      renderPanel(
        pipeline({
          // Applied: the verdict and the document agree.
          documentText: 'Summary\nLed the platform migration.',
          fabrications: [{ ...PENDING, decision: 'remove' }],
        })
      );
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

    /**
     * Two Removes clicked before the first write settles.
     *
     * A host's write is never instantaneous — the editor commits on a debounce,
     * the save round-trips — so both handlers close over the SAME document, the
     * second edit is computed from the pre-first text, and writing it puts the
     * first line back. A flagged claim silently returning after the user removed
     * it is worse than either verdict. The review locks every button while an
     * apply is in flight, so the second click lands against the post-first text.
     *
     * Mutation check: drop the `applying` gate in `FabricationReview` and the
     * first assertion finds "Cut cloud spend" back in the document.
     */
    it('does not let a second Remove resurrect the first one’s line', async () => {
      const LINE_A = '- Cut cloud spend by 250k in one quarter.';
      const LINE_B = '- Ran the kubernetes migration.';
      const START = ['EXPERIENCE', LINE_A, LINE_B, 'Skills: kubernetes'].join('\n');
      const entries: Fabrication[] = [
        { issueKey: 'a#0', code: 'factual.unsourced_metric', evidence: '250', line: LINE_A },
        { issueKey: 'b#1', code: 'factual.unsourced_term', evidence: 'kubernetes', line: LINE_B },
      ];

      function Host() {
        const [text, setText] = useState(START);
        const applyRemoval = async (entry: Fabrication) => {
          const next = removeEvidenceLines(text, entry);
          await Promise.resolve();
          if (next !== null) setText(next);
        };
        return (
          <>
            {panelElement(
              pipeline({
                documentText: text,
                fabrications: entries,
                onResolveFabrication: vi.fn(),
                onRemoveEvidence: applyRemoval,
              })
            )}
            <pre data-testid="doc">{text}</pre>
          </>
        );
      }

      render(<Host />);
      // Both clicks in ONE tick — a double-click, or an impatient user.
      const [removeA, removeB] = screen.getAllByRole('button', { name: /^remove$/i });
      if (!removeA || !removeB) throw new Error('expected a Remove per entry');
      fireEvent.click(removeA);
      fireEvent.click(removeB);

      await waitFor(() =>
        expect(screen.getByTestId('doc').textContent).toBe(
          ['EXPERIENCE', LINE_B, 'Skills: kubernetes'].join('\n')
        )
      );

      // The second verdict, taken once the first has settled, is computed
      // against the document as it now stands: B goes, A stays gone.
      const [, second] = screen.getAllByRole('button', { name: /^remove$/i });
      if (!second) throw new Error('expected the second entry to still be decidable');
      await userEvent.click(second);
      await waitFor(() =>
        expect(screen.getByTestId('doc').textContent).toBe(
          ['EXPERIENCE', 'Skills: kubernetes'].join('\n')
        )
      );
    });
  });
});
