/**
 * Centralized AI-provider credential SLOT NAMES — the single cross-language
 * source of truth shared by the renderer and the Rust aggregator.
 *
 * These are the BARE slot names. The `ai:` keyring namespace is a Rust-side
 * concern applied at read time (`read_credential(&format!("ai:{}", slot))`),
 * NOT part of the slot identity — keep these values free of any prefix.
 *
 * The Rust mirror (`apps/tauri/src-tauri/src/ipc_contracts/provider_slots.rs`)
 * is generated from this file by `pnpm gen:ipc`; `pnpm gen:ipc:check` guards the
 * two against drift. Changing a value here is a wire/keyring-slot change — keep
 * the strings byte-identical unless you are deliberately migrating a slot.
 */
export const PROVIDER_SLOTS = {
  adzunaAppId: 'adzuna-app-id',
  adzunaAppKey: 'adzuna-app-key',
  jsearchKey: 'jsearch-key',
} as const;
