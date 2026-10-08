import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { WizardStep } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('WizardStep', () => {
  it('renders the title and description but no step counter (the wizard top bar owns it)', () => {
    render(
      <WizardStep title="Contact" description="Your details" align="top">
        <div>body</div>
      </WizardStep>
    );

    expect(screen.getByRole('heading', { name: 'Contact' })).toBeInTheDocument();
    expect(screen.queryByText(/stepCounter/)).toBeNull();
  });
});
