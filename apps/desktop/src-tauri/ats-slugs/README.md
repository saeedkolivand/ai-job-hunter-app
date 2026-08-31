# Vendored ATS company-slug directory

Offline company-slug lists behind the discovery typeahead's "community slug
directory" feeder (ADR-030 §b names this feeder explicitly: `source` is
free-text "so future feeders \[...\] community slug directories require no
schema changes"). Compiled into the binary via `include_bytes!` in
`src/discovered/vendored.rs` — **nothing here is fetched at build time or at
runtime**; a query the index can't answer just returns no vendored rows.

| File                | Slugs  | ATS        |
| ------------------- | ------ | ---------- |
| `bamboohr.txt.gz`   | 11,314 | BambooHR   |
| `greenhouse.txt.gz` | 8,324  | Greenhouse |
| `lever.txt.gz`      | 4,365  | Lever      |
| `ashby.txt.gz`      | 3,161  | Ashby      |

Each file is a gzip-compressed, newline-delimited, sorted, deduplicated list of
bare slugs (one per line, no JSON/quoting overhead — cuts the raw ~300 KB of
slugs to ~129 KB gzipped, in the same spirit as `geodata/cities.tsv.gz`).
Empty and purely-numeric entries (crawler junk — a numeric ID is never a real
company slug) were dropped from the upstream source during import.

**Scope — four platforms only.** The upstream source also ships `icims`,
`paylocity`, and `workday` slug lists; those are deliberately **not** vendored
here because `scraping/ats_ref.rs::extract_ats_ref` has no parser for any of
the three (no `SCRAPERS` registry entry either) — seeding a slug our extractor
can't resolve would advertise a company the app cannot scrape.

**Ashby casing.** ADR-030 flags Ashby's slug casing as strict (board tokens
are case-sensitive, e.g. `Linear`, `Perplexity`). The upstream Ashby list is
entirely lowercase, but the `api.ashbyhq.com/posting-api/job-board/{slug}`
endpoint (the exact one `scraping/boards/ashby` calls) was live-verified
case-insensitive for several lowercase slugs before import (2026-08-31) — a
lowercase vendored slug still resolves.

## Provenance

- **Upstream:** [`Feashliaa/job-board-aggregator`](https://github.com/Feashliaa/job-board-aggregator), `data/{bamboohr,greenhouse,lever,ashby}_companies.json`
- **Upstream commit:** `644fb02f9630d55ba7210bd0d4e03ef7881f0ff8`
- **Snapshot date:** 2026-08-31

| Asset               | SHA-256 (gzip)                                                     |
| ------------------- | ------------------------------------------------------------------ |
| `bamboohr.txt.gz`   | `0a36b2c423767ce4771679e0e08f72de33021a09ac60cd251f1deb16bdd19644` |
| `greenhouse.txt.gz` | `7190fb24209f8c4c7083ae445ccd5c89132a97d71524a7f1c251e1a34c1fdb9c` |
| `lever.txt.gz`      | `c08c0860188d6e7942ce3311ab9c15d1e945747ca6c52e2e5628ac7bfdd50854` |
| `ashby.txt.gz`      | `2ab565a3f37b4689f23d5e9389cd1e1a67f37b438713902c67e5fda776e21c87` |

The same digests are recorded in `src/discovered/vendored.rs` and asserted by
`embedded_assets_match_their_recorded_digests`, so a corrupted checkout or a
hand-edited asset fails the test suite instead of silently shipping data of
unknown origin.

## License / attribution — required in distributed builds

Upstream's `LICENSE` (repo root) is MIT, but the upstream README carves the
`data/` datasets out under a **separate, stricter license**:

> The curated company datasets in `data/` are licensed under CC BY-NC 4.0
> (<https://creativecommons.org/licenses/by-nc/4.0/>). \[...\] Commercial use
> of the datasets requires permission.

This app is itself licensed PolyForm Noncommercial 1.0.0 (see the repo-root
`LICENSE`), so vendoring CC BY-NC 4.0 data here carries no license conflict —
but it does mean the MIT grant does **not** cover these four files; treat them
as CC BY-NC 4.0, attributed to Riley Dorrington
(<https://github.com/Feashliaa/job-board-aggregator>), and re-check this
section before any future change to this app's own license.

Attribution lives in:

- the header comment of `src/discovered/vendored.rs`;
- `docs/SCRAPING_ENDPOINTS.md` (the "Vendored company-slug seed data" note);
- this file.

## Refreshing

No refresh script exists (unlike `pnpm gen:geonames`) — this is a one-time
vendored snapshot, not a live-tracked upstream feed. To refresh: re-clone
upstream at a later commit, re-run the same filter (drop empty + `^[0-9]+$`
slugs, dedupe, sort) over the four `*_companies.json` files, gzip, replace the
four files here, and update the provenance table + digests above and in
`src/discovered/vendored.rs`.
