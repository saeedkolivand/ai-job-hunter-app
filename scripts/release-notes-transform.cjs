#!/usr/bin/env node
/**
 * release-notes-transform.cjs — utility module for contributor credit extraction.
 *
 * Exports extractGitHubLogin for unit testing and reuse.
 * The actual transform wrapping logic is in release.config.mjs.
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
