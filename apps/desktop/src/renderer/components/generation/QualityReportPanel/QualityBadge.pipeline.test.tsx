import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Fabrication } from '@/lib/generate';

import { QualityBadge } from './QualityBadge';
import type { QualityPipelineReview } from './QualityReportPanel';
import { CLEAN_REPORT, DOCUMENT, PENDING, pipeline } from './test-support';

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

/** The badge over `text` (default: the run's own document) carrying `review`. */
function badgeElement(
  review: QualityPipelineReview,
  {
    text = DOCUMENT,
    ...props
  }: Partial<React.ComponentProps<typeof QualityBadge>> & {
    text?: string;
  } = {}
) {
  return (
    <QualityBadge
      report={wrapperFor(text)}
      docKind="resume"
      currentText={text}
      pipeline={review}
      {...props}
    />
  );
}

/** Open the badge's panel, then click the (only) Remove button. */
async function openAndRemove() {
  await userEvent.click(screen.getByRole('button', { name: /1 issue/i }));
  await userEvent.click(screen.getByRole('button', { name: /^remove$/i }));
}

describe('QualityBadge — a needsReview run is never green', () => {
  it('counts undecided claims as open issues even on an otherwise clean report', () => {
    render(badgeElement(pipeline()));
    expect(screen.queryByText('Checked — no issues')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1 issue/i })).toBeInTheDocument();
  });

  it('goes green on Keep — the verdict and the document already agree', () => {
    render(badgeElement(pipeline({ fabrications: [{ ...PENDING, decision: 'keep' }] })));
    expect(screen.getByText('Checked — no issues')).toBeInTheDocument();
  });

  // THE finding: a recorded "Remove" over text that is still, verbatim, in the
  // document. Mutation-guard for `unresolvedCount` — count any decision as
  // resolved and this assertion flips to green.
  it('stays OFF green while a recorded Remove has not been applied', () => {
    render(badgeElement(pipeline({ fabrications: [{ ...PENDING, decision: 'remove' }] })));
    expect(screen.queryByText('Checked — no issues')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1 issue/i })).toBeInTheDocument();
  });

  it('goes green on Remove once the evidence is genuinely gone', () => {
    const edited = 'Summary\n\nExperience\nAcme';
    render(
      badgeElement(
        pipeline({ documentText: edited, fabrications: [{ ...PENDING, decision: 'remove' }] }),
        { text: edited }
      )
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
      badgeElement(
        pipeline({
          // Stale: still the pre-edit document the run produced.
          documentText: DOCUMENT,
          fabrications: [{ ...PENDING, decision: 'remove' }],
        }),
        { text: edited }
      )
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
      badgeElement(pipeline({ documentText: DOCUMENT, onResolveFabrication: vi.fn() }), {
        text: live,
        onDocumentTextChange,
      })
    );

    await openAndRemove();

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
        badgeElement(
          pipeline({
            documentText: RESUME,
            fabrications: [BARE_TOKEN],
            onResolveFabrication: vi.fn(),
          }),
          { text: RESUME, onDocumentTextChange }
        )
      );

      await openAndRemove();

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
      const badge = (fabrications: Fabrication[]) =>
        badgeElement(pipeline({ fabrications, onResolveFabrication }), { onDocumentTextChange });
      const { rerender } = render(badge([legacy]));

      await openAndRemove();

      await waitFor(() => expect(onResolveFabrication).toHaveBeenCalledWith('x#0', 'remove'));
      expect(onDocumentTextChange).not.toHaveBeenCalled();

      // …and once the recorded verdict comes back with the line still there,
      // the row says exactly that rather than "Removed".
      rerender(badge([{ ...legacy, decision: 'remove' }]));
      expect(screen.getByText(/still in the document/i)).toBeInTheDocument();
      expect(screen.getByText(/edit the line out there to finish/i)).toBeInTheDocument();
    });
  });

  it('records the verdict but writes nothing when the host has no writer', async () => {
    const onResolveFabrication = vi.fn();
    render(badgeElement(pipeline({ onResolveFabrication })));

    await openAndRemove();
    await waitFor(() =>
      expect(onResolveFabrication).toHaveBeenCalledWith(PENDING.issueKey, 'remove')
    );
  });
});
