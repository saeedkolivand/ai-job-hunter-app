# Notification Center

**Owning symbols:**

- Store: `apps/tauri/src-tauri/src/notifications/mod.rs` — `NotificationStore`, `AppNotification`, `NewNotification`, `NotificationRoute`
- Commands: `apps/tauri/src-tauri/src/commands/notifications.rs` — `notifications_list`, `notifications_mark_read`, `notifications_mark_all_read`, `notifications_remove`, `notifications_clear_all`, `notifications_clicked`, `push_and_notify`, `OsBanner`
- IPC contract: `packages/shared/src/ipc/contracts/notifications.ts` — request/response schemas
- Renderer service: `apps/tauri/src/renderer/services/use-notifications.ts` — query hook `useNotificationsList`, `useNotificationsEvents`, mutation hooks `useMarkRead`, `useMarkAllRead`, `useRemove`, `useClearAll`, `useNotificationClicked`
- UI: `apps/tauri/src/renderer/components/layout/Titlebar/NotificationBell.tsx` — bell icon + unread badge + dropdown inbox; `apps/tauri/src/renderer/routes/__root.tsx` — `NotificationToastBridge` for toast rendering; app routes (`/autopilot`, `/applications`) consume `focus` / `highlight` query params
- Sources: autopilot `apps/tauri/src-tauri/src/autopilot/mod.rs` (`tray::on_new_jobs`, pushes with `OsBanner::Always`); browser extension `apps/tauri/src-tauri/src/extension_bridge/mod.rs` (success branch, pushes with `OsBanner::WhenUnfocused`)

**Decision:** ADR-016 (centralized, Rust-owned, persisted store; open-typed kind + route for extensibility; English-only content Phase 1; desktop onAction identity-free routing to inbox)

**Setup:**

- Tauri `main.rs` registers via `manage_resettable(app, &mut reset_reg, "notifications", NotificationStore::new(data_dir))`
- Capabilities: `notification:allow-notify`, `notification:allow-show`, `notification:allow-request-permission`, `notification:allow-check-permissions`, `notification:allow-is-permission-granted`, `notification:allow-permission-state`

**Integration:** sources call `push_and_notify(app, NewNotification { kind, title, body, route }, OsBanner)` to create and broadcast; renderer refetches inbox on `notifications:changed` event; toast appears on `notifications:toast` event when focused; OS banner appears per policy; deep-link from banner/tray goes to `notifications_clicked` → inbox open
