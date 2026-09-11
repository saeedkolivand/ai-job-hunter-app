/**
 * A small top-tab bar, shared by every surface that needs one (PR0: the side
 * panel's Job / Answers; PR2/PR4 register Documents / Prep through the same
 * list — see the redesign spec). One `<button role="tab">` per entry with an
 * optional count badge, a `<section role="tabpanel">` per entry the caller
 * fills, and a red underline on the active tab (`popup.css`'s `.tab.active`).
 *
 * View-only: which tab is active is owned by the CALLER (e.g. persisted per
 * browser window in `storage.session`) — this module only renders whatever
 * `activeId` it is given and reports a click via `onSelect`.
 */

export interface TabSpec {
  id: string;
  label: string;
  /** Shown as a `(n)` badge next to the label when > 0 — omit/0 for no badge. */
  count?: number;
}

export interface TabsDeps {
  onSelect: (id: string) => void;
}

export interface TabsView {
  /** Re-render with `activeId` marked active — the caller decides which id
   *  that is (e.g. from `storage.session`), this module has no state of its own. */
  setActive: (activeId: string) => void;
  /** Update a tab's count badge without a full rebuild. */
  setCount: (id: string, count: number) => void;
  /** The mounted tab panel for `id` — the caller renders its content into it. */
  panel: (id: string) => HTMLElement;
}

/** Mount a tab bar for `tabs` into `host`; one panel per tab is created and
 *  appended to `host` right after the bar, in the given order. */
export function mountTabs(host: HTMLElement, tabs: readonly TabSpec[], deps: TabsDeps): TabsView {
  const bar = document.createElement('div');
  bar.className = 'tabs';
  bar.setAttribute('role', 'tablist');

  interface Entry {
    btn: HTMLButtonElement;
    panel: HTMLElement;
    label: string;
  }

  const entries = new Map<string, Entry>();

  function buttonText(spec: TabSpec): string {
    return spec.count ? `${spec.label} (${spec.count})` : spec.label;
  }

  for (const spec of tabs) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tab';
    btn.setAttribute('role', 'tab');
    btn.dataset.tab = spec.id;
    btn.textContent = buttonText(spec);
    btn.addEventListener('click', () => deps.onSelect(spec.id));
    bar.append(btn);

    const panel = document.createElement('section');
    panel.className = 'tab-body-section';
    panel.dataset.section = spec.id;
    panel.hidden = true;

    entries.set(spec.id, { btn, panel, label: spec.label });
    host.append(panel);
  }

  host.prepend(bar);

  function setActive(activeId: string): void {
    for (const [id, entry] of entries) {
      const active = id === activeId;
      entry.btn.classList.toggle('active', active);
      entry.btn.setAttribute('aria-selected', String(active));
      entry.panel.hidden = !active;
    }
  }

  function setCount(id: string, count: number): void {
    const entry = entries.get(id);
    if (!entry) return;
    entry.btn.textContent = count ? `${entry.label} (${count})` : entry.label;
  }

  function panel(id: string): HTMLElement {
    const entry = entries.get(id);
    if (!entry) throw new Error(`unknown tab id: ${id}`);
    return entry.panel;
  }

  return { setActive, setCount, panel };
}
