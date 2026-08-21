// One-shot: reconstruct a daily installer-downloads series from git history.
//
// WHY THIS EXISTS: GitHub's API only ever reports a release asset's download
// count RIGHT NOW — there is no historical endpoint. And the downloads badge
// workflow publishes to the `badges` branch as a PARENTLESS orphan commit that
// is force-pushed, so that branch has exactly one commit and preserves nothing.
// A downloads-over-time chart therefore had no data to draw, in either place.
//
// The one surviving record is `apps/landing/public/metrics/releases.json`, which
// the Mission Control snapshot workflow commits to main daily and which embeds
// every release's per-asset `download_count` at that moment. Walking its git
// history replays roughly a month of real readings.
//
// THE OFFSET, AND WHY IT IS NOT OPTIONAL: releases.json is page 1 of the API
// (30 releases), and the repo has 139. Only 51 distinct tags ever appeared in a
// snapshot, so a raw replay sums to 465 while the live badge reads 594. Charting
// the raw replay next to the live reading would draw a 129-unit cliff on the
// final day that never happened. So every release that appears in NO snapshot is
// counted once, at its current total, and added to every point as a constant
// floor — putting the reconstruction on the same basis as the live number.
//
// REMAINING LIMITS, stated rather than hidden:
//   1. Those never-seen releases did gain some downloads during the window, so a
//      constant floor slightly OVERSTATES the earliest points. Old releases grow
//      slowly and the error is far smaller than the cliff it replaces, but the
//      early part of this series is an estimate, not a reading.
//   2. Counts are carried forward per tag when a tag drops out of the 30-release
//      window. Sound, because a download count only grows — but a dropped-out
//      tag's later growth inside the window is missed.
//   3. Multiple snapshots can land on one date; the highest reading wins.
//   4. Everything before the first snapshot (2026-07-21) is simply unknowable.
//
// Run: GITHUB_TOKEN=$(gh auth token) node scripts/backfill-downloads-history.mjs \
//        > scripts/data/downloads-history-seed.json

import { execFileSync } from 'node:child_process';

import { fetchAllReleases, INSTALLER_RE, installerDownloads } from './lib/github-releases.mjs';

const METRICS = 'apps/landing/public/metrics/releases.json';

function git(args) {
  return execFileSync('git', args, { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
}

const log = git(['log', '--format=%H %ad', '--date=short', '--reverse', '--', METRICS])
  .trim()
  .split('\n')
  .filter(Boolean);

/** tag -> highest installer-download count seen so far (counts only grow). */
const seen = new Map();
/** date -> cumulative total on that date. */
const byDate = new Map();

for (const line of log) {
  const [sha, date] = line.split(' ');
  let releases;
  try {
    releases = JSON.parse(git(['show', `${sha}:${METRICS}`]));
  } catch {
    continue; // a snapshot that was empty or malformed at that commit
  }
  if (!Array.isArray(releases)) continue;

  for (const release of releases) {
    const tag = release?.tag_name;
    if (!tag) continue;
    let n = 0;
    for (const asset of release.assets ?? []) {
      if (INSTALLER_RE.test(asset.name)) n += asset.download_count ?? 0;
    }
    seen.set(tag, Math.max(seen.get(tag) ?? 0, n));
  }

  let total = 0;
  for (const n of seen.values()) total += n;
  byDate.set(date, Math.max(byDate.get(date) ?? 0, total));
}

// Releases that never appeared in ANY snapshot contribute a constant the replay
// can never see. Count them once, now, and lift the whole series by it.
const allReleases = await fetchAllReleases();
const unseen = allReleases.filter((r) => r?.tag_name && !seen.has(r.tag_name));
const offset = installerDownloads(unseen);

const points = [...byDate.entries()]
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([date, value]) => ({ date, value: value + offset }));

process.stderr.write(
  `backfilled ${points.length} daily points from ${log.length} snapshots\n` +
    `  tags seen in snapshots : ${seen.size} of ${allReleases.length} releases\n` +
    `  constant floor added   : ${offset} (installers on ${unseen.length} never-snapshotted releases)\n` +
    `  series                 : ${points[0]?.date} → ${points.at(-1)?.date}, ` +
    `${points[0]?.value} → ${points.at(-1)?.value} installers\n`
);
process.stdout.write(`${JSON.stringify(points, null, 2)}\n`);
