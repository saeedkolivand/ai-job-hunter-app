/**
 * TrustBadge — level gating, resolved i18n copy, and an i18n key-drift guard.
 *
 * Deliberately does NOT mock `@ajh/translations` (unlike most sibling tests —
 * see e.g. `match-band.test.tsx`'s `t: (k) => k` raw-key stub). The whole
 * point of the flag-label tests below is to catch a broken
 * `t(`jobs.trust.flags.${flag}`)` template string (typo'd key, missing
 * resource entry) — a raw-key stub would just echo the key back and could
 * never fail this way, and TypeScript can't check a key built at runtime.
 * `@ajh/translations` resolves to real source in vitest (see
 * `vitest.config.mts`'s alias) and initializes with the real bundled en/de
 * resources as an import side effect, so `t()` here returns real English
 * copy — or, if a key is missing/mistyped, the raw key string, which the
 * tests below assert does NOT appear.
 *
 * `@ajh/ui` (Button/Tag/HoverPopover) is also left unmocked — all three are
 * simple, provider-free components here, so real rendering is cheap and
 * avoids a stub silently drifting from the real trigger/DOM shape.
 */

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { JobTrustAssessment } from '@ajh/shared';
import i18n from '@ajh/translations';

import { TrustBadge } from './trust-badge';

type TrustFlag = JobTrustAssessment['flags'][number];

function assessment(
  level: JobTrustAssessment['level'],
  flags: TrustFlag[] = []
): JobTrustAssessment {
  return { score: 50, level, flags };
}

// ─────────────────────────────────────────────────────────────────────────────
// Level gating — no badge for a missing assessment or a trusted (high) one.
// ─────────────────────────────────────────────────────────────────────────────

