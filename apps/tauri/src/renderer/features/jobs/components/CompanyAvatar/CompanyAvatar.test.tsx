import React from 'react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import { CompanyAvatar } from './index';

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
