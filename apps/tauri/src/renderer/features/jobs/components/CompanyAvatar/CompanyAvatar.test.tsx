import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
  // Image stub: renders a div with data-src so tests can assert without a raw <img>
  Image: ({
    src,
    className,
  }: {
    src: string;
    alt?: string;
    className?: string;
    preview?: boolean;
  }) => <div data-testid="logo-image" data-src={src} className={className} />,
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
    // 'A' (65 % 7 = 2) vs 'G' (71 % 7 = 1) → different slots guaranteed here
    const { container: a } = render(<CompanyAvatar company="Acme" />);
    const { container: b } = render(<CompanyAvatar company="Google" />);
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
// The Image component handles onError internally; we test the render gate.
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

  it('renders the logo image with the correct src when setting is on and logo resolves', () => {
    mockFetchLogos = true;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    render(<CompanyAvatar company="Acme" />);
    const logoEl = screen.getByTestId('logo-image');
    expect(logoEl).toBeInTheDocument();
    expect(logoEl.getAttribute('data-src')).toBe('https://logo.clearbit.com/acme.com');
  });

  it('monogram span carries invisible class when logo is shown', () => {
    mockFetchLogos = true;
    mockLogoUrl = 'https://logo.clearbit.com/acme.com';
    render(<CompanyAvatar company="Acme" />);
    // The monogram span is rendered but visually hidden so the logo covers it
    const mono = screen.getByText('AC');
    expect(mono.className).toContain('invisible');
  });

  it('monogram span has no invisible class when setting is off', () => {
    mockFetchLogos = false;
    mockLogoUrl = null;
    render(<CompanyAvatar company="Acme" />);
    const mono = screen.getByText('AC');
    expect(mono.className ?? '').not.toContain('invisible');
  });
});
