import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';

import { PrefilledBadge } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children, title }: { children: ReactNode; title?: string }) => (
    <a title={title}>{children}</a>
  ),
}));

describe('PrefilledBadge', () => {
  it('renders the "from location settings" label exactly once', () => {
    const { container } = render(<PrefilledBadge />);

    const label = 'autopilot.wizard.target.fromLocationSettings';
    expect(container.textContent?.split(label)).toHaveLength(2);
  });
});
