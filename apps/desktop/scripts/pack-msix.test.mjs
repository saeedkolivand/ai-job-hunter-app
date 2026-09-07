import { describe, expect, it } from 'vitest';

import { fillManifest, findMakeappx, readIdentity, toStoreVersion } from './pack-msix.mjs';

const IDENTITY = {
  MSIX_IDENTITY_NAME: '12345Publisher.AIJobHunter',
  MSIX_PUBLISHER: 'CN=ABCDEF01-2345-6789-ABCD-EF0123456789',
  MSIX_PUBLISHER_DISPLAY_NAME: 'Example Publisher',
};

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
