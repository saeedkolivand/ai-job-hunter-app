import { Zap } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ActionTile } from './ActionTile';

describe('ActionTile', () => {
  it('renders label, description and badge', () => {
    render(
      <ActionTile icon={Zap} label="Generate" description="Make a doc" badge={<span>NEW</span>} />
    );
    expect(screen.getByText('Generate')).toBeInTheDocument();
    expect(screen.getByText('Make a doc')).toBeInTheDocument();
    expect(screen.getByText('NEW')).toBeInTheDocument();
  });

  it('fires onClick and reflects the active state', async () => {
    const onClick = vi.fn();
    render(<ActionTile icon={Zap} label="Run" active onClick={onClick} />);
    await userEvent.click(screen.getByText('Run'));
    expect(onClick).toHaveBeenCalledOnce();
  });
});
