#!/usr/bin/env node
/**
 * guard-empty-release-notes — local semantic-release plugin (prepare hook).
 *
 * Lifecycle: semantic-release calls `prepare` AFTER
 * `@semantic-release/release-notes-generator` has populated
 * `context.nextRelease.notes`, and BEFORE any `publish` step. In `release.config.mjs`
 * this plugin is ordered first among the `prepare` hooks (immediately after
 * release-notes-generator, before @semantic-release/exec / changelog / git /
 * github), so it runs only when a real release is actually proceeding and — by
 * throwing — aborts the run BEFORE the version files are synced, the changelog is
 * written, the release commit is made, or anything is tagged/published.
 *
 * It is pure JS reading the in-memory `context` object: no shell, and no template
 * interpolation of the (untrusted, multi-line) notes, so there is no
 * command-injection surface.
 *
 * Guard rationale: conventional-changelog-conventionalcommits v10 has a regression
 * that silently empties the generated notes (no error thrown) — it shipped a blank
 * changelog in v0.119.0. Empty notes on a proceeding release almost always mean
 * that regression, so we fail loudly with the fix instead of publishing a blank
 * release.
 */

'use strict';

/**
 * semantic-release `prepare` lifecycle hook.
 *
 * @param {object} _pluginConfig  Unused — this guard takes no options.
 * @param {{ nextRelease?: { notes?: string, version?: string }, logger: { log: (msg: string) => void } }} context
 * @throws {Error} when the generated release notes are empty/whitespace-only.
 */
function prepare(_pluginConfig, context) {
  const notes = context && context.nextRelease && context.nextRelease.notes;

  if (!notes || notes.trim() === '') {
    throw new Error(
      [
        'Release notes are empty — aborting BEFORE the changelog is written, committed, or published.',
        '',
        'This almost always means the conventional-changelog-conventionalcommits v10 regression,',
        'which silently empties the generated notes with no error (it shipped a blank changelog in v0.119.0).',
        '',
        'Fix: pin conventional-changelog-conventionalcommits to v9 in package.json, reinstall, and re-run the release.',
      ].join('\n')
    );
  }

  context.logger.log(
    `Release notes present (${notes.trim().length} chars) — empty-notes guard passed.`
  );
}

module.exports = { prepare };
