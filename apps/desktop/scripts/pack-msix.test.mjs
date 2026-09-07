import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterAll, describe, expect, it } from 'vitest';

import {
  compareSdkVersions,
  fillManifest,
  findMakeappx,
  packMsix,
  readIdentity,
  rel,
  scrubPaths,
  stageMsix,
  toStoreVersion,
} from './pack-msix.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));

const IDENTITY = {
  MSIX_IDENTITY_NAME: '12345Publisher.AIJobHunter',
  MSIX_PUBLISHER: 'CN=ABCDEF01-2345-6789-ABCD-EF0123456789',
  MSIX_PUBLISHER_DISPLAY_NAME: 'Example Publisher',
};

/** Scratch output root + a placeholder exe, never the real build tree. */
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'ajh-msix-'));
const placeholderExe = path.join(scratch, 'placeholder.exe');
fs.writeFileSync(placeholderExe, 'MZ placeholder');
// The SDK-discovery variables are copied over KEY BY KEY, not by spreading
// `process.env`: inside a vitest worker `process.env['ProgramFiles(x86)']`
// reads fine but does not survive `{ ...process.env }` (the parenthesised name
// is missing from the spread — measured, 174 other keys present). Naming the
// three variables the script actually reads is also the hermetic choice.
const packEnv = {
  ...IDENTITY,
  ProgramFiles: process.env.ProgramFiles,
  'ProgramFiles(x86)': process.env['ProgramFiles(x86)'],
  MAKEAPPX: process.env.MAKEAPPX,
  AJH_EXE: placeholderExe,
  AJH_MSIX_OUT: scratch,
};

afterAll(() => {
  fs.rmSync(scratch, { recursive: true, force: true });
});

describe('toStoreVersion', () => {
  it('appends the reserved zero revision', () => {
    expect(toStoreVersion('0.148.0')).toBe('0.148.0.0');
    expect(toStoreVersion('1.0.0')).toBe('1.0.0.0');
  });

  // Each of these would otherwise produce a package the Store rejects only
  // after upload, so the failure has to happen here.
  it.each(['0.148.0-rc.1', '0.148', '0.148.0.0', 'v0.148.0', ''])('rejects %o', (version) => {
    expect(() => toStoreVersion(version)).toThrow(/three-part release version/);
  });
});

describe('fillManifest', () => {
  const template =
    '<Identity Name="{{IDENTITY_NAME}}" Publisher="{{PUBLISHER}}" Version="{{VERSION}}" />' +
    '<PublisherDisplayName>{{PUBLISHER_DISPLAY_NAME}}</PublisherDisplayName>';

  it('substitutes every placeholder', () => {
    const filled = fillManifest(template, {
      IDENTITY_NAME: 'Contoso.AIJobHunter',
      PUBLISHER: 'CN=Contoso',
      PUBLISHER_DISPLAY_NAME: 'Contoso',
      VERSION: '0.148.0.0',
    });
    expect(filled).not.toContain('{{');
    expect(filled).toContain('Name="Contoso.AIJobHunter"');
    expect(filled).toContain('Publisher="CN=Contoso"');
    expect(filled).toContain('Version="0.148.0.0"');
    expect(filled).toContain('<PublisherDisplayName>Contoso</PublisherDisplayName>');
  });

  it('escapes XML metacharacters in a publisher display name', () => {
    const filled = fillManifest(template, {
      IDENTITY_NAME: 'a',
      PUBLISHER: 'CN=a',
      PUBLISHER_DISPLAY_NAME: 'Ampersand & Co',
      VERSION: '1.0.0.0',
    });
    expect(filled).toContain('Ampersand &amp; Co');
  });

  // A real Partner Center subject carries commas and quotes. Both land INSIDE
  // an XML attribute here, so an unescaped one would either truncate the
  // attribute or produce a manifest that does not parse at all.
  it('escapes quotes in a publisher subject so the attribute survives', () => {
    const filled = fillManifest(template, {
      IDENTITY_NAME: 'a',
      PUBLISHER: 'CN=Foo, O="Acme, Inc.", L=O\'Fallon',
      PUBLISHER_DISPLAY_NAME: "O'Fallon & Sons",
      VERSION: '1.0.0.0',
    });
    expect(filled).toContain('Publisher="CN=Foo, O=&quot;Acme, Inc.&quot;, L=O&apos;Fallon"');
    expect(filled).toContain(
      '<PublisherDisplayName>O&apos;Fallon &amp; Sons</PublisherDisplayName>'
    );
    // The raw forms must be gone, or the attribute above ended early.
    expect(filled).not.toContain('O="Acme');
  });

  it('refuses a manifest with a placeholder left over', () => {
    expect(() => fillManifest(template, { IDENTITY_NAME: 'a' })).toThrow(/unfilled placeholder/);
  });
});

describe('readIdentity', () => {
  it.each(['MSIX_IDENTITY_NAME', 'MSIX_PUBLISHER', 'MSIX_PUBLISHER_DISPLAY_NAME'])(
    'names %s when it is missing',
    (missing) => {
      const env = { ...IDENTITY, [missing]: '' };
      expect(() => readIdentity(env)).toThrow(new RegExp(`Missing ${missing}`));
    }
  );

  it('rejects a publisher that is not a full subject', () => {
    expect(() => readIdentity({ ...IDENTITY, MSIX_PUBLISHER: 'Example Publisher' })).toThrow(
      /must be the full subject/
    );
  });

  it('returns the trimmed values', () => {
    expect(readIdentity({ ...IDENTITY, MSIX_IDENTITY_NAME: '  Padded.Name  ' })).toEqual({
      IDENTITY_NAME: 'Padded.Name',
      PUBLISHER: IDENTITY.MSIX_PUBLISHER,
      PUBLISHER_DISPLAY_NAME: IDENTITY.MSIX_PUBLISHER_DISPLAY_NAME,
    });
  });
});

