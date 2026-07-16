# Notification Center

**Owning symbols:**

- Store: `apps/desktop/src-tauri/src/notifications/mod.rs` — `NotificationStore`, `AppNotification`, `NewNotification`, `NotificationRoute`
- Commands: `apps/desktop/src-tauri/src/commands/notifications.rs` — `notifications_list`, `notifications_mark_read`, `notifications_mark_all_read`, `notifications_remove`, `notifications_clear_all`, `notifications_clicked`, `push_and_notify`, `OsBanner`
- IPC contract: `packages/shared/src/ipc/contracts/notifications.ts` — request/response schemas
- Renderer service: `apps/desktop/src/renderer/services/use-notifications/use-notifications.ts` — query hook `useNotifications`, subscription hook `useNotificationEvents`, mutation hooks `useMarkNotificationRead`, `useMarkAllNotificationsRead`, `useRemoveNotification`, `useClearAllNotifications`
- UI: `apps/desktop/src/renderer/components/layout/Titlebar/NotificationBell/index.tsx` — bell icon + unread badge + dropdown inbox; `apps/desktop/src/renderer/routes/__root.tsx` — `NotificationToastBridge` for toast rendering; app routes (`/autopilot`, `/applications`) consume `focus` / `highlight` query params
- Sources: autopilot `apps/desktop/src-tauri/src/tray/mod.rs` (`on_new_jobs`, pushes with `OsBanner::Always`); browser extension `apps/desktop/src-tauri/src/extension_bridge/import_flow.rs` (import success) and `extension_bridge/status_update.rs` (applied/auto-track status, pushes with `OsBanner::WhenUnfocused`); email-confirmation watching `apps/desktop/src-tauri/src/email_watch_scheduler.rs` (ADR-0013)

**Decision:** ADR-016 (centralized, Rust-owned, persisted store; open-typed kind + route for extensibility; English-only content Phase 1; desktop onAction identity-free routing to inbox)

**Setup:**

- Tauri `lib.rs` registers via `notifications::manage(app, &mut reset_registry, &data_dir)` (line ~725)
- Capabilities: `notification:allow-notify`, `notification:allow-show`, `notification:allow-request-permission`, `notification:allow-check-permissions`, `notification:allow-is-permission-granted`, `notification:allow-permission-state`, `notification:allow-register-listener`

**Integration:** sources call `push_and_notify(app, NewNotification { kind, title, body, route }, OsBanner)` to create and broadcast; renderer refetches inbox on `notifications:changed` event; toast appears on `notifications:toast` event when focused; OS banner appears per policy; deep-link from banner/tray goes to `notifications_clicked` → inbox open
