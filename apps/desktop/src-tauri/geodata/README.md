# Bundled location data (GeoNames)

Offline index behind the location autocomplete. Compiled into the binary via
`include_bytes!` / `include_str!` in
`src/commands/geocoding/geonames.rs` — **nothing here is fetched at build time**,
and no network call happens for a query the index can answer.

| File            | Rows | Source                     | Columns                                                       |
| --------------- | ---- | -------------------------- | ------------------------------------------------------------- |
| `cities.tsv.gz` | ~34k | GeoNames `cities15000`     | `name · asciiname · cc · lat · lon · population · alternates` |
| `countries.tsv` | ~250 | GeoNames `countryInfo.txt` | `cc · ISO3 · English name · population`                       |

`cities15000` is every city with a population over 15 000 plus every capital.
`asciiname` is stored empty when it equals `name`; `alternates` is a `|`-joined
list of the Latin-script alternate spellings (`München`, `Praha`, `Wien`) — the
non-Latin aliases are dropped, which is what keeps the gzipped asset at ~1.6 MB
instead of ~4 MB.

ISO3 rides along so a typed country **code** (`usa`, `deu`) resolves exactly
instead of prefix-matching an unrelated city (`usa` used to return Uşak, TR).
Codes are matched exact-only. The small curated endonym table (`deutschland`,
`schweiz`, …) deliberately lives in `src/commands/geocoding/geonames.rs`, not in
the asset, so regenerating from upstream never wipes it.

**Runtime cost:** the index is built once (warmed from `lib.rs`'s setup on a
blocking thread, so it never lands on a command worker) and then retains
**~10–12 MB** for the life of the process (folded search keys + display names);
~60–78 ms to build and ~7–11 ms per query on a release build.

## Provenance

| Asset           | Fetched    | SHA-256                                                            |
| --------------- | ---------- | ------------------------------------------------------------------ |
| `cities.tsv.gz` | 2026-07-27 | `fc0b8a097f36afe3816afc53d948360e5d4c4b70bc9c7a902020401ffd4c5df6` |
| `countries.tsv` | 2026-07-27 | `a630d7efae6f267ff42c2abc4f02970f5381f0cbeef8b9275b6286d1a0c5488d` |

The same digests are recorded in `src/commands/geocoding/geonames.rs` and
asserted by `embedded_assets_match_their_recorded_digests`, so a regeneration
that skips this record — or a corrupted checkout — fails the test suite instead
of shipping data of unknown origin. `pnpm gen:geonames` prints both digests and
the fetch date for you to paste.

## License / attribution — required in distributed builds

GeoNames data is licensed **CC BY 4.0**
(<https://creativecommons.org/licenses/by/4.0/>), verified against
<https://download.geonames.org/export/dump/readme.txt>. Attribution is therefore
**mandatory** in anything we ship, not optional. It lives in:

- the app itself — Settings → About (`settings.about.dataAttribution`, en + de);
- `docs/adr/0005-network-egress-privacy-boundary.md` (egress class 5);
- this file.

The online fallback (Photon, <https://photon.komoot.io>) serves OpenStreetMap
data under **ODbL**; the same attribution line credits it.

## Refreshing

```bash
pnpm gen:geonames
```

Downloads both upstream dumps, re-strips them, and rewrites the two files above
(`scripts/build-geonames-index.mjs`). Then re-run the crate's tests — the
geocoding suite asserts against the real asset, so a corrupt download or an
upstream column change fails loudly:

```bash
cd apps/desktop/src-tauri && cargo test --lib commands::geocoding
```

Upstream refreshes daily; the checked-in copy is a snapshot. A yearly refresh is
plenty — city populations and country names move slowly, and a stale row only
costs a few thousand residents of precision, never a wrong country code.
