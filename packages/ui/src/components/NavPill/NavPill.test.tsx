import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { NavPill } from './NavPill';

describe('NavPill', () => {
  it('renders a decorative pill carrying the layoutId class hooks', () => {
    const { container } = render(<NavPill layoutId="test-pill" />);
    const pill = container.firstChild as HTMLElement;
    expect(pill).toBeInTheDocument();
    // decorative: hidden from the a11y tree (active state lives on the row)
    expect(pill).toHaveAttribute('aria-hidden');
    // never intercepts clicks meant for the row above it
    expect(pill.className).toContain('pointer-events-none');
    expect(pill.className).toContain('absolute');
    expect(pill.className).toContain('inset-0');
  });

  it('merges a custom className onto the pill', () => {
    const { container } = render(<NavPill layoutId="test-pill" className="rounded-full" />);
    expect((container.firstChild as HTMLElement).className).toContain('rounded-full');
  });
});

// ── accent-gradient token assertions (feat/accent-gradients) ──────────────────
//
// jsdom's cssstyle drops/normalises linear-gradient and color-mix values, so
// reading el.style.background is unreliable across the full-suite run.
// Instead we read the raw `style` attribute string, which preserves the
// authored text exactly as React serialises it to the DOM attribute.

describe('NavPill — accent-gradient tokens', () => {
  it('inline style references var(--color-brand) for background/border/boxShadow', () => {
    const { container } = render(<NavPill layoutId="accent-test" />);
    const pill = container.firstChild as HTMLElement;
    const styleAttr = pill.getAttribute('style') ?? '';

    // All three CSS properties must use the design-token CSS variable, not raw RGB.
    expect(styleAttr).toContain('--color-brand');
  });

  it('inline style does NOT contain the old hardcoded RGB values (168, 85, 247)', () => {
    const { container } = render(<NavPill layoutId="accent-test-no-hex" />);
    const styleAttr = (container.firstChild as HTMLElement).getAttribute('style') ?? '';

    // Neither space-separated nor comma-only form of the old purple RGB literal.
    expect(styleAttr).not.toContain('168, 85, 247');
    expect(styleAttr).not.toContain('168,85,247');
  });

  it('inline background references var(--color-brand-2) for the gradient end-stop', () => {
    const { container } = render(<NavPill layoutId="accent-brand2-test" />);
    const styleAttr = (container.firstChild as HTMLElement).getAttribute('style') ?? '';

    expect(styleAttr).toContain('--color-brand-2');
  });
});
