import { describe, expect, it } from 'vitest';

import { downloadsByPlatform, installerDownloads, PLATFORM_SUFFIX } from './github-releases.mjs';

const asset = (name, download_count) => ({ name, download_count });

/** One release carrying the full seven-installer set, plus the noise GitHub attaches. */
function release(v, counts = {}) {
  return {
    assets: [
      asset(`macos-AI-Job-Hunter_${v}_aarch64-apple-silicon.dmg`, counts.macArm ?? 1),
      asset(`macos-AI-Job-Hunter_${v}_x64-intel.dmg`, counts.macIntel ?? 1),
      asset(`windows-AI-Job-Hunter_${v}_x64-setup.exe`, counts.winExe ?? 1),
      asset(`windows-AI-Job-Hunter_${v}_x64_en-US.msi`, counts.winMsi ?? 1),
      asset(`linux-AI-Job-Hunter_${v}_amd64.AppImage`, counts.linuxAppImage ?? 1),
      asset(`linux-AI-Job-Hunter_${v}_amd64.deb`, counts.linuxDeb ?? 1),
      asset(`linux-AI-Job-Hunter-${v}-1.x86_64.rpm`, counts.linuxRpm ?? 1),
      // Not installs: updater channel, signatures, extension store bundles.
      asset('latest.json', 99),
      asset(`windows-AI-Job-Hunter_${v}_x64-setup.exe.sig`, 99),
      asset(`macos-AI-Job-Hunter-apple-silicon.app.tar.gz`, 99),
      asset(`ai-job-hunter-extension-chrome-${v}.zip`, 99),
    ],
  };
}

describe('downloadsByPlatform', () => {
  // The whole reason the function exists. Every asset picks up one download
  // nobody performed, and three platforms sit at exactly the release count
  // because that floor is all they have.
  it('reports zero for a platform that only ever saw the automated download', () => {
    const counts = downloadsByPlatform([release('1.0.0'), release('1.0.1'), release('1.0.2')]);
    expect(counts).toEqual({
      macArm: 0,
      macIntel: 0,
      winExe: 0,
      winMsi: 0,
      linuxAppImage: 0,
      linuxDeb: 0,
      linuxRpm: 0,
    });
    // Raw, this fixture would read 3 per platform — the exact overstatement
    // that would have shipped: 21 downloads claimed, 0 real.
    expect(installerDownloads([release('1.0.0'), release('1.0.1'), release('1.0.2')])).toBe(21);
  });

  it('accumulates real downloads across releases, one floor per asset', () => {
    const counts = downloadsByPlatform([
      release('1.0.0', { winExe: 40, macArm: 3 }),
      release('1.0.1', { winExe: 12 }),
    ]);
    expect(counts.winExe).toBe(50); // (40-1) + (12-1)
    expect(counts.macArm).toBe(2); // (3-1) + (1-1)
    expect(counts.linuxDeb).toBe(0);
  });

  it('clamps at zero rather than assuming the floor is always present', () => {
    // Four such assets exist in the real data, so the subtraction must not
    // be allowed to go negative.
    expect(downloadsByPlatform([release('1.0.0', { linuxRpm: 0 })]).linuxRpm).toBe(0);
  });

  it('ignores updater, signature and extension assets entirely', () => {
    // Every non-installer above carries 99; none of it may reach a bucket.
    const counts = downloadsByPlatform([release('1.0.0')]);
    expect(Object.values(counts).reduce((a, b) => a + b, 0)).toBe(0);
  });

  // A rename upstream must break the build, not publish a badge quietly
  // missing a platform.
  it('throws when an installer matches no known platform suffix', () => {
    const renamed = { assets: [asset('windows-AI-Job-Hunter_2.0.0_arm64-setup.exe', 5)] };
    expect(() => downloadsByPlatform([renamed])).toThrow(/matched 0 platform suffixes/);
  });

  // The `hits.length > 1` half of that guard is deliberately NOT exercised with
  // a fixture, because no fixture can reach it: a name can only end with two
  // suffixes if one suffix ends with the other, and the test below proves none
  // does. Writing a "two matches" case meant contriving a string that quietly
  // matched only once and asserting a throw that never came — a test that
  // passes for the wrong reason. The structural property is the real guard, so
  // it is asserted directly instead.
  it('has no suffix that ends with another, which is what keeps matching unambiguous', () => {
    const keys = Object.keys(PLATFORM_SUFFIX);
    expect(keys).toHaveLength(7);
    for (const a of keys) {
      for (const b of keys) {
        if (a === b) continue;
        // If one suffix ended with another, a single asset would match twice
        // and every real run would throw.
        expect(PLATFORM_SUFFIX[a].endsWith(PLATFORM_SUFFIX[b])).toBe(false);
      }
    }
  });
});
