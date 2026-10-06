/** RewritePopover — anchored portal mode (`anchorEl`). */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import { renderPopover } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('motion/react', async () => (await import('./test-mocks')).motionMock());
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-mocks')).uiMock(await importOriginal())
);

describe('RewritePopover — anchored portal (anchorEl)', () => {
  const originalGetBoundingClientRect = Element.prototype.getBoundingClientRect;
  let anchorEl: HTMLButtonElement;

  /** A trigger that reports `rect`, appended to the document. */
  function mountAnchor(rect: Pick<DOMRect, 'top' | 'bottom' | 'left' | 'right'>) {
    anchorEl = document.createElement('button');
    anchorEl.getBoundingClientRect = () => ({ ...rect, width: 100, height: 20 }) as DOMRect;
    document.body.appendChild(anchorEl);
  }

  /**
   * jsdom performs no layout, so every element's real getBoundingClientRect()
   * reads as zeros. Stub it globally to a realistic popover panel height; the
   * trigger overrides its own instance method (takes precedence over this
   * prototype stub) to report its own, separately-controlled rect.
   */
  function stubPanelHeight(height: number) {
    Element.prototype.getBoundingClientRect = () =>
      ({ top: 0, left: 0, right: 352, bottom: height, width: 352, height }) as DOMRect;
  }

  afterEach(() => {
    anchorEl?.remove();
    Element.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    vi.clearAllMocks();
  });

  it('renders inline (inside its own container) when anchorEl is not set', () => {
    const { container } = renderPopover();
    expect(container.contains(screen.getByRole('dialog'))).toBe(true);
  });

  it('portals to document.body, fixed-positions off the trigger rect, and lifts z-toast', () => {
    mountAnchor({ top: 100, bottom: 120, left: 200, right: 300 });

    const { container } = renderPopover({ anchorEl });

    const dialog = screen.getByRole('dialog');
    // Portaled OUT of the render container (a document.body sibling), not inline.
    expect(container.contains(dialog)).toBe(false);
    expect(dialog.className).toContain('z-toast');
    expect(dialog.style.position).toBe('fixed');
    expect(dialog.style.top).toBe('124px'); // rect.bottom (120) + 4px gap
    expect(dialog.style.left).toBe('8px'); // clamped — rect.right (300) - panel width would go negative
  });

  it('flips the popover upward when it would not fit below the trigger', () => {
    // ~380px matches header + selection echo + presets + input + footer.
    stubPanelHeight(380);
    // Trigger sits near the bottom of jsdom's default 768px-tall viewport —
    // opening below (720 + 380 + 4 = 1104) would push the footer far offscreen.
    mountAnchor({ top: 700, bottom: 720, left: 200, right: 300 });

    renderPopover({ anchorEl });

    const dialog = screen.getByRole('dialog');
    // Flipped above: anchorRect.top (700) - panelHeight (380) - 4px gap = 316.
    expect(dialog.style.top).toBe('316px');
    expect(Number(dialog.style.top.replace('px', ''))).toBeGreaterThanOrEqual(8);
  });

  it('clamps the flipped-upward position to never go above the 8px viewport margin', () => {
    // A trigger near the very TOP of the viewport with a panel taller than the
    // whole viewport: it doesn't fit below (forcing a flip), and flipping
    // naively (anchorRect.top - panelHeight - 4) would go negative — must
    // clamp to 8px instead of running off the top edge.
    stubPanelHeight(750);
    mountAnchor({ top: 20, bottom: 40, left: 200, right: 300 });

    renderPopover({ anchorEl });

    expect(screen.getByRole('dialog').style.top).toBe('8px');
  });
});
