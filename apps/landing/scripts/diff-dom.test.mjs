// Self-test for diff-dom.mjs — run via `node --test scripts/` (plain node:test,
// no vitest: apps/landing/scripts/** are build-time Node tooling, not app source).

import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { diffBodies } from './diff-dom.mjs';

const html = (body) => `<!doctype html><html><head></head><body>${body}</body></html>`;

describe('diff-dom', () => {
  it('treats "; " and ";" style separators as equal', () => {
    const { mismatches } = diffBodies(
      html('<div style="color: red; font-weight: bold;"></div>'),
      html('<div style="color: red;font-weight: bold"></div>')
    );
    assert.equal(mismatches.length, 0);
  });

  it('does not let a quoted semicolon mask a real style difference', () => {
    const { mismatches } = diffBodies(
      html(`<div style="content: 'a; b'; color: red;"></div>`),
      html(`<div style="content: 'a; b'; color: blue;"></div>`)
    );
    assert.equal(mismatches.length, 1);
  });

  it('treats "prop: value" and "prop:value" (React SSR) as equal', () => {
    const { mismatches } = diffBodies(
      html('<div style="border-color: var(--ui); color: var(--ui)"></div>'),
      html('<div style="border-color:var(--ui);color:var(--ui)"></div>')
    );
    assert.equal(mismatches.length, 0);
  });

  it('skips a real inline event handler (onclick) from the diff', () => {
    const { mismatches } = diffBodies(
      html('<button onclick="doThing()">go</button>'),
      html('<button>go</button>')
    );
    assert.equal(mismatches.length, 0);
  });

  it('does not skip a custom non-handler "onboarding" attribute', () => {
    const { mismatches } = diffBodies(
      html('<div onboarding="step-1"></div>'),
      html('<div onboarding="step-2"></div>')
    );
    assert.equal(mismatches.length, 1);
  });

  it('treats bare and empty-string boolean attributes as equal (hidden/inert)', () => {
    const { mismatches } = diffBodies(
      html('<div hidden inert></div>'),
      html('<div hidden="" inert=""></div>')
    );
    assert.equal(mismatches.length, 0);
  });

  it('normalizes URL colons in data URIs (background:url vs background: url)', () => {
    const { mismatches } = diffBodies(
      html('<div style="background:url(data:image/png;base64,AA==)"></div>'),
      html('<div style="background: url(data:image/png;base64,AA==)"></div>')
    );
    assert.equal(mismatches.length, 0);
  });
});
