/**
 * The MV3 permission surface is pinned, and the README's published table agrees
 * with it.
 *
 * ## Why this exists
 *
 * `manifest.ts` is the source of truth for a set of claims the project makes
 * OUTSIDE this repo: to Chrome Web Store and AMO reviewers, in the extension
 * README's "Permissions — minimal & justified" table, and on the landing site's
 * privacy page. Widening a permission is a two-line edit that changes what the
 * extension is allowed to do to every page a user visits — and nothing asserted
 * any of it, so the widened build would reach store review, and the user, with
 * the old justification still printed beside it.
 *
 * Store review is also the wrong place to find out. A rejection costs a release
 * cycle; an ACCEPTED over-broad permission costs the claim itself.
 *
 * ## What is pinned, and why in this shape
 *
 * The permission list is written out **by hand** rather than derived from the
 * manifest. A test that loops over the thing it guards can only ever catch
 * additions to that list — delete an entry and the assertion deletes itself.
 * The literal is the part that notices a removal; the checks around it are what
 * notice an addition the literal was lazily updated to accept:
 *
 *  * a **denylist** of permissions this extension must never hold, so adding
 *    `tabs` or `<all_urls>` fails even if someone also edited the literal;
 *  * host permissions checked as a **property** — every entry must parse to
 *    loopback — rather than as string equality, so `http://127.0.0.1.evil.com/*`
 *    cannot pass by looking right;
 *  * both browser targets compared to each other, so a per-target delta cannot
 *    smuggle a permission into one build only.
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { type BrowserTarget, buildManifest } from './manifest';

const HERE = dirname(fileURLToPath(import.meta.url));
const README = join(HERE, '..', 'README.md');

/**
 * Every permission this extension is allowed to request, spelled out.
 *
 * Adding one here is the moment to ask whether the README table, the store
 * listing and the privacy page still tell the truth.
 */
const ALLOWED_PERMISSIONS = ['activeTab', 'storage', 'scripting', 'nativeMessaging'];

/**
 * Permissions that must never appear, whatever the literal above says.
 *
 * This is the half that survives a careless update: bumping `ALLOWED_PERMISSIONS`
 * to include `tabs` would satisfy the equality check and fail here. Each of these
 * would contradict a claim the README makes in print, or hand the extension
 * standing access it has never needed — it reads one tab, on one click.
 */
const FORBIDDEN_PERMISSIONS = [
  '<all_urls>',
  'tabs',
  'webRequest',
  'webRequestBlocking',
  'cookies',
  'history',
  'bookmarks',
  'downloads',
  'management',
  'debugger',
  'proxy',
  'privacy',
  'clipboardRead',
  'geolocation',
  'declarativeNetRequest',
];

const TARGETS: BrowserTarget[] = ['chrome', 'firefox'];

const permissionsOf = (t: BrowserTarget) => buildManifest(t).permissions as string[];
const hostPermissionsOf = (t: BrowserTarget) => buildManifest(t).host_permissions as string[];

describe.each(TARGETS)('%s manifest — permission surface', (target) => {
  it('requests exactly the permissions this extension is allowed to hold', () => {
    expect([...permissionsOf(target)].sort()).toEqual([...ALLOWED_PERMISSIONS].sort());
  });

  it('holds none of the permissions the README says it does not', () => {
    const held = new Set(permissionsOf(target));
    const violations = FORBIDDEN_PERMISSIONS.filter((p) => held.has(p));

    expect(
      violations,
      'The README states in print that this extension requests no broad host access, no `tabs` ' +
        'and no `webRequest`. Holding one of these makes that false wherever it is published.'
    ).toEqual([]);
  });

  it('scopes host access to loopback, by parsing rather than by spelling', () => {
    const hosts = hostPermissionsOf(target);
    expect(hosts.length).toBeGreaterThan(0);

    for (const pattern of hosts) {
      // A match pattern is not a URL: strip the trailing path glob so `URL` can
      // parse it, then judge the HOST. String equality against a known-good list
      // would accept `http://127.0.0.1.evil.com/*`, which is a different origin
      // entirely and merely starts the same way.
      const { protocol, hostname } = new URL(pattern.replace(/\/\*$/, '/'));
      expect(hostname, `host permission is not loopback: ${pattern}`).toBe('127.0.0.1');
      expect(['ws:', 'http:'], `unexpected scheme in ${pattern}`).toContain(protocol);
    }
  });

  it('declares MV3 and does not loosen the content security policy', () => {
    const manifest = buildManifest(target);

    expect(manifest.manifest_version).toBe(3);
    // Absent means "the default", which is the strict one. Any value here is a
    // deliberate loosening and must be argued for, not inherited.
    expect(manifest.content_security_policy).toBeUndefined();
  });

  it('declares no content scripts, so nothing runs on a page unbidden', () => {
    // Scan mode injects via `chrome.scripting.executeScript` on a click, bounded
    // by `activeTab`. A static `content_scripts` block would run on every page
    // matching its pattern with no user action at all — a different product.
    expect(buildManifest(target).content_scripts).toBeUndefined();
  });
});

describe('both targets', () => {
  it('share one permission surface, so a per-target delta cannot smuggle one in', () => {
    expect([...permissionsOf('chrome')].sort()).toEqual([...permissionsOf('firefox')].sort());
    expect([...hostPermissionsOf('chrome')].sort()).toEqual(
      [...hostPermissionsOf('firefox')].sort()
    );
  });
});

describe('firefox — AMO data-collection declaration', () => {
  it('declares no data collection, which is the claim the privacy page also makes', () => {
    const gecko = (
      buildManifest('firefox').browser_specific_settings as {
        gecko: { data_collection_permissions?: { required?: string[] } };
      }
    ).gecko;

    // `['none']` is an assertion to AMO reviewers that nothing leaves the device
    // to the developer or a third party. It stays true only while the extension
    // neither stores nor transmits profile data — if that ever changes, this test
    // failing is the intended prompt to declare the real categories instead of
    // shipping a false one.
    expect(gecko.data_collection_permissions?.required).toEqual(['none']);
  });
});

describe('README parity', () => {
  const readme = readFileSync(README, 'utf8');

  it('documents every permission the manifest requests', () => {
    // The README table is the human-readable justification a store reviewer and a
    // user actually read. A permission present in the build and absent from the
    // table is an undocumented capability.
    const undocumented = ALLOWED_PERMISSIONS.filter((p) => !readme.includes(`\`${p}\``));

    expect(undocumented, 'requested but not justified in the README table').toEqual([]);
  });

  it('documents every host permission the manifest requests', () => {
    const undocumented = hostPermissionsOf('chrome').filter((h) => !readme.includes(h));

    expect(undocumented, 'host permission not documented in the README').toEqual([]);
  });

  it('still makes the negative claims this test enforces', () => {
    // If someone deletes the "we do not request..." paragraph, the denylist above
    // is still enforced but no longer corresponds to anything published — and the
    // next reader has no way to know the guard was ever meant to protect a promise.
    for (const claim of ['`<all_urls>`', '`tabs`', '`webRequest`', 'content_security_policy']) {
      expect(readme, `the README no longer claims to avoid ${claim}`).toContain(claim);
    }
  });
});
