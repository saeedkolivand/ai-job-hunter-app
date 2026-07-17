import { describe, it, expect } from 'vitest';
import releaseNotesTransform from './release-notes-transform.cjs';

describe('release-notes-transform', () => {
  describe('extractGitHubLogin (unit)', () => {
    const { extractGitHubLogin } = releaseNotesTransform;

    it('extracts login from GitHub noreply email', () => {
      const email = '35212698+thejesh23@users.noreply.github.com';
      expect(extractGitHubLogin(email)).toBe('thejesh23');
    });

    it('returns null for non-GitHub email', () => {
      const email = 'user@example.com';
      expect(extractGitHubLogin(email)).toBeNull();
    });

    it('returns null for missing email', () => {
      expect(extractGitHubLogin(null)).toBeNull();
      expect(extractGitHubLogin(undefined)).toBeNull();
      expect(extractGitHubLogin('')).toBeNull();
    });

    it('returns null for malformed GitHub email (missing +)', () => {
      const email = '35212698-thejesh23@users.noreply.github.com';
      expect(extractGitHubLogin(email)).toBeNull();
    });

    it('extracts login even if numeric id is very long', () => {
      const email = '999999999999999999+somelogin@users.noreply.github.com';
      expect(extractGitHubLogin(email)).toBe('somelogin');
    });

    it('recognizes bot accounts', () => {
      const email = '123456+dependabot[bot]@users.noreply.github.com';
      expect(extractGitHubLogin(email)).toBe('dependabot[bot]');
    });
  });

  describe('integration: config structure and guard script', () => {
    it('release.config.mjs loads successfully with ESM and top-level await', async () => {
      // Verify the config loads without errors (if load fails, test fails)
      const config = await import('../release.config.mjs');
      expect(config).toBeDefined();
      expect(config.default).toBeDefined();
    });

    it('release.config.mjs has wrapped transform in release-notes-generator', async () => {
      // Verify the release.config.mjs export includes the wrapped transform
      const config = await import('../release.config.mjs');
      const releaseNotesPlugin = config.default.plugins[1];

      // Plugin is [@semantic-release/release-notes-generator, options]
      expect(releaseNotesPlugin[0]).toBe('@semantic-release/release-notes-generator');
      expect(releaseNotesPlugin[1]).toBeDefined();

      // Options must have writerOpts with transform function (proof of wrapping)
      expect(releaseNotesPlugin[1].writerOpts).toBeDefined();
      expect(typeof releaseNotesPlugin[1].writerOpts.transform).toBe('function');
    });

    it('guard-empty-release-notes.cjs validates notes correctly', () => {
      // The guard script must still work with the new config
      const guard = require('./guard-empty-release-notes.cjs');
      const logger = { log() {} };

      // Non-empty notes should pass
      expect(() => {
        guard.prepare({}, { nextRelease: { notes: '## Release\n\nNotes' }, logger });
      }).not.toThrow();

      // Empty notes should fail
      expect(() => {
        guard.prepare({}, { nextRelease: { notes: '' }, logger });
      }).toThrow(/empty/i);
    });
  });
});
