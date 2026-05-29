import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { IconText } from './IconText';

describe('IconText', () => {
  it('renders icon and text together', () => {
    render(<IconText icon={<span data-testid="ic" />}>Label</IconText>);
    expect(screen.getByTestId('ic')).toBeInTheDocument();
    expect(screen.getByText('Label')).toBeInTheDocument();
  });

  it('applies the gap variants', () => {
    const { container: sm } = render(
      <IconText icon={null} gap="sm">
        a
      </IconText>
    );
    expect((sm.firstChild as HTMLElement).className).toContain('gap-1');
    const { container: lg } = render(
      <IconText icon={null} gap="lg">
        b
      </IconText>
    );
    expect((lg.firstChild as HTMLElement).className).toContain('gap-2');
  });
});
