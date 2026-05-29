import { Cpu } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { OptionTile } from './OptionTile';

describe('OptionTile', () => {
  it('renders label and description and fires onClick', async () => {
    const onClick = vi.fn();
    render(
      <OptionTile
        icon={Cpu}
        label="Performance"
        description="Use all cores"
        selected={false}
        onClick={onClick}
      />
    );
    expect(screen.getByText('Performance')).toBeInTheDocument();
    expect(screen.getByText('Use all cores')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('renders the selection ring when selected with a layoutId', () => {
    const { container } = render(
      <OptionTile icon={Cpu} label="Sel" selected onClick={() => {}} layoutId="group-1" />
    );
    expect(container.querySelector('.border-brand-soft\\/30')).toBeTruthy();
  });
});
