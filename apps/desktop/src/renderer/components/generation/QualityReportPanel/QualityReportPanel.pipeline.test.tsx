import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { buildSectionVerdicts, type Fabrication, removeEvidenceLines } from '@/lib/generate';

import { QualityBadge } from './QualityBadge';
import { type QualityPipelineReview, QualityReportPanel } from './QualityReportPanel';

const METRICS: ContentReportPayload['metrics'] = {
  keywordCoverage: 60,
  topRequirementHits: 1,
  topRequirementsMeasured: 2,
  duplicateRatio: 0,
  rolesSource: 1,
  rolesOutput: 1,
};

const CLEAN_REPORT: ContentReportPayload = { ok: true, issues: [], metrics: METRICS };

const FLAGGED_LINE = 'Cut latency by 40% across the fleet.';
const DOCUMENT = ['Summary', FLAGGED_LINE, '', 'Experience', 'Acme'].join('\n');

const PENDING: Fabrication = {
  issueKey: 'factual.unsourced_metric#0',
  code: 'factual.unsourced_metric',
  evidence: 'Cut latency by 40%',
  // The line the span sat on — the ONLY thing a removal may be anchored to.
  line: FLAGGED_LINE,
};

function pipeline(overrides: Partial<QualityPipelineReview> = {}): QualityPipelineReview {
  return {
    documentText: DOCUMENT,
    sections: buildSectionVerdicts(CLEAN_REPORT, DOCUMENT),
    fabrications: [PENDING],
    ...overrides,
  };
}

function renderPanel(review: QualityPipelineReview, report = CLEAN_REPORT) {
  return render(
    <QualityReportPanel open onClose={vi.fn()} report={report} docKind="resume" pipeline={review} />
  );
}

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
      rerender(
        <QualityReportPanel
          open
          onClose={vi.fn()}
          report={CLEAN_REPORT}
          docKind="resume"
          pipeline={pipeline({ fabrications: [{ ...PENDING, decision: 'remove' }] })}
        />
      );
      expect(screen.getByText(/still in the document/i)).toBeInTheDocument();
      expect(screen.getByText(/edit it out of the document to finish/i)).toBeInTheDocument();
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
            <QualityReportPanel
              open
              onClose={vi.fn()}
              report={CLEAN_REPORT}
              docKind="resume"
              pipeline={pipeline({
                documentText: text,
                fabrications: entries,
                onResolveFabrication: vi.fn(),
                onRemoveEvidence: applyRemoval,
              })}
            />
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

