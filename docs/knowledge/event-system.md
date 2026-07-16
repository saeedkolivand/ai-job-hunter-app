# Event System (Tauri Push Events)

Centralized, one-way event channels — the complement to IPC request/response (`IPC_CHANNELS`). Tauri `app.emit()` broadcasts events to all windows using colon-namespaced wire names (e.g. `ai:stream`, `menu:navigate`).

## Single source of truth

**Registry definition:** `packages/shared/src/events/index.ts`

- `EVENT_CHANNELS` — namespace → { key → wire-name } map; combined view of all 11 namespaces (agent, ai, applications, autopilot, boards, jobs, menu, notifications, scrape, system, updater)
- `AppEvents` — union type keyed by wire name → payload type; kept in 1:1 sync by the lock test `events.test.ts`
- **Lock test:** `packages/shared/src/events/events.test.ts` — asserts uniqueness, wire-name collision-free, and `AppEvents ↔ EVENT_CHANNELS` sync; run on every build

## Codegen

**Rust event-channel constants:** `packages/shared/scripts/gen-ipc-rust.ts` → `apps/desktop/src-tauri/src/ipc_contracts/events.rs`

- Emits screaming-snake const names (e.g. `pub const MENU_NAVIGATE: &str = "menu:navigate"`)
- Wire namespace derived from colon prefix; key from registry key
- Regenerate: `pnpm gen:ipc` (enforced in CI: `pnpm gen:ipc:check`)

**Rust credential-slot constants:** same codegen script → `apps/desktop/src-tauri/src/ipc_contracts/provider_slots.rs` (source: `packages/shared/src/provider-slots.ts`)

- Emits screaming-snake const names (e.g. `ADZUNA_APP_ID`) for the AI-provider keyring slots defined in `PROVIDER_SLOTS`
- Single cross-language source: `PROVIDER_SLOTS` object in the shared package
- Regenerate alongside events: `pnpm gen:ipc`

**Rust date-filter constants:** same codegen script → `apps/desktop/src-tauri/src/ipc_contracts/date_filters.rs` (source: `packages/shared/src/schemas/index.ts`)

- Emits a const array `pub const DATE_FILTER_OPTIONS: &[&str]` for the scraper's date-range tokens defined in `DATE_FILTER_OPTIONS`
- The aggregator Rust match arms for `adzuna_max_days_old` and `jsearch_date_posted` map each token to a provider-specific value; an exhaustiveness test fails if a new TS token isn't handled
- Regenerate alongside events: `pnpm gen:ipc`

## Emission layer

**L3 emit helper:** `apps/desktop/src-tauri/src/events/mod.rs`

- `emit_event(app: &AppHandle, channel: &str, payload: impl Serialize + Clone)` — the one place app events are emitted
- Generalizes old per-domain helpers (`emit_stream_error`, `emit_changed`, `dispatch_*`)
- Used throughout: autopilot, AI providers, commands, extension bridge, tray, notifications

**Convention:** Use `dispatch_menu` for local window broadcasts in `tray/mod.rs` (identity-free tray/window distinction); all other paths use centralized `emit_event`.

## Cold-start deep-link buffering

When a deep link (e.g. `ajh://autopilot/<id>`) or menu action fires during Tauri setup, before the renderer's JS event listeners attach, the emitted event is lost. Mitigate with a **buffer + pull pattern**:

1. Shell-side: `app.manage(Buffer<T>)` at the earliest point in setup (before any write path runs)
2. Write path: write intent to buffer BEFORE emit
3. Renderer: on mount (and on window focus/visibility-restore), call `takePendingIntent()` to atomically take and clear

Examples: `menu.takePending()` (menu + action intents), `autopilot.takePendingFocus()` (focus target id). Both use the same pattern.

**Critical ordering:** The buffer must be app-managed at the point the EARLIEST write can occur, which may be well before the buffer is consumed. Placing `app.manage()` inside a late builder (e.g. `tray::build`) will silently no-op any write that fires before the builder runs.

## Renderer consumption

**Tauri-client namespaces:** `apps/desktop/src/tauri-client/namespaces/**`

- Each namespace wraps `Tauri.listen()` for its event channels
- Types imported from `@ajh/shared` (via barrel `@ajh/shared/src/index.ts` → `EVENT_CHANNELS`, `AppEvents`)
- Example: `menu.ts` imports `MenuNavigateEvent`, `MenuActionEvent`, `PendingMenuIntent` from contracts, calls `listen<T>(EVENT_CHANNELS.menu.navigate, handler)`

## Completed & Planned phases

- **Phase 4:** Typed payload structs in Rust (shipped — `apps/desktop/src-tauri/src/ipc_contracts/event_payloads.rs`, codegen'd from `packages/shared/src/schemas/index.ts`; tray dispatch_menu still passes `serde_json::Value`)
- **Phase 6:** Jobs namespace collapse (merge `jobs:event` with autopilot stream)

## References

- Architecture: `docs/ARCHITECTURE.md` (IPC section)
- Patterns: `docs/PATTERNS.md` (messaging, event-driven)
