import { createRequire } from 'node:module';
import { describe, it, expect } from 'vitest';
import releaseNotesTransform from './release-notes-transform.cjs';

const require = createRequire(import.meta.url);

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

  describe('end-to-end: render release notes with wrapped transform', () => {
    it('renders notes with section grouping, contributor credits, and hidden filtering', async () => {
      // Import the actual release-notes-generator and config
      const { generateNotes } = await import('@semantic-release/release-notes-generator');
      const config = await import('../release.config.mjs');

      // Extract the release-notes-generator plugin options
      const releaseNotesPlugin = config.default.plugins.find(
        (p) => Array.isArray(p) && String(p[0]).includes('release-notes-generator')
      );
      expect(releaseNotesPlugin).toBeDefined();
      const pluginOpts = releaseNotesPlugin[1];

      // Create synthetic commits covering three cases:
      // 1. Fix from external contributor (thejesh23)
      // 2. Feature from owner (saeedkolivand)
      // 3. Chore (should be filtered out)
      const commits = [
        {
          hash: 'abc1234567890',
          hashShort: 'abc1234',
          message: 'fix(agent): resolve agent_run provider from backend store\n\nCloses #679',
          author: {
            name: 'thejesh23',
            email: '35212698+thejesh23@users.noreply.github.com',
          },
          committerDate: new Date('2026-07-16T12:00:00Z').toISOString(),
        },
        {
          hash: 'def5678901234',
          hashShort: 'def5678',
          message: 'feat: add contributor credit system\n\nFixes #680',
          author: {
            name: 'saeedkolivand',
            email: '123456789+saeedkolivand@users.noreply.github.com',
          },
          committerDate: new Date('2026-07-17T12:00:00Z').toISOString(),
        },
        {
          hash: 'ghi9012345678',
          hashShort: 'ghi9012',
          message: 'chore: update dependencies',
          author: {
            name: 'saeedkolivand',
            email: '123456789+saeedkolivand@users.noreply.github.com',
          },
          committerDate: new Date('2026-07-17T13:00:00Z').toISOString(),
        },
      ];

      // Minimal context matching semantic-release shape
      const context = {
        cwd: process.cwd(),
        env: process.env,
        options: {
          repositoryUrl: 'https://github.com/saeedkolivand/ai-job-hunter-assistant-app',
        },
        lastRelease: { gitTag: 'v9.9.8' },
        nextRelease: { gitTag: 'v9.9.9', version: '9.9.9', type: 'minor' },
        commits,
        logger: {
          log() {},
          error() {},
        },
      };

      // Call generateNotes with our config's wrapped transform
      const notes = await generateNotes(pluginOpts, context);

      // ========== Assertions ==========

      // (1) Non-empty release notes
      expect(notes).toBeTruthy();
      expect(notes.length).toBeGreaterThan(0);

      // (2) Section grouping is preserved (look for section headers like ### 🐛 Bug Fixes)
      const hasSectionHeader = /^### /m.test(notes);
      expect(hasSectionHeader).toBe(true);

      // (3) External contributor (thejesh23) gets exactly one (@thejesh23) credit
      const thejeshMatches = notes.match(/@thejesh23/g);
      expect(thejeshMatches).not.toBeNull();
      expect(thejeshMatches.length).toBe(1);

      // (4) Owner commits do NOT get @mention suffix
      const ownerLines = notes
        .split('\n')
        .filter((line) => line.includes('add contributor credit'));
      expect(ownerLines.length).toBeGreaterThan(0);
      const ownerLine = ownerLines[0];
      expect(ownerLine).not.toMatch(/@saeedkolivand/);

      // (5) Hidden commits (chore) are filtered out entirely
      const hasChore = /^- chore:/m.test(notes) || notes.includes('update dependencies');
      expect(hasChore).toBe(false);

      // (6) References are linkified (look for commit/ or issues/ links in GitHub URLs)
      const hasRef =
        notes.includes('https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/') ||
        notes.includes('https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/');
      expect(hasRef).toBe(true);
    });
  });

  describe('integration: config structure and guard script', () => {
    it('release.config.mjs loads successfully with ESM and top-level await', async () => {
      const config = await import('../release.config.mjs');
      expect(config).toBeDefined();
      expect(config.default).toBeDefined();
    });

    it('release.config.mjs has wrapped transform in release-notes-generator', async () => {
      const config = await import('../release.config.mjs');
      const releaseNotesPlugin = config.default.plugins[1];

      expect(releaseNotesPlugin[0]).toBe('@semantic-release/release-notes-generator');
      expect(releaseNotesPlugin[1].writerOpts).toBeDefined();
      expect(typeof releaseNotesPlugin[1].writerOpts.transform).toBe('function');
    });

    it('guard-empty-release-notes.cjs validates notes correctly', () => {
      const guard = require('./guard-empty-release-notes.cjs');
      const logger = { log() {} };

      expect(() => {
        guard.prepare({}, { nextRelease: { notes: '## Release\n\nNotes' }, logger });
      }).not.toThrow();

      expect(() => {
        guard.prepare({}, { nextRelease: { notes: '' }, logger });
      }).toThrow(/empty/i);
    });
  });
});
