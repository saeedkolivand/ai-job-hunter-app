// Builds every self-hosted repo graphic: the downloads badge, the downloads
// chart, and the star-history chart.
//
// WHY SELF-HOSTED: on 2026-06-30 GitHub restricted the stargazers API to a
// repo's own admins and collaborators, which broke every third-party star-history
// embed — star-history.com's chart now renders "GitHub restricted access to star
// data" instead of a graph. Running in-repo under GITHUB_TOKEN puts us on the
// allowed side of that restriction, and removes a live third-party service from
// the README's critical path.
//
// THE TWO SERIES ARE NOT SYMMETRIC, and that drives the whole design:
//   - STARS are fully re-derivable every run. `starred_at` on each stargazer is
//     the complete history, so the chart is rebuilt from scratch each time and
//     no state has to be persisted or trusted.
//   - DOWNLOADS are not. GitHub reports only a CURRENT count per asset, with no
//     historical endpoint, so a time series exists only if something records a
//     reading per day. That is what `downloads-history.json` is for. The past is
//     seeded once from git history (scripts/backfill-downloads-history.mjs) and
//     appended to daily from here.
//
// Output: badge-out/{downloads.json,downloads-by-platform.json,
//                    downloads-history.json,downloads.svg,stars.svg}
// Run locally: GITHUB_TOKEN=$(gh auth token) node scripts/build-repo-charts.mjs

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { mergePoints, readJsonArray } from './lib/downloads-history.mjs';
import {
  downloadsByPlatform,
  fetchAllReleases,
  fetchStargazers,
  installerDownloads,
} from './lib/github-releases.mjs';
import { GOLD, RED, renderLineChart } from './lib/line-chart-svg.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '..', 'badge-out');
const seedPath = join(here, 'data', 'downloads-history-seed.json');

/** Today in UTC — the runner's local zone must not decide which day a reading belongs to. */
const today = new Date().toISOString().slice(0, 10);

/** Shields does NOT humanize an endpoint `message`, so do it here. */
function humanize(n) {
  return n < 1000 ? String(n) : `${(n / 1000).toFixed(1)}k`;
}

/** Cumulative star count per day, derived wholly from `starred_at`. */
function starSeries(stargazers) {
  const perDay = new Map();
  for (const s of stargazers) {
    const at = s?.starred_at;
    if (!at) continue;
    const day = at.slice(0, 10);
    perDay.set(day, (perDay.get(day) ?? 0) + 1);
  }
  let running = 0;
  return [...perDay.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([date, n]) => {
      running += n;
      return { date, value: running };
    });
}

// ── Downloads ────────────────────────────────────────────────────────────────

const releases = await fetchAllReleases();
const total = installerDownloads(releases);

// `--prev-history` is the copy the workflow fetched off the `badges` branch.
// Absent on a first run, and absent locally — both degrade to seed-only.
const prevArg = process.argv.indexOf('--prev-history');
const prevHistory =
  prevArg === -1 ? [] : readJsonArray(process.argv[prevArg + 1], 'prior downloads history');

const history = mergePoints(readJsonArray(seedPath, 'downloads history seed'), prevHistory, [
  { date: today, value: total },
]);

mkdirSync(outDir, { recursive: true });

writeFileSync(
  join(outDir, 'downloads.json'),
  `${JSON.stringify({ schemaVersion: 1, label: 'downloads', message: humanize(total), color: RED.slice(1) }, null, 2)}\n`
);
writeFileSync(join(outDir, 'downloads-history.json'), `${JSON.stringify(history, null, 2)}\n`);

// Per-platform split for the /download buttons. Published here rather than
// fetched by the page: the honest number is cumulative across every release, so
// a client would need the whole paginated release list to compute it — against
// a 60/hour budget shared by every visitor. This costs one static same-origin
// file that pages.yml copies into public/ alongside the charts.
writeFileSync(
  join(outDir, 'downloads-by-platform.json'),
  `${JSON.stringify(downloadsByPlatform(releases), null, 2)}\n`
);

function writeChart(name, opts) {
  writeFileSync(join(outDir, `${name}.svg`), renderLineChart(opts));
}

writeChart('downloads', {
  points: history,
  title: 'Installer downloads',
  accent: RED,
  subtitle: 'real installs only — updater and extension traffic excluded',
  noun: 'installs',
});

// ── Stars ────────────────────────────────────────────────────────────────────

const stargazers = await fetchStargazers();
const stars = starSeries(stargazers);

if (stars.length) {
  writeChart('stars', {
    points: stars,
    title: 'Stars',
    accent: GOLD,
    subtitle: 'built in-repo from the GitHub API — no third-party service',
    noun: 'stars',
  });
} else {
  // No stars yet, or every `starred_at` was null. Writing a chart of nothing
  // would put an empty box in the README; leaving the previous one in place is
  // the better failure.
  process.stderr.write('no dated stargazers — leaving any existing stars.svg untouched\n');
}

process.stderr.write(
  `downloads: ${total} installers, ${history.length} daily points (${history[0]?.date} → ${history.at(-1)?.date})\n` +
    `stars: ${stargazers.length} stargazers, ${stars.length} dated points\n`
);
