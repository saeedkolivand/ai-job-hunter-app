#!/usr/bin/env node
/**
 * release-notes-transform.cjs — single source of truth for GitHub login extraction.
 *
 * Exports extractGitHubLogin, imported by release.config.mjs (the wrapped
 * writerOpts.transform) and by release-notes-transform.test.mjs (unit tests),
 * so both stay in sync — no duplicated regex to drift.
 */

'use strict';

/**
 * Extract GitHub login from noreply email.
 * Pattern: <id>+<login>@users.noreply.github.com
 *
 * @param {string} email  Author email
 * @returns {string|null}  GitHub login if pattern matches, null otherwise
 */
function extractGitHubLogin(email) {
  if (!email) return null;
  const match = email.match(/^\d+\+([^@]+)@users\.noreply\.github\.com$/);
  return match ? match[1] : null;
}

module.exports = { extractGitHubLogin };
