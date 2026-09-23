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

/** The Fill confirmation's default copy — extracted so a caller reusing this
 *  SAME inset for a different page-touching gesture (PR2's résumé attach)
 *  can pass its own, while every existing Fill call site keeps this text
 *  unchanged by omitting the override. */
export const DEFAULT_FILL_CONFIRM_COPY =
  "This will fill: name, email, phone, location, and your profile links. Nothing is submitted — you review and press the site's own button.";

export interface FirstFillConfirmView {
  /**
   * Ask the user to confirm the first Fill (or, with a `copy`/`label`
   * override, a different page-touching gesture reusing this same inset —
   * e.g. PR2's résumé attach) on `host`. Resolves `true` immediately (no UI
   * shown) when `host` is already remembered or `null` (unparsable url);
   * otherwise shows the inset (with `copy`, or {@link
   * DEFAULT_FILL_CONFIRM_COPY} when omitted; `label`, default `'Fill'`, names
   * both the heading — "First {label} on {host}" — and the primary button, so
   * a reusing gesture never calls itself "First Fill" with a "Fill" button)
   * and resolves with the user's choice. The "don't ask again"
   * memory is shared across every gesture that calls this — reusing the
   * SAME per-host record, not a second one, per this module's own "reuses
   * the R6 Fill confirmation" contract.
   */
  confirm: (host: string | null, copy?: string, label?: string) => Promise<boolean>;
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

  async function confirm(
    siteHost: string | null,
    copy = DEFAULT_FILL_CONFIRM_COPY,
    label = 'Fill'
  ): Promise<boolean> {
    // Fail CLOSED on an unknown host (#1249). This used to resolve `true`,
    // which quietly turned "we don't know what page this is" into
    // "approved": the side panel's `currentOrigin` is null until its first
    // state push, so a Fill/Attach in that window wrote into a page the
    // surface could not name, with no confirmation shown. Every caller
    // (popup + panel, the only two) wants the refusal — the popup already
    // checks for this itself and says so visibly before it ever gets here.
    if (!siteHost) return false;
    const remembered = await deps.getRememberedHosts();
    if (!shouldConfirmFill(siteHost, remembered)) return true;

    return new Promise<boolean>((resolve) => {
      inset.replaceChildren();
      inset.append(el('p', 'inset-label', `First ${label} on ${siteHost}`));
      inset.append(el('p', 'inset-copy', copy));

      const actions = el('div', 'action-row');
      const fillBtn = el('button', 'btn btn--primary', label);
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
