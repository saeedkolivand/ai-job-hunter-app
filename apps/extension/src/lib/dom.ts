import { browser } from '@wxt-dev/browser';

/**
 * Plain-DOM builders shared by the panel's tabs (the extension has no React or
 * `@ajh/ui` — see `answer-tools.ts`'s doc). Text goes in via `textContent`
 * only, never `innerHTML`, so page- or desktop-derived strings stay inert.
 */

export const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

export const button = (className: string, label: string): HTMLButtonElement => {
  const b = el('button', className, label);
  b.type = 'button';
  return b;
};

/** Open an `ajh://` deep link in a new tab. Best-effort: a failure is a no-op, same
 *  discipline as `connection-status.ts`'s own deep links. */
export async function openDeepLink(url: string): Promise<void> {
  try {
    await browser.tabs.create({ url });
  } catch {
    // No-op: the app may simply not be installed.
  }
}

/** A `field-label` wrapping a `<select>` of `[value, label]` options. */
export function selectField(
  labelText: string,
  options: Iterable<readonly [value: string, label: string]>,
  value: string,
  onChange: (value: string) => void
): HTMLLabelElement {
  const select = el('select');
  for (const [optValue, optLabel] of options) {
    const opt = el('option', undefined, optLabel);
    opt.value = optValue;
    select.append(opt);
  }
  select.value = value;
  select.addEventListener('change', () => onChange(select.value));
  const label = el('label', 'field-label', labelText);
  label.append(select);
  return label;
}

/**
 * The empty state shared by the Documents and Prep tabs: "Loading…" ONLY while
 * a refresh is genuinely in flight (never at mount, after a settled refresh or
 * after a reset — #1225), otherwise the status line plus an optional deep-link
 * button. A button, not an `<a href="ajh://…">`: a raw custom-scheme anchor is
 * unverified cross-browser and can silently no-op, whereas `tabs.create` in a
 * click handler is the trigger the rest of the extension already relies on.
 */
export function appendEmptyState(
  host: HTMLElement,
  loading: boolean,
  statusText: string,
  link: { label: string; onClick: () => void } | null
): void {
  if (loading) {
    host.append(el('p', 'msg msg--muted', 'Loading…'));
  } else if (statusText) {
    host.append(el('p', 'msg msg--muted', statusText));
    if (link) {
      const btn = button('btn btn--quiet', link.label);
      btn.addEventListener('click', link.onClick);
      host.append(btn);
    }
  }
}
