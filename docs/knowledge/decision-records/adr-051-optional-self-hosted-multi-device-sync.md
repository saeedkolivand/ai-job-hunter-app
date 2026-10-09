# ADR-051 — Optional self-hosted multi-device sync

**Status:** Accepted

**Date:** 2026-10-08

**Deciders:** repo owner (direction given on issue #1402), contributor

## Context

AI Job Hunter is local-first: every user-data store lives on the device, and there is no
app-operated backend ([ADR 0005](0005-network-egress-privacy-boundary.md)). People who use the app
on more than one computer can only move state with Settings → Privacy → export/import.

That path is a backup tool, not a sync tool. `DataStore::import` (`apps/desktop/src-tauri/src/data_store.rs`)
has REPLACE semantics by design, and `data_import` (`apps/desktop/src-tauri/src/commands/data.rs`)
restores a whole snapshot. If both machines changed since the last export, importing on one of
them silently discards the other's work. The project also does not want to host user data, so a
first-party cloud is not an option.

Most stores have no per-record `updated_at`, so a write-path change log would need a migration
and write-path changes in every store.

## Decision

1. **Optional, off by default.** Sync is an explicit opt-in. With it disabled, behaviour and
   on-disk data are unchanged. The app stays fully usable offline: sync is a background
   reconcile, never a dependency, and never blocks the UI.
2. **Client only, user-run server.** The app implements the client side of a small documented
   HTTP protocol ([sync-protocol.md](../sync-protocol.md)) and talks only to an endpoint the user
   configures. The project does not ship, host or maintain a sync server. Users run any compatible
   server (for example an old laptop reachable over Tailscale, a NAS or a VPS). An unofficial
   reference server may be linked from the docs, but it is not part of this repository.
3. **Change detection by diff at sync time.** No write-path change log and no per-store
   `updated_at` migration for sync's sake. On each sync the client builds the export bundle
   (`build_bundle`, `commands/data.rs`), hashes every record, and compares the hashes with those
   saved at the last successful sync **for that server URL** in a small local sync-state store.
   New or changed records become upserts; records present in the baseline but missing now become
   tombstones. A device's **first** sync has an empty baseline and must never produce tombstones,
   so a fresh install cannot wipe another machine.
4. **Scope is the whole export bundle.** Every section the bundle contains syncs (the canonical
   list is `ARRAY_SECTIONS` + `OBJECT_SECTIONS` in `commands/data.rs`). Everything the bundle
   already excludes stays device-local; the exclusion rule is documented on `DataStore`
   (`data_store.rs`) and enforced by what `build_bundle` assembles.
5. **Transport and auth.** Plain HTTP API with `GET /health`, `POST /changes` and
   `GET /changes?since=<cursor>`, authenticated with a bearer sync token. The server stores opaque
   envelopes per namespace and never has to understand the payload.
6. **TLS expected, warned `http://` allowed, no end-to-end encryption in v1.** The server operator can read synced data. A
   `http://` URL is allowed, but the Sync settings must warn that data travels unencrypted unless
   the link is already protected (Tailscale, a trusted LAN). This is a new user-configured egress
   class, recorded as an amendment to [ADR 0005](0005-network-egress-privacy-boundary.md).
7. **Conflicts: whole record, last sync wins.** The whole record is replaced; the change from the
   later sync wins and ties are broken by device id. Append-only data (status history, spend rows)
   consists of separate records with their own ids, so it unions and is never overwritten.
   Deletes are tombstones, so a deleted record cannot come back. Accepted ceiling: if two
   different fields of the same record are edited on two machines between syncs, one edit is
   lost.
8. **Schema versions.** Every envelope carries its store's schema version. A client keeps changes
   from a newer schema without applying them until the app is updated, and never down-converts.
9. **Failure safety.** Sync never replaces a store wholesale. Each record is applied inside its
   store's own transaction ([ADR-022](adr-022-atomic-store-transactions-and-centralized-db.md)).
   An interrupted run resumes from the last acknowledged cursor; a malformed response is rejected,
   never applied. An unreachable server skips the run quietly and the UI shows "last synced".
10. **Triggers (runtime, later PRs).** On launch, on a periodic interval (decided as every 15
    minutes; the client engine owns it as a source constant, which becomes the reference), and
    from a "Sync now" button.
11. **Secrets.** The sync token is a device-local credential in the OS keychain, like the other
    credentials. It is never logged, never sent to crash reporting and never written to the
    export bundle. Keychain contents never enter sync.
12. **No heavy dependencies.** No CRDT or replication framework; prefer crates already in the
    tree.

## Considered options

1. **Diff the export bundle at sync time, whole-record last-sync-wins (chosen).** Reuses the
   existing `DataStore` contract, needs no migration, keeps the server trivial. Cost: the
   two-field-edit ceiling in decision 7, and a full-bundle hash per sync run.
2. **Per-write change log with hybrid logical clocks** (the original proposal on #1402).
   Rejected: needs a migration plus write-path changes in every store for a benefit the diff
   already delivers at this data size.
3. **Sync the snapshot as-is (export on one device, import on another through a server or
   shared folder).** Rejected: REPLACE semantics loses data whenever both devices changed.
4. **Sync the SQLite files with a file-sync tool.** Rejected: unsafe with WAL-mode databases and
   concurrent writers, and it would also carry machine-local files.
5. **CRDT or SQLite replication frameworks.** Rejected: large dependency and schema impact for a
   problem whole-record last-sync-wins solves for this data model.
6. **A first-party hosted sync service.** Rejected: contradicts the no-backend principle and puts
   user data on the maintainer.

## Consequences

- The wire contract lives in [sync-protocol.md](../sync-protocol.md); any third party can implement
  a compatible server from it.
- [ADR 0005](0005-network-egress-privacy-boundary.md) gains egress class 9 (self-hosted sync).
- No runtime change ships with this ADR. Delivery is incremental, each step mergeable on its own:
  1. this ADR, the ADR 0005 amendment and the protocol doc;
  2. the client sync engine behind a default-off setting (bundle diff, sync-state store, HTTP
     client through `net::http`, per-record apply, scheduler), with in-memory SQLite tests and the
     endpoint added to the egress inventory (`apps/desktop/src-tauri/tests/egress.rs`);
  3. the Settings → Data → Sync card (enable, server URL, token, device name, test connection,
     sync now, last sync and health, the `http://` warning), with en and de strings.
- README and SECURITY privacy wording is updated in the PR that ships the runtime egress, not
  before.

## References

- Issue and maintainer direction: #1402.
- Export bundle: `build_bundle`, `data_export`, `data_import`, `ARRAY_SECTIONS`, `OBJECT_SECTIONS`
  in `apps/desktop/src-tauri/src/commands/data.rs`; `DataStore` in
  `apps/desktop/src-tauri/src/data_store.rs`.
- Store transactions: [ADR-022](adr-022-atomic-store-transactions-and-centralized-db.md).
- Egress boundary: [ADR 0005](0005-network-egress-privacy-boundary.md).
- Wire contract: [sync-protocol.md](../sync-protocol.md).
