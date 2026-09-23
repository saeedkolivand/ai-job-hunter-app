import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    storage: {
      local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
    },
  },
}));

const {
  DEFAULT_FILL_CONFIRM_COPY,
  getRememberedHosts,
  rememberHost,
  forgetHost,
  shouldConfirmFill,
  hostOf,
  mountFirstFillConfirm,
} = await import('./site-memory');

const localGet = vi.mocked(browser.storage.local.get);
const localSet = vi.mocked(browser.storage.local.set);

beforeEach(() => {
  localGet.mockReset().mockResolvedValue({});
  localSet.mockReset();
});

describe('shouldConfirmFill (pure)', () => {
  it('requires confirmation for a host not yet remembered', () => {
    expect(shouldConfirmFill('acme.com', [])).toBe(true);
    expect(shouldConfirmFill('acme.com', ['other.com'])).toBe(true);
  });

  it('skips confirmation for an already-remembered host', () => {
    expect(shouldConfirmFill('acme.com', ['acme.com', 'other.com'])).toBe(false);
  });
});

describe('hostOf (pure)', () => {
  it('extracts the hostname from a url', () => {
    expect(hostOf('https://acme.com/careers/123?x=1')).toBe('acme.com');
  });

  it('returns null for an unparsable value', () => {
    expect(hostOf('not a url')).toBeNull();
    expect(hostOf(null)).toBeNull();
    expect(hostOf(undefined)).toBeNull();
  });
});

describe('DEFAULT_FILL_CONFIRM_COPY (#1226)', () => {
  it('names exactly the profile slots autofill fills — no résumé/attach/file mention', () => {
    // A résumé/attach mention would promise a gesture that goes through the
    // Documents tab's own (separate) ATTACH_CONFIRM_COPY — never here.
    expect(DEFAULT_FILL_CONFIRM_COPY).not.toMatch(/r[eé]sum[eé]|attach|\bfile\b/i);
    expect(DEFAULT_FILL_CONFIRM_COPY).toContain('name');
    expect(DEFAULT_FILL_CONFIRM_COPY).toContain('email');
    expect(DEFAULT_FILL_CONFIRM_COPY).toContain('phone');
    expect(DEFAULT_FILL_CONFIRM_COPY).toContain('location');
  });

  it('still promises nothing is submitted and that the site button is pressed by the user', () => {
    expect(DEFAULT_FILL_CONFIRM_COPY).toContain('Nothing is submitted');
    expect(DEFAULT_FILL_CONFIRM_COPY).toContain("press the site's own button");
  });
});

describe('getRememberedHosts / rememberHost / forgetHost', () => {
  it('defaults to an empty list when nothing is stored', async () => {
    expect(await getRememberedHosts()).toEqual([]);
  });

  it('ignores a malformed stored value instead of throwing', async () => {
    localGet.mockResolvedValueOnce({ fillConfirmDontAskHosts: 'not-an-array' });
    expect(await getRememberedHosts()).toEqual([]);
  });

  it('adds a host without duplicating an existing one', async () => {
    localGet.mockResolvedValueOnce({ fillConfirmDontAskHosts: ['acme.com'] });
    await rememberHost('acme.com');
    expect(localSet).toHaveBeenCalledWith({ fillConfirmDontAskHosts: ['acme.com'] });
  });

  it('removes exactly the forgotten host', async () => {
    localGet.mockResolvedValueOnce({ fillConfirmDontAskHosts: ['acme.com', 'other.com'] });
    await forgetHost('acme.com');
    expect(localSet).toHaveBeenCalledWith({ fillConfirmDontAskHosts: ['other.com'] });
  });
});

