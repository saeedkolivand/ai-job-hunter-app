// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { AGENT_COUNT, BY_NAME } from '@/data/agent-fleet';

import { FleetMapSection } from './FleetMapSection';

// buildLinks() (useMapLinks) reads window.matchMedia synchronously on mount —
// see the AgentFleet.test.tsx header note; jsdom provides neither matchMedia
// nor real layout geometry (getBoundingClientRect returns all-zero rects,
// which is fine here since we only assert class wiring, never coordinates).
function stubDesktopMatchMedia() {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: false,
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }));
}

function stubClipboard() {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  return writeText;
}

beforeEach(stubDesktopMatchMedia);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, 'clipboard');
});

const AUTHOR = 'rust-backend-author';
const CRITIC = 'rust-backend-architect';
const authorTuple = BY_NAME.get(AUTHOR);
if (!authorTuple) throw new Error(`fixture agent not found in BY_NAME: ${AUTHOR}`);

describe('FleetMapSection — data-driven rendering', () => {
  it('renders exactly one node per PAIRS author/critic plus CROSS_NODES, matching AGENT_COUNT', () => {
    const { container } = render(<FleetMapSection />);
    expect(container.querySelectorAll('.node')).toHaveLength(AGENT_COUNT);
  });

  it('shows the AGENT_COUNT placeholder before any node is selected', () => {
    render(<FleetMapSection />);
    expect(
      screen.getByText((t) => t.includes(`every one of the ${AGENT_COUNT} is here`))
    ).toBeTruthy();
  });
});

describe('FleetMapSection — selection wiring', () => {
  it('clicking a node sets aria-expanded and renders its detail panel from the data', () => {
    const { container } = render(<FleetMapSection />);
    const btn = screen.getByRole('button', { name: AUTHOR });

    fireEvent.click(btn);

    expect(btn.getAttribute('aria-expanded')).toBe('true');
    expect(container.querySelector('.d-name')?.textContent).toBe(authorTuple[0]);
    expect(container.querySelector('.d-role')?.textContent).toBe(authorTuple[2]);
    const copyChip = container.querySelector('.copy-cmd');
    expect(copyChip?.textContent).toBe(authorTuple[5]);
    expect(copyChip?.getAttribute('data-copy')).toBe(authorTuple[5]);
  });

  it('moving the selection to another node clears aria-expanded on the previous one', () => {
    render(<FleetMapSection />);
    const first = screen.getByRole('button', { name: AUTHOR });
    const second = screen.getByRole('button', { name: CRITIC });

    fireEvent.click(first);
    expect(first.getAttribute('aria-expanded')).toBe('true');

    fireEvent.click(second);
    expect(first.getAttribute('aria-expanded')).toBe('false');
    expect(second.getAttribute('aria-expanded')).toBe('true');
  });

  it('hovering a paired node lights its link path, and clears it on mouse-leave', () => {
    const { container } = render(<FleetMapSection />);
    const btn = screen.getByRole('button', { name: AUTHOR });
    const path = container.querySelector(`path[data-pair="${AUTHOR}|${CRITIC}"]`);
    if (!path) throw new Error('expected a link path for the author/critic pair');
    expect(path.classList.contains('lit')).toBe(false);

    fireEvent.mouseEnter(btn);
    expect(path.classList.contains('lit')).toBe(true);

    fireEvent.mouseLeave(btn);
    expect(path.classList.contains('lit')).toBe(false);
  });
});

describe('FleetMapSection — copy chip wiring', () => {
  it('copies the delegate example on click', () => {
    const writeText = stubClipboard();
    const { container } = render(<FleetMapSection />);
    fireEvent.click(screen.getByRole('button', { name: AUTHOR }));
    const chip = container.querySelector('.copy-cmd');
    if (!chip) throw new Error('expected a copy chip once a node is selected');

    fireEvent.click(chip);

    expect(writeText).toHaveBeenCalledWith(authorTuple[5]);
  });

  it.each(['Enter', ' '])('copies on key "%s"', (key) => {
    const writeText = stubClipboard();
    const { container } = render(<FleetMapSection />);
    fireEvent.click(screen.getByRole('button', { name: AUTHOR }));
    const chip = container.querySelector('.copy-cmd');
    if (!chip) throw new Error('expected a copy chip once a node is selected');

    fireEvent.keyDown(chip, { key });

    expect(writeText).toHaveBeenCalledWith(authorTuple[5]);
  });

  it.each(['Escape', 'Tab', 'a'])('does not copy on key "%s"', (key) => {
    const writeText = stubClipboard();
    const { container } = render(<FleetMapSection />);
    fireEvent.click(screen.getByRole('button', { name: AUTHOR }));
    const chip = container.querySelector('.copy-cmd');
    if (!chip) throw new Error('expected a copy chip once a node is selected');

    fireEvent.keyDown(chip, { key });

    expect(writeText).not.toHaveBeenCalled();
  });
});

describe('FleetMapSection — NodeButton identity (the #916 remount regression)', () => {
  it('does not remount an already-focused node when an unrelated hover/selection re-render fires', () => {
    render(<FleetMapSection />);
    const focused = screen.getByRole('button', { name: AUTHOR });
    const other = screen.getByRole('button', { name: CRITIC });

    focused.focus();
    expect(document.activeElement).toBe(focused);

    // Hovering a *different* node updates FleetMapSection's `litName` state,
    // re-rendering the whole node list. If NodeButton were still defined
    // inside FleetMapSection's render body (the pre-#916 shape), React would
    // see a brand-new component type at every node's position on this
    // re-render and remount them all, destroying `focused`'s DOM node and
    // losing focus. Verified against a local harness that this genuinely
    // fails for an inline-defined child component (see PR notes).
    fireEvent.mouseEnter(other);

    expect(document.activeElement).toBe(focused);
  });

  it('also survives a re-render triggered by clicking (selecting) a different node', () => {
    render(<FleetMapSection />);
    const focused = screen.getByRole('button', { name: AUTHOR });
    const other = screen.getByRole('button', { name: CRITIC });

    focused.focus();
    fireEvent.click(other);

    expect(document.activeElement).toBe(focused);
  });
});
