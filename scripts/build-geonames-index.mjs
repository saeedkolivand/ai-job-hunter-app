#!/usr/bin/env node
/**
 * Regenerate the bundled offline geocoding index from the GeoNames dumps.
 *
 *   pnpm gen:geonames
 *
 * Downloads the two upstream dumps, strips them to the handful of columns the
 * location autocomplete actually needs, and writes the two checked-in assets
 * consumed by `apps/desktop/src-tauri/src/commands/geocoding/geonames.rs`:
 *
 *   apps/desktop/src-tauri/geodata/cities.tsv.gz   (gzip TSV, ~25k rows)
 *   apps/desktop/src-tauri/geodata/countries.tsv   (plain TSV, ~250 rows)
 *
 * Network is used HERE (regeneration, run by hand) and never at cargo build
 * time — the app compiles the checked-in assets in with `include_bytes!`.
 *
 * Sources (public dumps, refreshed daily upstream):
 *   https://download.geonames.org/export/dump/cities15000.zip
 *   https://download.geonames.org/export/dump/countryInfo.txt
 *
 * License: GeoNames data is CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
 * Attribution ships in the app (Settings → About) and in
 * apps/desktop/src-tauri/geodata/README.md.
 */
import { gzipSync } from 'node:zlib';
import { inflateRawSync } from 'node:zlib';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const CITIES_URL = 'https://download.geonames.org/export/dump/cities15000.zip';
const COUNTRIES_URL = 'https://download.geonames.org/export/dump/countryInfo.txt';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'apps',
  'desktop',
  'src-tauri',
  'geodata'
);

/** Longest alternate name kept; longer entries are descriptive junk, not input. */
const MAX_ALTERNATE_LEN = 40;

async function download(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`GET ${url} -> ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

/**
 * Extract the single member of a GeoNames .zip without a zip dependency.
 * Reads the end-of-central-directory record, then the first central-directory
 * entry, then inflates the raw deflate stream that follows its local header.
 */
function unzipSingleMember(buf) {
  const eocd = buf.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  if (eocd < 0) throw new Error('not a zip archive (no EOCD record)');
  const centralOffset = buf.readUInt32LE(eocd + 16);
  if (buf.readUInt32LE(centralOffset) !== 0x02014b50) throw new Error('bad central directory');
  const method = buf.readUInt16LE(centralOffset + 10);
  const compressedSize = buf.readUInt32LE(centralOffset + 20);
  const localOffset = buf.readUInt32LE(centralOffset + 42);
  if (buf.readUInt32LE(localOffset) !== 0x04034b50) throw new Error('bad local header');
  const nameLen = buf.readUInt16LE(localOffset + 26);
  const extraLen = buf.readUInt16LE(localOffset + 28);
  const start = localOffset + 30 + nameLen + extraLen;
  const data = buf.subarray(start, start + compressedSize);
  if (method === 0) return data;
  if (method !== 8) throw new Error(`unsupported zip compression method ${method}`);
  return inflateRawSync(data);
}

/** Latin-script-ish: ASCII plus Latin-1 Supplement / Latin Extended-A. Drops
 * the Cyrillic/Arabic/CJK/… aliases nobody types into a Latin-script location
 * field, which is what keeps the asset at ~1.4 MB instead of ~4 MB. Everything
 * that survives the filter is kept — an arbitrary per-city cap silently loses
 * the endonym users actually type ("München" sits far down GeoNames' list). */
const LATIN = /^[ -~À-ſ'’\-. ]+$/;

function alternates(raw, name, asciiname) {
  const seen = new Set([name.toLowerCase(), asciiname.toLowerCase()]);
  const out = [];
  for (const candidate of raw.split(',')) {
    const alt = candidate.trim();
    const key = alt.toLowerCase();
    if (!alt || alt.length > MAX_ALTERNATE_LEN || alt.includes('|')) continue;
    if (seen.has(key) || !LATIN.test(alt)) continue;
    seen.add(key);
    out.push(alt);
  }
  return out;
}

function buildCities(text) {
  const rows = [];
  for (const line of text.split('\n')) {
    if (!line) continue;
    // cities15000 columns (tab-separated, see the `readme.txt` in the dump):
    // 1 name · 2 asciiname · 3 alternatenames · 4 lat · 5 lon · 8 country code · 14 population
    const c = line.split('\t');
    const [name, asciiname, alt, lat, lon] = [c[1], c[2], c[3] ?? '', c[4], c[5]];
    const cc = (c[8] ?? '').toUpperCase();
    const population = Number.parseInt(c[14] ?? '0', 10) || 0;
    if (!name || !cc || !lat || !lon) continue;
    rows.push(
      [
        name,
        asciiname === name ? '' : asciiname,
        cc,
        Number.parseFloat(lat).toFixed(4),
        Number.parseFloat(lon).toFixed(4),
        String(population),
        alternates(alt, name, asciiname ?? '').join('|'),
      ].join('\t')
    );
  }
  // Descending population: the runtime index keeps file order as its tie-break,
  // so the most-likely-meant city already wins before any scoring runs.
  rows.sort((a, b) => Number(b.split('\t')[5]) - Number(a.split('\t')[5]));
  return rows;
}

function buildCountries(text) {
  const rows = [];
  for (const line of text.split('\n')) {
    if (!line || line.startsWith('#')) continue;
    // countryInfo columns: 0 ISO · 1 ISO3 · 4 Country name (English) · 7 Population.
    // ISO3 rides along so the runtime can resolve a typed country CODE
    // ("usa", "deu") exactly, instead of letting it prefix-match an unrelated
    // city ("usa" → Uşak, TR). Population is what lets a country and a city
    // compete in the same ranking tier ("ber" → Berlin before Bermuda) instead
    // of countries always winning.
    const c = line.split('\t');
    const cc = (c[0] ?? '').trim().toUpperCase();
    const iso3 = (c[1] ?? '').trim().toUpperCase();
    const name = (c[4] ?? '').trim();
    const population = Number.parseInt(c[7] ?? '0', 10) || 0;
    if (cc.length !== 2 || !name) continue;
    rows.push(`${cc}\t${iso3}\t${name}\t${population}`);
  }
  rows.sort();
  return rows;
}

const [citiesZip, countriesTxt] = await Promise.all([
  download(CITIES_URL),
  download(COUNTRIES_URL),
]);

const cities = buildCities(unzipSingleMember(citiesZip).toString('utf8'));
const countries = buildCountries(countriesTxt.toString('utf8'));
if (cities.length < 10_000)
  throw new Error(`only ${cities.length} cities parsed — upstream format changed?`);
if (countries.length < 200)
  throw new Error(`only ${countries.length} countries parsed — upstream format changed?`);

mkdirSync(outDir, { recursive: true });
// `level: 9` + a fixed mtime keep the checked-in blob byte-stable across
// regenerations that produce identical data (no gratuitous git churn).
const gz = gzipSync(Buffer.from(`${cities.join('\n')}\n`, 'utf8'), { level: 9, mtime: 0 });
writeFileSync(join(outDir, 'cities.tsv.gz'), gz);
writeFileSync(join(outDir, 'countries.tsv'), `${countries.join('\n')}\n`);

console.log(`cities.tsv.gz  ${cities.length} rows, ${(gz.length / 1024).toFixed(0)} KB gzipped`);
console.log(`countries.tsv  ${countries.length} rows`);
