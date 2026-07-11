/**
 * Centralized AI-provider credential SLOT NAMES — the single cross-language
 * source of truth shared by the renderer and the Rust aggregator.
 *
 * These are the BARE slot names. The `ai:` keyring namespace is a Rust-side
 * concern applied at read time (`read_credential(&format!("ai:{}", slot))`),
 * NOT part of the slot identity — keep these values free of any prefix.
 *
 * The Rust mirror (`apps/desktop/src-tauri/src/ipc_contracts/provider_slots.rs`)
 * is generated from this file by `pnpm gen:ipc`; `pnpm gen:ipc:check` guards the
 * two against drift. Changing a value here is a wire/keyring-slot change — keep
 * the strings byte-identical unless you are deliberately migrating a slot.
 */
export const PROVIDER_SLOTS = {
  adzunaAppId: 'adzuna-app-id',
  adzunaAppKey: 'adzuna-app-key',
  jsearchKey: 'jsearch-key',
  // Jooble API key — path-segment auth (`POST /api/{key}`) for the Jooble
  // aggregator fallback provider (fires after Adzuna + JSearch both come up
  // empty/erroring — see `aggregator/mod.rs: primary_chain`).
  joobleKey: 'jooble-key',
  // Apify API token — Bearer auth for the LinkedIn (Apify) aggregator provider.
  apifyToken: 'apify-token',
  // Comeet board credentials — company UID + API token for the
  // `careers-api/2.0/company/{uid}/positions?token={token}` endpoint.
  comeetCompanyUid: 'comeet-company-uid',
  comeetApiToken: 'comeet-api-token',
} as const;
