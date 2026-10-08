import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { WeakAnalysisNotice } from './WeakAnalysisNotice';

describe('WeakAnalysisNotice', () => {
  it('renders the translated notice, not the raw key', () => {
    render(<WeakAnalysisNotice />);
    const el = screen.getByTestId(TEST_IDS.documents.weakAnalysisNotice);
    expect(el.textContent).toMatch(/keyword matching/);
  });
});
