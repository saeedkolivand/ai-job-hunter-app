/**
 * "Don't ask again" memory for the first-time Fill confirmation (PR0 §4).
 *
 * Storage is hostnames ONLY (`browser.storage.local`, e.g. `"acme.com"`) —
 * never a URL, never PII. A host is remembered only after the user explicitly
 * ticks the checkbox on the confirmation inset; it is forgettable one at a
 * time from Settings → Sites (`getRememberedHosts`/`forgetHost` below).
 */

import { browser } from '@wxt-dev/browser';

const REMEMBERED_HOSTS_KEY = 'fillConfirmDontAskHosts';

/** Read the set of hosts the user told us not to ask again for. */
export async function getRememberedHosts(): Promise<string[]> {
  const stored = await browser.storage.local.get(REMEMBERED_HOSTS_KEY);
  const value = stored[REMEMBERED_HOSTS_KEY];
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

/** Remember `host` — the first-time Fill confirmation is skipped for it from now on. */
export async function rememberHost(host: string): Promise<void> {
  const hosts = new Set(await getRememberedHosts());
  hosts.add(host);
  await browser.storage.local.set({ [REMEMBERED_HOSTS_KEY]: [...hosts] });
}

/** Forget `host` — the confirmation shows again next time. */
export async function forgetHost(host: string): Promise<void> {
  const hosts = (await getRememberedHosts()).filter((h) => h !== host);
  await browser.storage.local.set({ [REMEMBERED_HOSTS_KEY]: hosts });
}

/**
 * Whether the first-time Fill confirmation must be shown for `host`, given
 * the current remembered set. Pure: no storage access, no DOM.
 */
export function shouldConfirmFill(host: string, rememberedHosts: readonly string[]): boolean {
  return !rememberedHosts.includes(host);
}

/** Best-effort hostname for `url` — `null` for an unparsable/non-http(s) url
 *  (the confirmation is then skipped rather than shown for a meaningless key). */
export function hostOf(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    const parsed = new URL(url);
    return parsed.hostname || null;
  } catch {
    return null;
  }
}

// ── the confirmation inset (shared by popup + side panel) ──────────────────

export interface FirstFillConfirmDeps {
  getRememberedHosts: () => Promise<string[]>;
  rememberHost: (host: string) => Promise<void>;
}

export interface FirstFillConfirmView {
  /**
   * Ask the user to confirm the first Fill on `host`. Resolves `true`
   * immediately (no UI shown) when `host` is already remembered or `null`
   * (unparsable url); otherwise shows the inset and resolves with the user's
   * choice.
   */
  confirm: (host: string | null) => Promise<boolean>;
  /**
   * Cancel a currently-open confirmation: hide the inset and resolve its
   * pending {@link confirm} promise `false`, as if the user had clicked "Not
   * now". A no-op when nothing is open. For a caller whose followed
   * tab/page changed while the inset was up (`sidepanel.ts`'s `follow()`) —
   * the confirmation belonged to the page it opened on, never to whatever
   * the panel is now showing.
   */
  cancel: () => void;
}

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

/** Mount the (initially hidden) first-time Fill confirmation inset into `host`. */
export function mountFirstFillConfirm(
  host: HTMLElement,
  deps: FirstFillConfirmDeps
): FirstFillConfirmView {
  const inset = el('div', 'inset');
  inset.hidden = true;
  host.append(inset);

  /** The currently-open confirmation's own "resolve as Not now" step, or
   *  `null` when none is open — what {@link cancel} invokes. */
  let pendingCancel: (() => void) | null = null;

  async function confirm(siteHost: string | null): Promise<boolean> {
    if (!siteHost) return true;
    const remembered = await deps.getRememberedHosts();
    if (!shouldConfirmFill(siteHost, remembered)) return true;

    return new Promise<boolean>((resolve) => {
      inset.replaceChildren();
      inset.append(el('p', 'inset-label', `First Fill on ${siteHost}`));
      inset.append(
        el(
          'p',
          'inset-copy',
          "This will fill: name, email, phone, résumé file. Nothing is submitted — you review and press the site's own button."
        )
      );

      const actions = el('div', 'action-row');
      const fillBtn = el('button', 'btn btn--primary', 'Fill');
      fillBtn.type = 'button';
      const notNowBtn = el('button', 'btn btn--quiet', 'Not now');
      notNowBtn.type = 'button';
      actions.append(fillBtn, notNowBtn);
      inset.append(actions);

      const checkLabel = el('label', 'check');
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      const checkSpan = el('span', undefined, `Don't ask again for ${siteHost}`);
      checkLabel.append(checkbox, checkSpan);
      inset.append(checkLabel);

      const finish = async (ok: boolean): Promise<void> => {
        pendingCancel = null;
        inset.hidden = true;
        inset.replaceChildren();
        if (ok && checkbox.checked) {
          try {
            await deps.rememberHost(siteHost);
          } catch (err) {
            // Best-effort — a failed "remember" must not block the Fill the
            // user just confirmed; only the "don't ask again" convenience is
            // lost for next time.
            console.warn(
              '[ajh] remember host failed:',
              err instanceof Error ? err.name : 'unknown'
            );
          }
        }
        resolve(ok);
      };
      fillBtn.addEventListener('click', () => void finish(true));
      notNowBtn.addEventListener('click', () => void finish(false));
      pendingCancel = () => void finish(false);

      inset.hidden = false;
    });
  }

  function cancel(): void {
    pendingCancel?.();
  }

  return { confirm, cancel };
}
