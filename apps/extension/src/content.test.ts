/**
 * Unit tests for apps/extension/src/content.ts.
 *
 * content.ts is injected via chrome.scripting.executeScript. Its completion
 * value (what executeScript returns to the background) is
 * `document.documentElement.outerHTML`.  Pure helpers are now exported so we
 * can call the REAL implementations directly — no logic duplication in tests.
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { capture, markLikelyJobNode } from './content';

describe('content script – DOM capture', () => {
  it('capture() returns document.documentElement.outerHTML for the current document', () => {
    const result = capture();

    expect(result).toBe(document.documentElement.outerHTML);
    // Must be a non-empty HTML string.
    expect(result.length).toBeGreaterThan(0);
    expect(result).toContain('<html');
  });

  it('capture() reflects content added to document.body before the call', () => {
    // Confirm the capture always reflects the CURRENT live DOM — mirrors what
    // executeScript sees on a real page.
    const marker = 'ajh-test-marker-12345';
    const div = document.createElement('div');
    div.id = marker;
    document.body.appendChild(div);

    expect(capture()).toContain(marker);

    document.body.removeChild(div);
  });
});

describe('content script – markLikelyJobNode', () => {
  afterEach(() => {
    // Reset the whole body — several tests append elements that never get
    // the hint attribute (e.g. an unmarked decoy or a losing candidate), so
    // removing only `[data-ajh-job-root]` elements would leak those into
    // later tests (a stale `<main>` from an earlier test qualifying for the
    // >200-char gate before the current test's own element).
    document.body.innerHTML = '';
  });

  it('annotates a <main> element whose text content exceeds 200 chars', () => {
    const main = document.createElement('main');
    main.textContent = 'x'.repeat(201);
    document.body.appendChild(main);

    markLikelyJobNode();

    expect(main.getAttribute('data-ajh-job-root')).toBe('true');
    // The annotation must also appear in the captured outerHTML.
    expect(capture()).toContain('data-ajh-job-root');
  });

  it('does NOT annotate a <main> element with exactly 200 chars of text', () => {
    const main = document.createElement('main');
    main.textContent = 'x'.repeat(200); // exactly 200 — must NOT qualify (> 200 required)
    document.body.appendChild(main);

    markLikelyJobNode();

    expect(main.hasAttribute('data-ajh-job-root')).toBe(false);
  });

  it('prefers a detail-pane container (e.g. "jobs-details") over <main> — a search/list-shell view has both', () => {
    // Mirrors a LinkedIn search view: <main> wraps the whole list-shell (list
    // + detail pane) and would qualify on its own text length, but the
    // detail-pane container must win so the hint marks the SELECTED job's
    // pane, not the whole shell.
    const main = document.createElement('main');
    main.textContent = 'y'.repeat(500);
    document.body.appendChild(main);

    const pane = document.createElement('div');
    pane.className = 'jobs-details__container';
    pane.textContent = 'x'.repeat(201);
    main.appendChild(pane);

    markLikelyJobNode();

    expect(pane.getAttribute('data-ajh-job-root')).toBe('true');
    expect(main.hasAttribute('data-ajh-job-root')).toBe(false);
  });

  it('does NOT let a hidden (display:none) [class*="job-details"] node steal the hint from a visible <main>', () => {
    // Import capture runs on ANY active tab, not just known job boards — a
    // hidden decoy container (an off-screen/SEO-only block) must never win
    // the hint over real, visible content.
    const hidden = document.createElement('div');
    hidden.className = 'job-details-decoy';
    hidden.style.display = 'none';
    hidden.textContent = 'x'.repeat(201);
    document.body.appendChild(hidden);

    const main = document.createElement('main');
    main.textContent = 'y'.repeat(201);
    document.body.appendChild(main);

    markLikelyJobNode();

    expect(hidden.hasAttribute('data-ajh-job-root')).toBe(false);
    expect(main.getAttribute('data-ajh-job-root')).toBe('true');
  });

  it('does NOT let a visibility:hidden [class*="jobs-description"] node steal the hint from a visible <main>', () => {
    const hidden = document.createElement('div');
    hidden.className = 'jobs-description-decoy';
    hidden.style.visibility = 'hidden';
    hidden.textContent = 'x'.repeat(201);
    document.body.appendChild(hidden);

    const main = document.createElement('main');
    main.textContent = 'y'.repeat(201);
    document.body.appendChild(main);

    markLikelyJobNode();

    expect(hidden.hasAttribute('data-ajh-job-root')).toBe(false);
    expect(main.getAttribute('data-ajh-job-root')).toBe('true');
  });

  it('does NOT let a [class*="job-details"] node hidden by an ANCESTOR\'s display:none steal the hint from a visible <main>', () => {
    // display:none does NOT inherit — the decoy's OWN computed display can be
    // "block" even while an ancestor is display:none. Only an ancestor-chain
    // walk (not an own-style-only check) catches this.
    const hiddenAncestor = document.createElement('div');
    hiddenAncestor.style.display = 'none';
    document.body.appendChild(hiddenAncestor);

    const decoy = document.createElement('div');
    decoy.className = 'job-details-nested-decoy';
    decoy.textContent = 'x'.repeat(201);
    hiddenAncestor.appendChild(decoy);

    const main = document.createElement('main');
    main.textContent = 'y'.repeat(201);
    document.body.appendChild(main);

    markLikelyJobNode();

    expect(decoy.hasAttribute('data-ajh-job-root')).toBe(false);
    expect(main.getAttribute('data-ajh-job-root')).toBe('true');
  });
});
