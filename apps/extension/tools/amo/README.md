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

Two steps, deliberately split so that **installing** this tree never happens with
a credential in scope:

1. `npm ci --ignore-scripts` here, in a step with **no secrets** in `env:`.
   `--ignore-scripts` is safe — the tree has no build step web-ext needs (a
   from-scratch `npm ci --ignore-scripts` run of the `sign` command was verified
   to parse its flags, validate the manifest and build the xpi).
2. `node_modules/.bin/web-ext sign …` in a step whose `env:` is the only place
   `WEB_EXT_API_KEY` / `WEB_EXT_API_SECRET` appear.

### What that does and does not buy

Be clear about the boundary: step 2 **executes this entire dependency tree with
the AMO credential in its environment**. Running web-ext at all means running its
~330 packages, and that key can publish a Mozilla-signed version of every add-on
on the account. The split does not remove that exposure — nothing can, short of a
first-party uploader, and there isn't one.

What it does buy is that the exposure is bounded and the code is not a moving
target:

- the key exists in exactly one step, never during checkout, install, the build,
  the source-archive step or the reproducibility gate;
- the tree is pinned by a committed `package-lock.json` with an integrity hash
  for every package, so it cannot change under us between releases;
- it is installed with lifecycle scripts disabled, so nothing runs at install
  time at all.

The residual supply-chain risk — a compromised release of web-ext or one of its
dependencies, pulled in by a future bump — is accepted, and is the reason the
pin is a lockfile rather than `npx`.

## Accepted advisories

`npm audit` here reports `image-size` (reached as web-ext → addons-linter →
image-size) for denial of service via infinite loops in its ICNS and JXL/HEIF
parsers, which propagates the rating up to `addons-linter` and `web-ext`. The
only remediation npm offers is downgrading web-ext across a major, off the AMO
submission API this tool exists to use — so it stays. It is also not reachable
in this usage: those parsers run in addons-linter's icon checks, which
`web-ext sign` never invokes, and the only images in scope would be this repo's
own committed icons. `.github/dependabot.yml` suppresses PRs for it on the same
grounds; revisit when addons-linter widens its range.

## Bumping

Dependabot has its own npm entry for this directory (`.github/dependabot.yml`).
By hand: edit the pin in `package.json`, run `npm install --package-lock-only`
here, and commit both files.
