// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { HowItWorksBody } from './HowItWorksBody';

afterEach(() => {
  cleanup();
});

// public/scripts/how-it-works-0.js (the sidebar tab switcher + boot/flow
// players + IPC filter) and how-it-works-1.js (the console egg) both bind to
// this markup by id/class/data-attr (ADR 0018) — these ids/classes must each
// exist exactly once.
const VIEW_IDS = [
  'view-overview',
  'view-boot',
  'view-flows',
  'view-ipc',
  'view-subsystems',
  'view-cheatsheet',
];
const MOUNT_IDS = ['bootPlayer', 'flowTabs', 'flowPlayer', 'ipcBody', 'subs', 'qa'];

describe('HowItWorksBody', () => {
  it('wraps aside/main/footer in a display:contents root div', () => {
    const { container } = render(<HowItWorksBody />);
    const root = container.firstElementChild;
    expect(root?.tagName).toBe('DIV');
    expect((root as HTMLElement | null)?.style.display).toBe('contents');

    expect(container.querySelector('aside')?.parentElement).toBe(root);
    expect(container.querySelector('main')?.parentElement).toBe(root);
    expect(container.querySelector('footer')?.parentElement).toBe(root);
  });

  it('renders the top back-link and the 6-tab nav with the first tab active', () => {
    const { container } = render(<HowItWorksBody />);
    expect(container.querySelector('a.top-back')?.getAttribute('href')).toBe('/');

    const buttons = Array.from(container.querySelectorAll('#nav button[data-view]'));
    expect(buttons).toHaveLength(6);
    expect(buttons[0]?.className).toBe('active');
    expect(buttons[0]?.getAttribute('aria-current')).toBe('true');
    for (const btn of buttons.slice(1)) {
      expect(btn.className).toBe('');
      expect(btn.getAttribute('aria-current')).toBe('false');
    }
  });

  it('renders every view section and JS-populated mount point exactly once', () => {
    const { container } = render(<HowItWorksBody />);
    for (const id of VIEW_IDS) {
      expect(container.querySelectorAll(`#${id}`)).toHaveLength(1);
    }
    for (const id of MOUNT_IDS) {
      expect(container.querySelectorAll(`#${id}`)).toHaveLength(1);
    }
  });

  it('keeps the empty data-nodes attribute (boolean-style, not "true")', () => {
    const { container } = render(<HowItWorksBody />);
    const nodesEls = container.querySelectorAll('.nodes');
    expect(nodesEls.length).toBeGreaterThan(0);
    for (const el of nodesEls) {
      expect(el.getAttribute('data-nodes')).toBe('');
    }
  });

  // Line-wrapped prose in the original body.html relies on the browser's
  // whitespace collapsing; a JSX conversion can silently swallow the space at
  // a text/element line-wrap boundary (e.g. "...talks to a" + newline +
  // "<b>Rust</b>" loses the space unless written with an explicit `{' '}`).
  // Assert the collapsed textContent keeps every such boundary intact.
  it('keeps every line-wrapped inline-element boundary spaced correctly', () => {
    const { container } = render(<HowItWorksBody />);
    const collapse = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();

    expect(collapse(container.querySelector('#view-overview .lede')?.textContent)).toContain(
      'talks to a Rust core over'
    );
    expect(collapse(container.querySelector('#view-overview .lede')?.textContent)).toContain(
      'storing everything on your machine.'
    );
    expect(collapse(container.querySelector('#view-boot .lede')?.textContent)).toContain(
      'and the Rust setup() wires up all shared state'
    );
    expect(collapse(container.querySelector('#autopilotNote')?.textContent)).toContain(
      'every 60s (autopilot_scheduler.rs). For each due autopilot'
    );
    expect(collapse(container.querySelector('#autopilotNote')?.textContent)).toContain(
      'validated pipeline → apply → record the run.'
    );
    expect(collapse(container.querySelector('#view-ipc .footer')?.textContent)).toContain(
      'Source of truth: apps/desktop/src/tauri-client/namespaces/*, assembled in'
    );
    expect(collapse(container.querySelector('#view-cheatsheet .muted')?.textContent)).toContain(
      'Long jobs return a jobId immediately'
    );
  });

  it('renders the footer with all links (no "current" page to omit)', () => {
    const { container } = render(<HowItWorksBody />);
    const footLinks = container.querySelector('.foot-links');
    const hrefs = Array.from(footLinks?.querySelectorAll('a') ?? []).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).toContain('/');
    expect(hrefs).toContain('/download');
    expect(hrefs).toContain('/privacy');
  });
});
