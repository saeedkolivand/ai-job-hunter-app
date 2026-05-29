import { Settings } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { SettingsSection } from './SettingsSection';

describe('SettingsSection', () => {
  it('renders the label and children', () => {
    render(
      <SettingsSection icon={Settings} label="Appearance">
        <p>body</p>
      </SettingsSection>
    );
    expect(screen.getByText('Appearance')).toBeInTheDocument();
    expect(screen.getByText('body')).toBeInTheDocument();
  });
});
