# ADR-016: Centralized Notification Center

Last updated: 2026-07-16

**Status:** Accepted

## Context

The app has multiple asynchronous workflows that need to surface transient updates to the user: autopilot finding new jobs, the browser extension importing applications, and future integrations (Slack notifications, email alerts, calendar sync). Each source originally implemented its own UI surface — a tray "New jobs: N" counter, an import toast, session-level draft state — with no unified inbox or persistence. As the app grows, sources multiply and coexist: the user applies for a job while autopilot runs in the background, then closes the app and reopens it later. The tray counter resets, the import toast vanishes, and there is no record of what happened while the app was closed.

## Decision

The app adopts a **centralized, Rust-owned, persisted Notification Center** — a single source of truth for all notifications:

### 1. Persisted store (L1 data layer, Phase 1)

A pure `NotificationStore` in `apps/desktop/src-tauri/src/notifications/mod.rs` owns all notification records. It is:

- **JSON-file-backed**, persisted to `<dataDir>/notifications.json` — survives app restart and close-to-tray suspend.
- **Capped at 50 records**, newest-first; pushing past the cap drops the oldest.
- **Character-clamped** at the store boundary (title ≤ 200 chars, body ≤ 500 chars, counted as characters not bytes to preserve UTF-8). This guards against untrusted/scraped input from sources like autopilot and the browser extension — the clamp is enforced once per notification, for every current and future source.
- **Deliberately `AppHandle`-free** — no Tauri imports beyond serde. This keeps the store pure data + disk, unit-testable without a Tauri runtime. Push orchestration (OS banner, event emission, tray interaction) is delegated to the shell layer (Phase 4).
- Records are `AppNotification { id, kind, title, body, createdAt, read, route? }` where `kind` is an open string (e.g., `"autopilot.new_jobs"`, `"import.result"`) for zero-codebase-change extensibility.

### 2. IPC surface (L3 shell, Phase 2)

Commands in `apps/desktop/src-tauri/src/commands/notifications.rs` expose read/mutate operations:

- `notifications_list()` → all notifications, newest-first.
- `notifications_mark_read(id)`, `notifications_mark_all_read()`, `notifications_remove(id)`, `notifications_clear_all()` → mutate + emit `notifications:changed` event so the renderer refetches a live inbox.
- `notifications_clicked(id)` → unified target for OS-banner and tray clicks; focuses the main window, emits `notifications:open` event so the renderer opens the inbox.

All store methods are infallible (reads return values, mutators return `()`), so handlers are infallible too. No `AppError` serialization.

### 3. Push orchestration (L3 shell, Phase 4a)

A helper `push_and_notify(app, input, banner_policy)` centralizes delivery:

- `OsBanner::Always` — banner even when the app window is focused (autopilot use case: background run finished, user wants OS-level nudge regardless).
- `OsBanner::WhenUnfocused` — banner only when the app is NOT focused; when focused, the in-app toast covers it (extension import use case: responsive, don't startle the user who is already looking at the app).
- `OsBanner::Never` — reserved for future use; inbox + toast only, no OS banner.

Ordering:

1. `store.push(input)` — persist, assign id/created_at.
2. `notifications:changed` event — live inbox refetches.
3. If main window is focused: `notifications:toast` event with `{ title, body, route }` (lifted from the new record) so the renderer shows an in-app toast.
4. OS banner when `Always`, or `WhenUnfocused && !focused` — single permission-gated `show_os_notification()` path (the only `.show()` call in the app).

### 4. Navigation intent (zero-change extensibility)

Each notification carries an optional `route: { to, search? }` — the destination the renderer navigates to when the notification is actioned (from the inbox "View" button, toast "View" link, or deep-link from OS banner). This is **open-typed** (no enum) so new sources need zero codebase changes: the renderer self-consumes the route and navigates using the app's standard router.

Example routes:

- `/autopilot?focus=<job-id>` — autopilot source provides the job ID in search params; the `/autopilot` page reads `focus` on mount and scrolls into the matching job.
- `/applications?highlight=<app-id>` — extension source provides the application ID; the `/applications` page reads `highlight` and visually highlights the matching row.

The route pattern **generalizes**: any new source (Slack notification listener, email alert watcher, calendar sync) pushes a notification with the appropriate route, and the destination page is responsible for parsing its own route params.

### 5. Layering and ownership

`notifications` is an **L3 shell module** (like `extension_bridge`, `tray`, `updater`). Its `manage()` registration helper in `main.rs` is the single integration point:

```rust
let mut reset_reg = ResetRegistry::new();
let notifications_store = NotificationStore::new(data_dir);
manage_resettable(app, &mut reset_reg, "notifications", notifications_store);
```

This pairs store registration with factory-reset coverage — the Resettable trait ensures `privacy_reset_app` clears all notifications without additional code. (See ADR-009.)

## Consequences

- **Unified user experience:** all notifications live in one inbox with a consistent read/unread status, "View" action routing, and persistence across app lifecycle.
- **Single-source attack surface:** untrusted input (job titles, body copy from scraped résumés, extension names) enters through `push()`, where character clamping is enforced once. No duplicate guards per source.
- **Zero-change extensibility:** new notification kinds (`"slack.new_message"`, `"email.alert"`) and new destination pages (`/integrations`) require zero Notification Center changes — the open `kind` string and open `route` struct absorb them. Existing sources (autopilot, extension) do not need refactoring.
- **Desktop `onAction` is identity-free:** the OS notification carries no per-notification id (Tauri's notification API does not support payload). A banner click opens the in-app inbox rather than deep-linking directly to a specific notification. Per-record routing happens from within the inbox ("View" button) or the toast's "View" link.
- **English-only persisted content (Phase 5):** record titles and bodies are generated by the Rust orchestration layer in English (the `push_and_notify` callsites decide the text). Localizing persisted record content is a tracked follow-up; for now, the inbox displays English text across all locales. Toast `title` and `body` are rendered as plain text (no DOM), and route-based breadcrumbs (e.g., "Application saved") are localized by the destination page.
- **Residual coupling:** sources (autopilot, extension_bridge) in L1/L2 reach into the shell to call `push_and_notify(app, …)`. This is deferred to Phase 4b — the target is to inject a push-handler port so L1/L2 never touches the shell. For now, each source imports `crate::commands::notifications::{push_and_notify, OsBanner}` (a re-export from the Phase 1 store's public API in the shell).

## Considered options

1. **Centralized Rust store + shell-layer push orchestration (chosen).** Single source of truth, survives app restart, extensible via open types, least-privilege character clamping at the boundary, unit-testable store body (AppHandle-free).

2. **Session-only in-renderer React state (like the old ImportToastBridge/tray counter).** Lost on app close; separate source-specific code paths for tray / toast / deep-link; no unified inbox. Rejected.

3. **Cloud-backed notification inbox (Firebase, Supabase, local server on the machine).** Additional infrastructure, network latency, potential data leakage surface, vendor lock-in. Rejected.

## Related ADRs

- **ADR-009** (Resettable registry) — `notifications` integrates via `manage_resettable()`, ensuring factory reset clears all persisted notifications.
- **ADR-001** (Rust-first business logic) — the Rust store is the authoritative source; IPC is a thin read surface.
- **ADR-015** (Extension bridge) — the browser extension imports are one notification source; they route through `push_and_notify(app, …, OsBanner::WhenUnfocused)`.
