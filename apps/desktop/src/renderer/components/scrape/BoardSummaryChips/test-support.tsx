/** Render helpers for the BoardSummaryChips tests (mocks live in `test-mocks`). */

import { render, screen } from '@testing-library/react';

import type { BoardScrapeSummary } from '@ajh/shared';

import { BoardSummaryChips } from '../BoardSummaryChips';

export function chips() {
  return screen.queryAllByTestId('chip');
}

export function texts() {
  return chips().map((c) => c.textContent ?? '');
}

/** Render the strip and return its chips. */
export function renderChips(summaries: BoardScrapeSummary[], now?: number) {
  render(<BoardSummaryChips summaries={summaries} now={now} />);
  return chips();
}

/** Render a single summary and return its first chip. */
export function renderFirstChip(summary: BoardScrapeSummary) {
  return renderChips([summary])[0];
}
