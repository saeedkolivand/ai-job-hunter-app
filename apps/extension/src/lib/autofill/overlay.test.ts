/**
 * Unit tests for autofill's in-page summary overlay, the `runAutofill` entry
 * point, the pinned global name, and the popup fields-probe's WIDER signal
 * (`hasAutofillableFields`) in apps/extension/src/lib/autofill.ts.
 */

import { afterEach, describe, expect, it } from 'vitest';

import {
  AUTOFILL_GLOBAL,
  hasAutofillableFields,
  renderSummaryOverlay,
  runAutofill,
} from '../autofill';
import { labelled, PROFILE, resetDocument, setForm, val } from './test-support';

afterEach(resetDocument);

const overlay = () => document.getElementById('ajh-autofill-overlay');
const NOTHING = { filled: [], nameSplit: null, filledNothing: true };

describe('renderSummaryOverlay', () => {
  it('renders a dismissable overlay listing the filled fields', () => {
    renderSummaryOverlay(document, {
      filled: [
        { key: 'email', label: 'Email', count: 2 },
        { key: 'firstName', label: 'First name', count: 1 },
      ],
      nameSplit: { first: 'Saeed', last: 'Kolivand' },
      filledNothing: false,
    });

    expect(overlay()).not.toBeNull();
    expect(overlay()!.textContent).toContain('Email → 2 fields');
    expect(overlay()!.textContent).toContain('First name → 1 field');
    expect(overlay()!.textContent).toContain('Name split (guess)');
    expect(overlay()!.textContent).toContain('Saeed');

    // Dismiss removes it.
    overlay()!
      .querySelector('button')!
      .dispatchEvent(new Event('click', { bubbles: true }));
    expect(overlay()).toBeNull();
  });

  it('renders the "nothing matched" message so a no-op does not look broken', () => {
    renderSummaryOverlay(document, NOTHING);
    expect(overlay()!.textContent).toContain('No matchable fields found');
  });

  it('replaces a prior overlay instead of stacking', () => {
    renderSummaryOverlay(document, NOTHING);
    renderSummaryOverlay(document, NOTHING);
    expect(document.querySelectorAll('#ajh-autofill-overlay')).toHaveLength(1);
  });

  it('notes skipped-ambiguous extra-link fields alongside a successful fill', () => {
    renderSummaryOverlay(document, {
      filled: [{ key: 'extraLink:Portfolio', label: 'Portfolio', count: 1 }],
      nameSplit: null,
      filledNothing: false,
      skippedAmbiguous: 2,
    });
    expect(overlay()!.textContent).toContain('Portfolio → 1 field');
    expect(overlay()!.textContent).toContain('2 fields skipped');
  });

  it('omits the skipped-ambiguous note when there is nothing to report', () => {
    renderSummaryOverlay(document, {
      filled: [{ key: 'email', label: 'Email', count: 1 }],
      nameSplit: null,
      filledNothing: false,
      skippedAmbiguous: 0,
    });
    expect(overlay()!.textContent).not.toContain('skipped');
  });

  it('suppresses the "no matchable fields" line when fields were skipped as ambiguous instead', () => {
    // filledNothing + skippedAmbiguous both true reads as contradictory
    // ("no matchable fields" + "N fields skipped") — the skipped-note alone
    // already explains the outcome.
    renderSummaryOverlay(document, { ...NOTHING, skippedAmbiguous: 1 });
    expect(overlay()!.textContent).not.toContain('No matchable fields found');
    expect(overlay()!.textContent).toContain('1 field skipped');
  });
});

describe('runAutofill', () => {
  it('fills the document and injects the summary overlay, returning the summary', () => {
    setForm(labelled([['email', 'Email', 'email']]));
    const summary = runAutofill(PROFILE);

    expect(val('email')).toBe('saeed@example.com');
    expect(overlay()).not.toBeNull();
    expect(summary.filledNothing).toBe(false);
    expect(summary.filled.map((f) => f.key)).toContain('email');
  });
});

describe('AUTOFILL_GLOBAL', () => {
  it('is pinned — background.ts hardcodes the same literal (kept in lockstep)', () => {
    // background.ts intentionally duplicates this literal (it cannot runtime-import
    // autofill.ts, or fill.js would gain an ES import and break classic injection).
    // If this value changes, update the local const in background.ts too.
    expect(AUTOFILL_GLOBAL).toBe('__ajhRunAutofill');
  });
});

describe("hasAutofillableFields — the popup fields-probe's WIDER signal (Form group gating)", () => {
  it.each([
    [
      'false for a page with no form fields at all (a plain job listing)',
      `<p>Senior Rust Engineer at Acme Corp.</p>`,
      false,
    ],
    [
      "true for an IDENTITY-ONLY form (name/email/phone) — the exact case answers-capture's narrower signal misses, since it excludes identity fields by design",
      labelled([
        ['name', 'Full name'],
        ['email', 'Email', 'email'],
        ['phone', 'Phone', 'tel'],
      ]),
      true,
    ],
    ['true when only ONE identity field is present', labelled([['email', 'Email', 'email']]), true],
    [
      'false when every input is non-identity/ambiguous/hidden (nothing autofill would ever touch)',
      `
      ${labelled([
        ['q1', 'Why this role?'],
        ['pw', 'Password', 'password'],
      ])}
      <label for="h">Honeypot email</label><input id="h" type="email" value="" style="display:none" />`,
      false,
    ],
    [
      'false for an already-FILLED identity field (autofill never overwrites — nothing left for Fill to do)',
      `<label for="email">Email</label><input id="email" type="email" value="already@example.com" />`,
      false,
    ],
  ])('returns %s', (_name, html, expected) => {
    setForm(html);
    expect(hasAutofillableFields(document)).toBe(expected);
  });
});
