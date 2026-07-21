// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { DocShell } from './DocShell';

afterEach(() => {
  cleanup();
});

describe('DocShell', () => {
  it('renders the "back to the chaos" chrome pointing home', () => {
    render(
      <DocShell>
        <p>body</p>
      </DocShell>
    );
    const back = screen.getByRole('link', { name: /back to the chaos/i });
    expect(back.getAttribute('href')).toBe('/');
  });

  it('renders the optional eyebrow, title, and lede when given', () => {
    render(
      <DocShell eyebrow="the eyebrow" title="Mission Control" lede="a one-line lede">
        <p>body</p>
      </DocShell>
    );
    expect(screen.getByRole('heading', { name: 'Mission Control' })).toBeTruthy();
    expect(screen.getByText('the eyebrow')).toBeTruthy();
    expect(screen.getByText('a one-line lede')).toBeTruthy();
  });

  it('omits the head block entirely when no eyebrow/title/lede is passed', () => {
    const { container } = render(
      <DocShell>
        <p data-testid="child">body</p>
      </DocShell>
    );
    expect(container.querySelector('.doc-shell__head')).toBeNull();
    expect(screen.getByTestId('child')).toBeTruthy();
  });

  it('inlines the docs token palette (never a bare #000 ground)', () => {
    const { container } = render(
      <DocShell>
        <p>body</p>
      </DocShell>
    );
    const style = container.querySelector('style');
    expect(style?.textContent).toContain('--doc-ink: #0d0f14');
    expect(style?.textContent).toContain('--doc-green: #34d399');
  });
});
