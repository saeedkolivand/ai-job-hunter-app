/**
 * Shared fixtures for the autofill suites: a real form built in the shared jsdom
 * `document`, the REAL `planAndFill` run against it, and tiny assertion helpers
 * over what each field ended up holding.
 */

import { expect } from 'vitest';

import { type AutofillProfile, type AutofillSummary, planAndFill } from '../autofill';

export const PROFILE: AutofillProfile = {
  fullName: 'Saeed Kolivand',
  email: 'saeed@example.com',
  phone: '+31612345678',
  location: 'Amsterdam, Netherlands',
  linkedin: 'https://linkedin.com/in/saeed',
  github: 'https://github.com/saeed',
  website: 'https://saeed.dev',
};

export function setForm(html: string): void {
  document.body.innerHTML = `<form>${html}</form>`;
}

export function val(id: string): string {
  return (document.getElementById(id) as HTMLInputElement).value;
}

/** Remove the form and any `<style>` a honeypot-CSS test injected (call from `afterEach`). */
export function resetDocument(): void {
  document.body.innerHTML = '';
  document.head.querySelectorAll('style[data-ajh-test]').forEach((s) => s.remove());
}

/** Inject a stylesheet tagged for removal by {@link resetDocument}. */
export function addStyle(css: string): void {
  const style = document.createElement('style');
  style.setAttribute('data-ajh-test', '');
  style.textContent = css;
  document.head.appendChild(style);
}

/** Build the form from `html`, run `planAndFill` against `profile`, return its summary. */
export function fill(html: string, profile: AutofillProfile = PROFILE): AutofillSummary {
  setForm(html);
  return planAndFill(document, profile);
}

/** `<label for=id>label</label><input id=id type=type>` for each `[id, label, type]`. */
export const labelled = (rows: [id: string, label: string, type?: string][]): string =>
  rows
    .map(
      ([id, label, type = 'text']) =>
        `<label for="${id}">${label}</label><input id="${id}" type="${type}" />`
    )
    .join('\n');

/** `<input id=id [attr]=value>` for each `[id, attr, value]`. */
export const withAttr = (attr: string, rows: [id: string, value: string][]): string =>
  rows.map(([id, value]) => `<input id="${id}" ${attr}="${value}" />`).join('\n');

/** Every `id` holds exactly the given value. */
export function expectValues(expected: Record<string, string>): void {
  for (const [id, value] of Object.entries(expected)) expect(val(id), id).toBe(value);
}

/** Every `id` was left empty. */
export function expectEmpty(...ids: string[]): void {
  for (const id of ids) expect(val(id), id).toBe('');
}