describe('compareSdkVersions', () => {
  // Sorting SDK directory names as STRINGS puts 10.0.9... above 10.0.26100.0,
  // so the packer would silently use an ancient SDK. Numeric, newest first.
  it('orders newest first, numerically', () => {
    const found = ['10.0.19041.0', '10.0.26100.0', '10.0.22621.0', '10.0.9600.0'];
    expect([...found].sort(compareSdkVersions)).toEqual([
      '10.0.26100.0',
      '10.0.22621.0',
      '10.0.19041.0',
      '10.0.9600.0',
    ]);
  });

  it('treats a missing segment as zero', () => {
    expect(compareSdkVersions('10.0.26100', '10.0.26100.0')).toBe(0);
    expect(compareSdkVersions('10.1', '10.0.99999.0')).toBeLessThan(0);
  });
});

describe('path privacy in printed output', () => {
  it('keeps in-repo paths relative and reduces outside ones to a basename', () => {
    expect(rel(path.join(HERE, 'pack-msix.mjs'))).toBe('apps/desktop/scripts/pack-msix.mjs');
    const outside = rel(path.join(os.tmpdir(), 'someone', 'secret-dir', 'makeappx.exe'));
    expect(outside).toBe('<outside repo>/makeappx.exe');
  });

  // makeappx echoes `\\?\`-prefixed absolute paths for every file it packs, and
  // a spawn failure carries the absolute command line — both would put a
  // username into a public CI log.
  it('scrubs every absolute path out of tool output', () => {
    const output = [
      'Processing "\\\\?\\C:\\Users\\somebody\\secrets\\staging\\ajh-tauri.exe" as a payload file.',
      'The path (/p) parameter is: "D:\\build\\out.msix"',
    ].join('\n');
    const scrubbed = scrubPaths(output);
    expect(scrubbed).not.toMatch(/[A-Za-z]:[\\/]/);
    expect(scrubbed).not.toContain('somebody');
    expect(scrubbed).toContain('<outside repo>/ajh-tauri.exe');
  });
});

describe('findMakeappx', () => {
  // No SDK on the machine (or a typo'd override) must be a named failure, not
  // a spawn error from execFileSync with a bare "ENOENT".
  it('reports a missing SDK instead of returning nothing', () => {
    expect(() => findMakeappx({})).toThrow(/makeappx\.exe not found/);
  });

  it('reports an override that points nowhere', () => {
    expect(() => findMakeappx({ MAKEAPPX: 'Z:/nope/makeappx.exe' })).toThrow(/missing file/);
  });
});

describe('stageMsix', () => {
  it('stages exactly the package payload, with a manifest that has no placeholders left', () => {
    const { staging, output } = stageMsix(packEnv);

    expect(fs.readdirSync(staging).sort()).toEqual(['AppxManifest.xml', 'Assets', 'ajh-tauri.exe']);
    // The exe is staged under the name the manifest's Executable attribute
    // uses, whatever the source file was called.
    expect(fs.readFileSync(path.join(staging, 'ajh-tauri.exe'), 'utf8')).toBe('MZ placeholder');
    expect(fs.readdirSync(path.join(staging, 'Assets')).sort()).toEqual([
      'Square150x150Logo.png',
      'Square44x44Logo.png',
      'StoreLogo.png',
    ]);

    const manifest = fs.readFileSync(path.join(staging, 'AppxManifest.xml'), 'utf8');
    expect(manifest).not.toContain('{{');
    expect(manifest).toContain(`Name="${IDENTITY.MSIX_IDENTITY_NAME}"`);
    expect(manifest).toContain(`Publisher="${IDENTITY.MSIX_PUBLISHER}"`);
    // Four-part version with the reserved zero revision, mirrored in the
    // output filename the Store submission is uploaded from.
    expect(manifest).toMatch(/Version="\d+\.\d+\.\d+\.0"/);
    expect(path.basename(output)).toMatch(/^AI-Job-Hunter_\d+\.\d+\.\d+\.0_x64\.msix$/);
  });

  it('rebuilds the staging directory instead of adding to it', () => {
    const { staging } = stageMsix(packEnv);
    const intruder = path.join(staging, 'leftover-from-a-previous-run.txt');
    fs.writeFileSync(intruder, 'do not ship me');

    stageMsix(packEnv);

    expect(fs.existsSync(intruder)).toBe(false);
  });
});

describe('packMsix', () => {
  // The Windows SDK is present on the Windows release runner and on a dev
  // machine with the packaging tools; elsewhere (Linux CI legs) there is
  // nothing to test, and `findMakeappx` already has its own coverage above.
  const sdk = (() => {
    try {
      return findMakeappx(packEnv);
    } catch {
      return null;
    }
  })();

  it.skipIf(!sdk)('produces a package makeappx accepts, at the expected path', () => {
    const output = packMsix(packEnv);

    expect(fs.existsSync(output)).toBe(true);
    expect(fs.statSync(output).size).toBeGreaterThan(0);
    expect(path.dirname(output)).toBe(scratch);
  });
});
