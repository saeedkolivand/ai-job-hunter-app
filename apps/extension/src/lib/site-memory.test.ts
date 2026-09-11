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

  it('resolves true immediately, with no UI shown, for a null host', async () => {
    const { view, host } = mount();
    await expect(view.confirm(null)).resolves.toBe(true);
    expect(host.querySelector('.inset')?.hasAttribute('hidden')).toBe(true);
  });

  it('resolves true immediately for an already-remembered host, without rendering', async () => {
    const { view, host, getRememberedHostsMock } = mount();
    getRememberedHostsMock.mockResolvedValueOnce(['acme.com']);
    await expect(view.confirm('acme.com')).resolves.toBe(true);
    expect(host.querySelector('.inset')?.hasAttribute('hidden')).toBe(true);
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
});
