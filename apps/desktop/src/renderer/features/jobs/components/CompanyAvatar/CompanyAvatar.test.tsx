import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
  // Image stub: renders a div with data-src + a button to simulate onError.
  // We use a div not an img to avoid the ESLint raw-<img> ban in renderer tests.
  Image: ({
    src,
    className,
    onError,
  }: {
    src: string;
    alt?: string;
    className?: string;
    preview?: boolean;
    onError?: () => void;
  }) => (
    // The div itself is the error trigger: fireEvent.click fires onError.
    // data-testid="logo-image" for presence assertions; data-src for src assertions.
    <div
      data-testid="logo-image"
      data-src={src}
      className={className}
      onClick={onError}
      role="presentation"
    />
  ),
}));

// ── preferences store — control fetchCompanyLogos per test ───────────────────

let mockFetchLogos = false;
vi.mock('@/store/preferences-store', () => ({
  useFetchCompanyLogos: () => mockFetchLogos,
}));

// ── useCompanyLogo — controlled per test ─────────────────────────────────────

let mockLogoUrl: string | null = null;
vi.mock('@/services', () => ({
  useCompanyLogo: (_company: string, _enabled: boolean) => mockLogoUrl,
}));

import { CompanyAvatar } from './index';

// ─────────────────────────────────────────────────────────────────────────────
// Initials
// ─────────────────────────────────────────────────────────────────────────────

describe('CompanyAvatar — initials', () => {
  it('single-word company → first 2 chars', () => {
    render(<CompanyAvatar company="Apple" />);
    expect(screen.getByText('AP')).toBeInTheDocument();
  });

  it('multi-word company → first+last word initial', () => {
    render(<CompanyAvatar company="Google Inc" />);
    expect(screen.getByText('GI')).toBeInTheDocument();
  });

  it('falls back to sourceFallback initials when company is empty', () => {
    render(<CompanyAvatar company="" sourceFallback="linkedin" />);
    expect(screen.getByText('LI')).toBeInTheDocument();
  });

  it('shows ? when both company and sourceFallback are empty', () => {
    render(<CompanyAvatar company="" />);
    expect(screen.getByText('?')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Deterministic color slot
// ─────────────────────────────────────────────────────────────────────────────

describe('CompanyAvatar — deterministic color slot', () => {
  it('same company always maps to the same slot class (idempotent)', () => {
    const { container: a } = render(<CompanyAvatar company="Acme" />);
    const { container: b } = render(<CompanyAvatar company="Acme" />);
    const divA = a.querySelector('[aria-hidden="true"]');
    const divB = b.querySelector('[aria-hidden="true"]');
    expect(divA?.className).toBe(divB?.className);
  });

  it('different first-char companies CAN produce different slot classes', () => {
    // 'A' (65 % 6 = 5) vs 'G' (71 % 6 = 5) → same slot for these two; pick chars with different slots
    // 'A' (65 % 6 = 5) vs 'B' (66 % 6 = 0) → guaranteed different
    const { container: a } = render(<CompanyAvatar company="Acme" />);
    const { container: b } = render(<CompanyAvatar company="Bobs" />);
    const divA = a.querySelector('[aria-hidden="true"]');
    const divB = b.querySelector('[aria-hidden="true"]');
    expect(divA?.className).not.toBe(divB?.className);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Size prop
// ─────────────────────────────────────────────────────────────────────────────

describe('CompanyAvatar — size prop', () => {
  it('sm produces h-8 w-8', () => {
    const { container } = render(<CompanyAvatar company="Acme" size="sm" />);
    const div = container.querySelector('[aria-hidden="true"]');
    expect(div?.className).toContain('h-8');
    expect(div?.className).toContain('w-8');
  });

  it('md produces h-10 w-10', () => {
    const { container } = render(<CompanyAvatar company="Acme" size="md" />);
    const div = container.querySelector('[aria-hidden="true"]');
    expect(div?.className).toContain('h-10');
    expect(div?.className).toContain('w-10');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Logo layer — setting off → monogram only; setting on + logo → img shown.
// onError → logoFailed=true → monogram visible again (never an empty avatar).
// ─────────────────────────────────────────────────────────────────────────────

describe('CompanyAvatar — logo layer (fetchCompanyLogos preference)', () => {
  it('renders no logo image when setting is off (monogram only)', () => {
    mockFetchLogos = false;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    render(<CompanyAvatar company="Acme" />);
    expect(screen.queryByTestId('logo-image')).toBeNull();
    expect(screen.getByText('AC')).toBeInTheDocument();
  });

  it('renders no logo image when setting is on but logo is null', () => {
    mockFetchLogos = true;
    mockLogoUrl = null;
    render(<CompanyAvatar company="Acme" />);
    expect(screen.queryByTestId('logo-image')).toBeNull();
    expect(screen.getByText('AC')).toBeInTheDocument();
  });

  it('renders logo image with the correct src when setting is on and logo resolves', () => {
    mockFetchLogos = true;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    render(<CompanyAvatar company="Acme" />);
    const logoEl = screen.getByTestId('logo-image');
    expect(logoEl).toBeInTheDocument();
    expect(logoEl.getAttribute('data-src')).toBe('https://logo.clearbit.com/acme.com');
  });

  it('monogram span carries invisible class while logo layer is active', () => {
    mockFetchLogos = true;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    render(<CompanyAvatar company="Acme" />);
    const mono = screen.getByText('AC');
    expect(mono.className).toContain('invisible');
  });

  it('monogram is visible (no invisible class) when setting is off', () => {
    mockFetchLogos = false;
    mockLogoUrl = null;
    render(<CompanyAvatar company="Acme" />);
    const mono = screen.getByText('AC');
    expect(mono.className ?? '').not.toContain('invisible');
  });

  it('monogram becomes visible again after the image errors (onError fallback)', () => {
    mockFetchLogos = true;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    render(<CompanyAvatar company="Acme" />);

    // Before error: monogram is invisible, logo image present
    expect(screen.getByText('AC').className).toContain('invisible');
    expect(screen.getByTestId('logo-image')).toBeInTheDocument();

    // Simulate the image failing to load (404 / CORS block on img-src).
    // The mock fires onError when the logo-image div is clicked.
    fireEvent.click(screen.getByTestId('logo-image'));

    // After error: logo image removed, monogram visible (no invisible class)
    expect(screen.queryByTestId('logo-image')).toBeNull();
    expect(screen.getByText('AC').className ?? '').not.toContain('invisible');
  });

  it('stale logoFailed resets when rerendered with a new company + working logo', () => {
    // Render company A with a logo that errors → logoFailed=true → monogram shown.
    mockFetchLogos = true;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    const { rerender } = render(<CompanyAvatar company="Acme" />);

    // Trigger the error so logoFailed becomes true for company A.
    fireEvent.click(screen.getByTestId('logo-image'));
    expect(screen.queryByTestId('logo-image')).toBeNull();

    // Rerender with company B + a different (working) logo URL.
    // logoFailed must reset via the useEffect that watches logoUrl.
    mockLogoUrl = 'https://logo.clearbit.com/google.com';
    rerender(<CompanyAvatar company="Google" />);

    // Company B's logo should render (no error fired yet) and monogram invisible.
    const logoEl = screen.getByTestId('logo-image');
    expect(logoEl).toBeInTheDocument();
    expect(logoEl.getAttribute('data-src')).toBe('https://logo.clearbit.com/google.com');
    expect(screen.getByText('GO').className).toContain('invisible');
  });
});
