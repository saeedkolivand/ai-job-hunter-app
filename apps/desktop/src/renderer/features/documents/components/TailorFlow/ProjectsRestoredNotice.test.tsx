import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { ProjectsRestoredNotice } from './ProjectsRestoredNotice';

describe('ProjectsRestoredNotice', () => {
  it('renders the translated notice, not the raw key', () => {
    render(<ProjectsRestoredNotice />);
    const el = screen.getByTestId(TEST_IDS.documents.projectsRestoredNotice);
    expect(el.textContent).toMatch(/Projects section/);
  });
});
