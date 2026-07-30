import { type OnFn, ZOOM_FACTOR } from './geometry';
import type { ViewportControls } from './viewport';

export const ARROW_STEP = 64; // px per arrow-key pan
export const ARROW_STEP_FAST = 160; // px per arrow-key pan while holding Shift

export interface KeyboardOpts {
  viewport: Pick<ViewportControls, 'fit' | 'zoomAt' | 'panBy' | 'stageCenter'>;
  onEscape: () => void;
}

// Keyboard shortcuts + the "?" help overlay's own focus trap. `root` is the
// component's outer element, scoping every querySelector like the rest of the
// interaction engine.
export function attachKeyboardShortcuts(root: HTMLElement, on: OnFn, opts: KeyboardOpts): void {
  const helpEl = root.querySelector<HTMLDivElement>('#kbd-help');
  const helpBtn = root.querySelector<HTMLButtonElement>('#help-btn');
  const helpClose = root.querySelector<HTMLButtonElement>('#kbd-help-close');

  const helpFocusables = (): HTMLElement[] =>
    helpEl
      ? Array.from(
          helpEl.querySelectorAll<HTMLElement>(
            'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
          )
        ).filter((el) => !el.hasAttribute('disabled'))
      : [];
  const openHelp = () => {
    if (!helpEl) return;
    helpEl.removeAttribute('hidden');
    helpBtn?.setAttribute('aria-expanded', 'true');
    helpFocusables()[0]?.focus();
  };
  const closeHelp = () => {
    if (!helpEl) return;
    helpEl.setAttribute('hidden', '');
    helpBtn?.setAttribute('aria-expanded', 'false');
    helpBtn?.focus();
  };
  const toggleHelp = () => {
    if (!helpEl) return;
    if (helpEl.hasAttribute('hidden')) openHelp();
    else closeHelp();
  };
  if (helpBtn) on(helpBtn, 'click', toggleHelp);
  if (helpClose) on(helpClose, 'click', closeHelp);
  if (helpEl) {
    on(helpEl, 'keydown', (ev) => {
      const ke = ev as KeyboardEvent;
      if (ke.key === 'Escape') {
        ke.stopPropagation();
        closeHelp();
        return;
      }
      if (ke.key !== 'Tab') return;
      const focusable = helpFocusables();
      const first = focusable[0];
      const lastEl = focusable[focusable.length - 1];
      if (!first || !lastEl) {
        ke.preventDefault();
        return;
      }
      if (ke.shiftKey) {
        if (document.activeElement === first) {
          ke.preventDefault();
          lastEl.focus();
        }
      } else if (document.activeElement === lastEl) {
        ke.preventDefault();
        first.focus();
      }
    });
  }

  on(window, 'keydown', (ev) => {
    const ke = ev as KeyboardEvent;
    const t = ke.target as HTMLElement | null;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
    if (ke.metaKey || ke.ctrlKey || ke.altKey) return;
    if (helpEl && !helpEl.hasAttribute('hidden')) {
      if (ke.key === 'Escape') {
        ke.preventDefault();
        closeHelp();
      }
      return;
    }
    const [cx, cy] = opts.viewport.stageCenter();
    const step = ke.shiftKey ? ARROW_STEP_FAST : ARROW_STEP;
    switch (ke.key) {
      case 'ArrowLeft':
        opts.viewport.panBy(step, 0);
        ke.preventDefault();
        break;
      case 'ArrowRight':
        opts.viewport.panBy(-step, 0);
        ke.preventDefault();
        break;
      case 'ArrowUp':
        opts.viewport.panBy(0, step);
        ke.preventDefault();
        break;
      case 'ArrowDown':
        opts.viewport.panBy(0, -step);
        ke.preventDefault();
        break;
      case '+':
      case '=':
        opts.viewport.zoomAt(cx, cy, ZOOM_FACTOR);
        ke.preventDefault();
        break;
      case '-':
      case '_':
        opts.viewport.zoomAt(cx, cy, 1 / ZOOM_FACTOR);
        ke.preventDefault();
        break;
      case '0':
      case 'f':
      case 'F':
        opts.viewport.fit();
        ke.preventDefault();
        break;
      case '?':
        toggleHelp();
        ke.preventDefault();
        break;
      case 'Escape':
        opts.onEscape();
        ke.preventDefault();
        break;
    }
  });
}
