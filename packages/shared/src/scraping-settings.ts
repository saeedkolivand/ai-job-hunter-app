/**
 * Non-secret scraping settings — the cross-language contract shared by the
 * renderer (writes them) and the Rust aggregator (reads them).
 *
 * These are NOT secrets, so unlike credential slots they do NOT live in the OS
 * keychain. They are persisted with `@tauri-apps/plugin-store` to the JSON file
 * named by {@link SCRAPING_SETTINGS_FILE}, which plugin-store resolves relative to
 * the app data dir — the SAME directory the Rust worker resolves via
 * `platform::config::data_dir()`. The Rust side reads the exact key strings below
 * (mirrored as literals in `scraping/boards/aggregator/mod.rs`, pinned by a unit
 * test there). Keep these values byte-stable: changing one is a settings-file
 * migration, not a rename.
 *
 * The renderer writes them like:
 *   const store = await Store.load(SCRAPING_SETTINGS_FILE);
 *   await store.set(SCRAPING_SETTINGS_KEYS.apifyLinkedinEnabled, true);
 *   await store.save();
 */
export const SCRAPING_SETTINGS_FILE = 'scraping-settings.json';

export const SCRAPING_SETTINGS_KEYS = {
  /** Master opt-in for the paid LinkedIn (Apify) provider. Default OFF. boolean. */
  apifyLinkedinEnabled: 'apifyLinkedinEnabled',
  /** Optional Apify actor-id override. Absent/blank → the built-in default actor. string. */
  apifyLinkedinActorId: 'apifyLinkedinActorId',
} as const;
