// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { clusters, edges, nodes } from '@/data/architecture-map';

import { ArchitectureMap } from './ArchitectureMap';

afterEach(() => {
  cleanup();
});

function requireNode(id: string) {
  const n = nodes.find((candidate) => candidate.id === id);
  if (!n) throw new Error(`fixture node not found: ${id}`);
  return n;
}

function getNodeEl(container: HTMLElement, id: string) {
  const el = container.querySelector<SVGGElement>(`.node[data-id="${id}"]`);
  if (!el) throw new Error(`node element not found: ${id}`);
  return el;
}

// Render-smoke + the click/keyboard/filter/help interactions that go through
// handlers + React state and need no layout — they work in jsdom without
// getBoundingClientRect. Pan/zoom/drag DOES read getBoundingClientRect (jsdom
// zeroes it) so is intentionally NOT covered here; that stays live-QA-only.
describe('ArchitectureMap', () => {
  it('mounts without throwing and renders one element per node/cluster/edge', () => {
    const { container } = render(<ArchitectureMap />);
    expect(container.querySelectorAll('.node')).toHaveLength(nodes.length);
    expect(container.querySelectorAll('.clusterBox')).toHaveLength(clusters.length);
    expect(container.querySelectorAll('path.edge')).toHaveLength(edges.length);
  });

  it('renders the default sidebar overview on mount', () => {
    render(<ArchitectureMap />);
    expect(screen.getByRole('heading', { name: 'AI Job Hunter — architecture' })).toBeTruthy();
  });

  it('pins a node on click, showing its detail, then unpins on a second click', () => {
    const pinTarget = requireNode('c-main');
    const { container } = render(<ArchitectureMap />);
    const nodeEl = getNodeEl(container, pinTarget.id);

    fireEvent.click(nodeEl);
    expect(screen.getByRole('heading', { name: pinTarget.label })).toBeTruthy();
    expect(screen.getByText(pinTarget.path)).toBeTruthy();

    fireEvent.click(nodeEl);
    expect(screen.queryByRole('heading', { name: pinTarget.label })).toBeNull();
    expect(screen.getByRole('heading', { name: 'AI Job Hunter — architecture' })).toBeTruthy();
  });

  it('pins a node via keyboard Enter and clears it on Escape, announcing both', () => {
    const pinTarget = requireNode('c-main');
    const { container } = render(<ArchitectureMap />);
    const nodeEl = getNodeEl(container, pinTarget.id);
    const status = container.querySelector('#a11y-status');

    nodeEl.focus();
    fireEvent.keyDown(nodeEl, { key: 'Enter' });
    expect(screen.getByRole('heading', { name: pinTarget.label })).toBeTruthy();
    expect(status?.textContent).toBe(`${pinTarget.label} selected`);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.getByRole('heading', { name: 'AI Job Hunter — architecture' })).toBeTruthy();
    expect(status?.textContent).toBe('Selection cleared');
  });

  it('flips aria-pressed on the clicked chip and dims non-matching nodes/edges only', () => {
    // Derived from the data rather than a hardcoded id/index, so this stays
    // correct if the graph's content changes shape.
    const matchNode = nodes.find((n) => n.tag.includes('aigenerate'));
    const missNode = nodes.find((n) => !n.tag.includes('aigenerate'));
    const matchEdgeIdx = edges.findIndex((e) => e.tag.includes('aigenerate'));
    const missEdgeIdx = edges.findIndex((e) => !e.tag.includes('aigenerate'));
    if (!matchNode || !missNode || matchEdgeIdx < 0 || missEdgeIdx < 0) {
      throw new Error('fixture needs both an aigenerate-tagged and a non-tagged node/edge');
    }

    const { container } = render(<ArchitectureMap />);
    const overviewChip = screen.getByRole('button', { name: 'Overview' });
    const aiChip = screen.getByRole('button', { name: 'AI Generate' });
    expect(overviewChip.getAttribute('aria-pressed')).toBe('true');

    fireEvent.click(aiChip);

    expect(aiChip.getAttribute('aria-pressed')).toBe('true');
    expect(overviewChip.getAttribute('aria-pressed')).toBe('false');
    expect(getNodeEl(container, matchNode.id).classList.contains('dim')).toBe(false);
    expect(getNodeEl(container, missNode.id).classList.contains('dim')).toBe(true);
    expect(
      container.querySelector(`[data-edge="${matchEdgeIdx}"]`)?.classList.contains('hide')
    ).toBe(false);
    expect(
      container.querySelector(`[data-edge="${missEdgeIdx}"]`)?.classList.contains('hide')
    ).toBe(true);
  });

  it('opens the keyboard-shortcuts dialog on help click and closes it on Escape', () => {
    render(<ArchitectureMap />);
    expect(screen.queryByRole('dialog')).toBeNull();

    const helpBtn = screen.getByRole('button', { name: 'Keyboard shortcuts' });
    fireEvent.click(helpBtn);

    expect(screen.getByRole('dialog')).toBeTruthy();
    expect(helpBtn.getAttribute('aria-expanded')).toBe('true');

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });

    expect(screen.queryByRole('dialog')).toBeNull();
    expect(helpBtn.getAttribute('aria-expanded')).toBe('false');
  });
});