describe('TrustBadge — level gating', () => {
  it('renders nothing when trust is undefined', () => {
    const { container } = render(<TrustBadge trust={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing for level "high" (no badge = trusted)', () => {
    const { container } = render(<TrustBadge trust={assessment('high')} />);
    expect(container).toBeEmptyDOMElement();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Resolved level labels — must be real copy, not the raw i18n key.
// ─────────────────────────────────────────────────────────────────────────────

describe('TrustBadge — resolved level labels', () => {
  it('shows "Medium trust" (resolved copy, not the raw key) for level medium', () => {
    const { container } = render(<TrustBadge trust={assessment('medium', ['suspiciousDomain'])} />);
    expect(screen.getByText('Medium trust')).toBeInTheDocument();
    expect(container.textContent).not.toMatch(/jobs\.trust\.level\./);
  });

  it('shows "Low trust" (resolved copy, not the raw key) for level low', () => {
    const { container } = render(<TrustBadge trust={assessment('low', ['invalidUrl'])} />);
    expect(screen.getByText('Low trust')).toBeInTheDocument();
    expect(container.textContent).not.toMatch(/jobs\.trust\.level\./);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// i18n key-drift guard — the point of this file. Every `TrustFlag` must
// resolve through `jobs.trust.flags.${flag}` to real copy; a broken template
// string / missing resource entry would leave the raw key visible instead.
// ─────────────────────────────────────────────────────────────────────────────

const FLAG_LABEL: Record<TrustFlag, string> = {
  missingApplyUrl: 'Missing apply link',
  invalidUrl: 'Broken apply link',
  suspiciousDomain: 'Suspicious domain',
  companyDomainMismatch: "Domain doesn't match company",
};

const ALL_FLAGS = Object.keys(FLAG_LABEL) as TrustFlag[];

describe('TrustBadge — i18n key-drift guard (every flag)', () => {
  it.each(ALL_FLAGS)(
    'flag "%s" resolves to real copy, no raw i18n key leaks into the DOM',
    (flag) => {
      const { container } = render(<TrustBadge trust={assessment('medium', [flag])} />);
      expect(container.textContent).toContain(FLAG_LABEL[flag]);
      expect(container.textContent).not.toMatch(/jobs\.trust\.flags\./);
    }
  );

  it('joins multiple flag labels with ", " in the reasons text', () => {
    const { container } = render(
      <TrustBadge trust={assessment('low', ['missingApplyUrl', 'suspiciousDomain'])} />
    );
    expect(container.textContent).toContain('Missing apply link, Suspicious domain');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// interactive prop — false drops the focusable popover trigger; reasons stay
// reachable via the always-present sr-only suffix either way.
// ─────────────────────────────────────────────────────────────────────────────

describe('TrustBadge — interactive prop', () => {
  it('interactive=false renders inline with no focusable trigger; reasons still present via sr-only text', () => {
    const { container } = render(
      <TrustBadge trust={assessment('medium', ['suspiciousDomain'])} interactive={false} />
    );
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    expect(screen.getByText('Medium trust')).toBeInTheDocument();
    expect(container.textContent).toContain('Suspicious domain');
  });

  it('interactive=true (default) renders a focusable popover trigger button', () => {
    render(<TrustBadge trust={assessment('medium', ['suspiciousDomain'])} />);
    expect(screen.getByRole('button')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// strong prop — opaque solid-fill path; not asserting exact color classes,
// just that both variants mount and still show the resolved label.
// ─────────────────────────────────────────────────────────────────────────────

describe('TrustBadge — strong prop', () => {
  it('mounts without error for both medium and low when strong', () => {
    const { rerender } = render(
      <TrustBadge trust={assessment('medium', ['suspiciousDomain'])} strong />
    );
    expect(screen.getByText('Medium trust')).toBeInTheDocument();

    rerender(<TrustBadge trust={assessment('low', ['invalidUrl'])} strong />);
    expect(screen.getByText('Low trust')).toBeInTheDocument();
  });
});

// ── Level description ───────────────────────────────────────────────────────
//
// The flag list answers "which checks failed" but never "so what?" —
// "Suspicious domain" doesn't tell a reader whether to skip the posting or just
// look twice. These pin the plain-language verdict that now leads the tooltip.
// Real translations (see the file header), so a missing `jobs.trust.levelDesc.*`
// key surfaces as the raw key here.
// Expected copy is read from the SAME resource the component renders from, so
// these assert the description is actually present without hardcoding a
// sentence that would have to be edited every time the wording is tweaked.
// (Absence-of-raw-key assertions alone were too weak: with `levelDescription`
// deleted the detail string still contained no raw keys and still placed the
// flag list past character 10, so every assertion passed with the feature gone.)
const levelDescriptionFor = (level: 'low' | 'medium') => i18n.t(`jobs.trust.levelDesc.${level}`);

describe('TrustBadge — level description', () => {
  for (const level of ['low', 'medium'] as const) {
    it(`explains what "${level} trust" means, in resolved copy`, () => {
      const expected = levelDescriptionFor(level);
      // Guard the guard: if the key ever goes missing, `t` returns the key
      // itself and every containment check below would trivially pass.
      expect(expected).not.toContain('jobs.trust');

      render(<TrustBadge trust={assessment(level, ['suspiciousDomain'])} />);
      const detail = screen.getByText(/:/).textContent ?? '';

      expect(detail).toContain(expected);
      // The verdict comes before the evidence — a reader needs to know whether
      // to care before being handed a list of failed checks.
      expect(detail.indexOf(expected)).toBeLessThan(detail.indexOf('Suspicious domain'));
    });
  }

  it('gives each level its own words', () => {
    // A shared sentence would defeat the point: "treat with caution" and "worth
    // a second look" are different instructions to the reader.
    expect(levelDescriptionFor('low')).not.toBe(levelDescriptionFor('medium'));
  });

  it('still explains the level when no flags accompany it', () => {
    // Previously the whole detail block was gated on `reasons` being non-empty,
    // so a flagged-but-reasonless assessment rendered a bare badge with nothing
    // to explain it.
    render(<TrustBadge trust={assessment('low', [])} />);
    const detail = screen.getByText(/^:/).textContent ?? '';
    expect(detail).toContain(levelDescriptionFor('low'));
  });

  it('carries the same words on a non-interactive surface, via title', () => {
    // Listbox rows and in-Button placements can't host a focusable popover, but
    // must not therefore be silent — and must not say LESS than the popover
    // does, which is what checking only the flag reason here allowed.
    const { container } = render(
      <TrustBadge trust={assessment('low', ['invalidUrl'])} interactive={false} />
    );
    const title = container.querySelector('[title]')?.getAttribute('title') ?? '';
    const srText = container.querySelector('.sr-only')?.textContent ?? '';

    expect(title).toContain(levelDescriptionFor('low'));
    expect(title).toContain('Broken apply link');
    // Sighted-hover and screen-reader paths must not drift apart.
    expect(srText).toContain(title);
  });
});
