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
    // Remove any annotation left by the real function between tests.
    document.querySelectorAll('[data-ajh-job-root]').forEach((el) => {
      el.removeAttribute('data-ajh-job-root');
      el.remove();
    });
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
});