describe('mountFirstFillConfirm', () => {
  function mount() {
    const host = document.createElement('div');
    const rememberHostMock = vi.fn(() => Promise.resolve(undefined));
    const getRememberedHostsMock = vi.fn(() => Promise.resolve<string[]>([]));
    const view = mountFirstFillConfirm(host, {
      getRememberedHosts: getRememberedHostsMock,
      rememberHost: rememberHostMock,
    });
    return { host, view, rememberHostMock, getRememberedHostsMock };
  }

  // #1249: an unknown host must REFUSE, not approve. "We cannot name this
  // page" is not consent — the side panel reaches here with a null origin
  // until its first state push lands.
  it('resolves false immediately, with no UI shown, for a null host', async () => {
    const { view, host } = mount();
    await expect(view.confirm(null)).resolves.toBe(false);
    expect(host.querySelector('.inset')?.hasAttribute('hidden')).toBe(true);
  });

  it('resolves false for an empty-string host too', async () => {
    const { view } = mount();
    await expect(view.confirm('')).resolves.toBe(false);
  });

  it('resolves true immediately for an already-remembered host, without rendering', async () => {
    const { view, host, getRememberedHostsMock } = mount();
    getRememberedHostsMock.mockResolvedValueOnce(['acme.com']);
    await expect(view.confirm('acme.com')).resolves.toBe(true);
    expect(host.querySelector('.inset')?.hasAttribute('hidden')).toBe(true);
  });

  it('renders a caller-supplied copy override instead of the default Fill text (PR2 reuse)', async () => {
    const { view, host } = mount();
    const pending = view.confirm(
      'acme.com',
      'this will attach your résumé file — nothing is submitted'
    );
    await Promise.resolve();
    await Promise.resolve();

    expect(host.textContent).toContain('this will attach your résumé file — nothing is submitted');
    expect(host.textContent).not.toContain('This will fill: name, email, phone, location');

    host.querySelector<HTMLButtonElement>('.btn--primary')!.click();
    await expect(pending).resolves.toBe(true);
  });

  it('renders a caller-supplied label override in both the heading and the primary button (PR2 attach reuse)', async () => {
    const { view, host } = mount();
    const pending = view.confirm(
      'acme.com',
      'this will attach your résumé file — nothing is submitted',
      'Attach'
    );
    await Promise.resolve();
    await Promise.resolve();

    expect(host.textContent).toContain('First Attach on acme.com');
    expect(host.textContent).not.toContain('First Fill on acme.com');
    const primaryBtn = host.querySelector<HTMLButtonElement>('.btn--primary')!;
    expect(primaryBtn.textContent).toBe('Attach');

    primaryBtn.click();
    await expect(pending).resolves.toBe(true);
  });

  it('shows the inset and resolves true on Fill, remembering the host when the checkbox is checked', async () => {
    const { view, host, rememberHostMock } = mount();
    const pending = view.confirm('acme.com');
    await Promise.resolve();
    await Promise.resolve();

    expect(host.textContent).toContain('First Fill on acme.com');
    host.querySelector<HTMLInputElement>('input[type=checkbox]')!.checked = true;
    host.querySelector<HTMLButtonElement>('.btn--primary')!.click();

    await expect(pending).resolves.toBe(true);
    expect(rememberHostMock).toHaveBeenCalledWith('acme.com');
  });

  it('resolves false on Not now, without remembering the host', async () => {
    const { view, host, rememberHostMock } = mount();
    const pending = view.confirm('acme.com');
    await Promise.resolve();
    await Promise.resolve();

    host.querySelector<HTMLButtonElement>('.btn--quiet')!.click();

    await expect(pending).resolves.toBe(false);
    expect(rememberHostMock).not.toHaveBeenCalled();
  });

  it('does not remember the host on Fill when the checkbox is left unchecked', async () => {
    const { view, host, rememberHostMock } = mount();
    const pending = view.confirm('acme.com');
    await Promise.resolve();
    await Promise.resolve();

    host.querySelector<HTMLButtonElement>('.btn--primary')!.click();

    await expect(pending).resolves.toBe(true);
    expect(rememberHostMock).not.toHaveBeenCalled();
  });

  it('still resolves true (the Fill proceeds) when rememberHost rejects', async () => {
    const host = document.createElement('div');
    const rememberHostMock = vi.fn(() => Promise.reject(new Error('storage full')));
    const getRememberedHostsMock = vi.fn(() => Promise.resolve<string[]>([]));
    const view = mountFirstFillConfirm(host, {
      getRememberedHosts: getRememberedHostsMock,
      rememberHost: rememberHostMock,
    });

    const pending = view.confirm('acme.com');
    await Promise.resolve();
    await Promise.resolve();

    host.querySelector<HTMLInputElement>('input[type=checkbox]')!.checked = true;
    host.querySelector<HTMLButtonElement>('.btn--primary')!.click();

    await expect(pending).resolves.toBe(true);
    expect(rememberHostMock).toHaveBeenCalledWith('acme.com');
  });

  it('cancel() hides an open inset and resolves confirm() false, without remembering the host', async () => {
    const { view, host, rememberHostMock } = mount();
    const pending = view.confirm('acme.com');
    await Promise.resolve();
    await Promise.resolve();
    expect(host.textContent).toContain('First Fill on acme.com');

    view.cancel();

    await expect(pending).resolves.toBe(false);
    expect(rememberHostMock).not.toHaveBeenCalled();
    expect(host.querySelector('.inset')?.hasAttribute('hidden')).toBe(true);
  });

  it('cancel() is a no-op when no confirmation is open', () => {
    const { view } = mount();
    expect(() => view.cancel()).not.toThrow();
  });
});
