# AMO submission tool (isolated)

`web-ext`, pinned by its own `package-lock.json`, used by exactly one thing: the
`publish-firefox` job in `.github/workflows/release.yml`.

## Why it is not a normal devDependency

`web-ext` is ~330 packages / ~78 MB (`addons-linter` plus a second, deprecated
copy of eslint). Putting it in `apps/extension/package.json` would add all of
that to **every** `pnpm install --frozen-lockfile` in the monorepo — every CI
job, every contributor clone, and the weekly audit surface — for a tool that
runs once per release.

## Why it is not `npx web-ext@<version>` either

`npx --yes web-ext@10.6.0` pins only the entry package: every transitive
dependency re-resolves at run time with no integrity hashes, in the job that
holds `WEB_EXT_API_SECRET` — a credential that can publish a Mozilla-signed
version of every add-on on the account. A committed `package-lock.json` pins all
329 transitive packages by version **and** integrity hash.

## Why it is not a pnpm workspace member

`pnpm-workspace.yaml` globs `apps/*` and `packages/*`, so this directory is
invisible to pnpm and its own lockfile is the only thing that resolves it. That
isolation is the point — the root install stays unaffected.

## How the release workflow uses it

Two steps, deliberately split so no credential is in scope while third-party code
is being fetched or executed:

1. `npm ci --ignore-scripts` here, in a step with **no secrets** in `env:`.
   `--ignore-scripts` is safe — the tree has no build step web-ext needs (a
   from-scratch `npm ci --ignore-scripts` run of the `sign` command was verified
   to parse its flags, validate the manifest and build the xpi).
2. `node_modules/.bin/web-ext sign …` in a step whose `env:` is the only place
   `WEB_EXT_API_KEY` / `WEB_EXT_API_SECRET` appear.

## Bumping

Dependabot has its own npm entry for this directory (`.github/dependabot.yml`).
By hand: edit the pin in `package.json`, run `npm install --package-lock-only`
here, and commit both files.
