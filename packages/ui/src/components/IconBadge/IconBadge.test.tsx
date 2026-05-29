import { Star } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { IconBadge } from './IconBadge';

describe('IconBadge', () => {
  it('renders with default size and shape', () => {
    const { container } = render(<IconBadge icon={Star} />);
    const badge = container.firstChild as HTMLElement;
    expect(badge.className).toContain('h-8');
    expect(badge.className).toContain('rounded-lg');
  });

  it('honours size, shape and extra classes', () => {
    const { container } = render(
      <IconBadge icon={Star} size="lg" shape="circle" className="ring" iconClassName="text-x" />
    );
    const badge = container.firstChild as HTMLElement;
    expect(badge.className).toContain('h-10');
    expect(badge.className).toContain('rounded-full');
    expect(badge.className).toContain('ring');
  });
});