describe('QualityBadge — a needsReview run is never green', () => {
  function hashOf(text: string): number {
    let hash = 5381;
    for (let i = 0; i < text.length; i++) hash = (hash * 33) ^ text.charCodeAt(i);
    return hash >>> 0;
  }

  /** A report whose slot hash matches `text`, so staleness stays out of the way
   *  and the assertion is about the review alone. */
  function wrapperFor(text: string) {
    return {
      schemaVersion: 2 as const,
      pipeline: 'quality' as const,
      generatedAt: 0,
      resume: { report: CLEAN_REPORT, sourceTextHash: hashOf(text) },
    };
  }

  const wrapper = wrapperFor(DOCUMENT);

  it('counts undecided claims as open issues even on an otherwise clean report', () => {
    render(
      <QualityBadge
        report={wrapper}
        docKind="resume"
        currentText={DOCUMENT}
        pipeline={pipeline()}
      />
    );
    expect(screen.queryByText('Checked — no issues')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1 issue/i })).toBeInTheDocument();
  });

  it('goes green on Keep — the verdict and the document already agree', () => {
    render(
      <QualityBadge
        report={wrapper}
        docKind="resume"
        currentText={DOCUMENT}
        pipeline={pipeline({ fabrications: [{ ...PENDING, decision: 'keep' }] })}
      />
    );
    expect(screen.getByText('Checked — no issues')).toBeInTheDocument();
  });

  // THE finding: a recorded "Remove" over text that is still, verbatim, in the
  // document. Mutation-guard for `unresolvedCount` — count any decision as
  // resolved and this assertion flips to green.
  it('stays OFF green while a recorded Remove has not been applied', () => {
    render(
      <QualityBadge
        report={wrapper}
        docKind="resume"
        currentText={DOCUMENT}
        pipeline={pipeline({ fabrications: [{ ...PENDING, decision: 'remove' }] })}
      />
    );
    expect(screen.queryByText('Checked — no issues')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1 issue/i })).toBeInTheDocument();
  });

  it('goes green on Remove once the evidence is genuinely gone', () => {
    const edited = 'Summary\n\nExperience\nAcme';
    render(
      <QualityBadge
        report={wrapperFor(edited)}
        docKind="resume"
        currentText={edited}
        pipeline={pipeline({
          documentText: edited,
          fabrications: [{ ...PENDING, decision: 'remove' }],
        })}
      />
    );
    expect(screen.getByText('Checked — no issues')).toBeInTheDocument();
  });

  // The LIVE text is the authority, not the bundle's snapshot. A host that
  // still carries the run's original `documentText` after the user hand-edited
  // the line away must not keep the chip red — and, worse, the mirror case
  // (bundle already clean, line still on screen) would go green over text the
  // user is looking at.
  it('measures claims against the LIVE text, not the bundle’s snapshot', () => {
    const edited = 'Summary\n\nExperience\nAcme';
    render(
      <QualityBadge
        report={wrapperFor(edited)}
        docKind="resume"
        currentText={edited}
        pipeline={pipeline({
          // Stale: still the pre-edit document the run produced.
          documentText: DOCUMENT,
          fabrications: [{ ...PENDING, decision: 'remove' }],
        })}
      />
    );
    expect(screen.getByText('Checked — no issues')).toBeInTheDocument();
  });

  it('turns Remove into a real edit through the host’s document writer', async () => {
    const onDocumentTextChange = vi.fn();
    // The user typed a line AFTER the run produced its snapshot, so the live
    // text and the bundle's `documentText` differ. The edit must be computed
    // from the LIVE text — building it from the snapshot would write the
    // hand-typed line back out of existence.
    const live = `${DOCUMENT}\nHand-typed note`;
    render(
      <QualityBadge
        report={wrapperFor(live)}
        docKind="resume"
        currentText={live}
        pipeline={pipeline({ documentText: DOCUMENT, onResolveFabrication: vi.fn() })}
        onDocumentTextChange={onDocumentTextChange}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: /1 issue/i }));
    await userEvent.click(screen.getByRole('button', { name: /^remove$/i }));

    // The flagged LINE is gone from the text handed to the host's save path,
    // and nothing else is.
    await waitFor(() =>
      expect(onDocumentTextChange).toHaveBeenCalledWith(
        'Summary\n\nExperience\nAcme\nHand-typed note'
      )
    );
  });

  // ── The apply is anchored on the entry's LINE ──────────────────────────────
  //
  // Validator evidence is routinely a bare token, and locating the line by
  // searching for it deletes whatever else happens to contain those characters.
  // These two pin the wiring `removeEvidenceLines`' own unit tests cannot: that
  // the badge hands the review the ENTRY, and refuses when it has no anchor.
  describe('a removal never guesses which line it meant', () => {
    const RESUME = [
      'ADA LOVELACE',
      '+1 (555) 250-8817 · ada@example.test',
      '',
      'EXPERIENCE',
      '- Cut cloud spend by 250k in one quarter.',
    ].join('\n');

    const BARE_TOKEN: Fabrication = {
      issueKey: 'factual.unsourced_metric#0',
      code: 'factual.unsourced_metric',
      evidence: '250',
      line: '- Cut cloud spend by 250k in one quarter.',
    };

    it('keeps a contact header whose phone number contains the evidence', async () => {
      const onDocumentTextChange = vi.fn();
      render(
        <QualityBadge
          report={wrapperFor(RESUME)}
          docKind="resume"
          currentText={RESUME}
          pipeline={pipeline({
            documentText: RESUME,
            fabrications: [BARE_TOKEN],
            onResolveFabrication: vi.fn(),
          })}
          onDocumentTextChange={onDocumentTextChange}
        />
      );

      await userEvent.click(screen.getByRole('button', { name: /1 issue/i }));
      await userEvent.click(screen.getByRole('button', { name: /^remove$/i }));

      await waitFor(() =>
        expect(onDocumentTextChange).toHaveBeenCalledWith(
          'ADA LOVELACE\n+1 (555) 250-8817 · ada@example.test\n\nEXPERIENCE'
        )
      );
    });

    // A report persisted before `line` existed. The verdict is still recorded —
    // it is the user's — but nothing is written, and the row says the line is
    // still there rather than claiming a deletion that never happened.
    it('refuses to write for a legacy entry with no line, and says so', async () => {
      const onDocumentTextChange = vi.fn();
      const onResolveFabrication = vi.fn();
      const legacy: Fabrication = { issueKey: 'x#0', code: 'c', evidence: 'Cut latency by 40%' };
      const badge = (fabrications: Fabrication[]) => (
        <QualityBadge
          report={wrapperFor(DOCUMENT)}
          docKind="resume"
          currentText={DOCUMENT}
          pipeline={pipeline({ fabrications, onResolveFabrication })}
          onDocumentTextChange={onDocumentTextChange}
        />
      );
      const { rerender } = render(badge([legacy]));

      await userEvent.click(screen.getByRole('button', { name: /1 issue/i }));
      await userEvent.click(screen.getByRole('button', { name: /^remove$/i }));

      await waitFor(() => expect(onResolveFabrication).toHaveBeenCalledWith('x#0', 'remove'));
      expect(onDocumentTextChange).not.toHaveBeenCalled();

      // …and once the recorded verdict comes back with the line still there,
      // the row says exactly that rather than "Removed".
      rerender(badge([{ ...legacy, decision: 'remove' }]));
      expect(screen.getByText(/still in the document/i)).toBeInTheDocument();
      expect(screen.getByText(/edit it out of the document to finish/i)).toBeInTheDocument();
    });
  });

  it('records the verdict but writes nothing when the host has no writer', async () => {
    const onResolveFabrication = vi.fn();
    render(
      <QualityBadge
        report={wrapper}
        docKind="resume"
        currentText={DOCUMENT}
        pipeline={pipeline({ onResolveFabrication })}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: /1 issue/i }));
    await userEvent.click(screen.getByRole('button', { name: /^remove$/i }));
    await waitFor(() =>
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'remove')
    );
  });
});
