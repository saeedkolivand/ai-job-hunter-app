# Self-hosted sync protocol (v1)

Status: specification. The client that speaks it is not implemented yet; see
[ADR-051](decision-records/adr-051-optional-self-hosted-multi-device-sync.md) for the decision and
the delivery plan. AI Job Hunter does not ship or operate a server: this page is the contract a
user-run server implements.

Keywords **MUST**, **MUST NOT**, **SHOULD** and **MAY** are used in the RFC 2119 sense.

## Roles

- **Client:** an AI Job Hunter install with sync enabled. Each install has a stable, randomly
  generated **device id** (an opaque string, unique per install).
- **Server:** any HTTP service that implements the endpoints below. It stores **opaque
  envelopes** and never needs to parse `payload`.
- **Namespace:** the server-side bucket a set of devices share. The server derives it from the
  bearer token; the protocol has no namespace parameter. Two devices sync with each other exactly
  when the server maps their tokens to the same namespace.

## Authentication and transport

- Every request carries `Authorization: Bearer <sync token>`. A missing or unknown token MUST get
  `401`. How tokens are issued and mapped to namespaces is up to the server operator.
- The server URL is a base URL; endpoint paths are appended to it.
- `https://` is the expected transport. A client MAY accept `http://`, but MUST warn the user that
  traffic is unencrypted unless the link is already protected (for example Tailscale or a trusted
  LAN).
- There is no end-to-end encryption in v1: whoever operates the server can read synced data.

## Endpoints

### `GET /health`

Returns `200` with a JSON object containing at least `protocolVersion` (integer). This document
defines protocol version `1`. A client MUST NOT sync with a server whose `protocolVersion` it does
not support; it reports the mismatch instead. The endpoint MAY be called without a token; if it
requires one, the same bearer rules apply.

### `POST /changes`

Uploads changes. Request body:

```json
{ "deviceId": "<device id>", "changes": [<envelope>, ...] }
```

- The server MUST append the whole batch atomically: either every envelope is stored, or none is
  and it returns an error status.
- On success it returns `200` with `{ "accepted": <number of envelopes stored> }`.
- The server MUST NOT reject or rewrite an envelope because of its `payload`, `store`, `rev` or
  `schemaVersion`; it only validates that the body is well-formed JSON with the envelope fields
  present and correctly typed (`400` otherwise).
- Duplicate uploads are allowed. Applying is idempotent (see [Applying changes](#applying-changes)),
  so a client that retries after a timeout cannot corrupt anything.
- A server MAY cap the batch size and answer `413`; the client then retries with smaller batches.

### `GET /changes?since=<cursor>`

Downloads changes in the order the server stored them.

- `since` is the cursor from the previous response. Omitted or empty means "from the beginning".
- Response: `200` with

  ```json
  { "changes": [<envelope>, ...], "cursor": "<opaque string>", "hasMore": <boolean> }
  ```

- `cursor` identifies the position after the last returned envelope. Passing it back returns
  only envelopes stored after that position. It is opaque to the client and MUST stay valid for
  as long as the server keeps the data.
- When `hasMore` is `true` the client repeats the call with the new cursor until it is `false`.
- The server returns every envelope in the namespace, including ones uploaded by the requesting
  device. The server does not filter by device.
- An empty page returns `"changes": []`, `"hasMore": false` and a cursor the client can reuse.

## Envelope

```json
{
  "store": "<export bundle section key>",
  "recordId": "<record id within that section>",
  "rev": <integer>,
  "deviceId": "<device id of the writer>",
  "deleted": <boolean>,
  "schemaVersion": <integer>,
  "payload": <JSON value or null>
}
```

- `store` — the export bundle section the record belongs to. The valid keys are whatever the
  bundle contains: `ARRAY_SECTIONS` and `OBJECT_SECTIONS` in
  `apps/desktop/src-tauri/src/commands/data.rs`, each matching a store's `DataStore::key()`.
  Servers MUST NOT validate it against a fixed list; a newer client may add sections.
- `recordId` — the record's own id inside the section. For a single-object section (a settings
  store with one row), the record id is the section key itself. The exact per-section id source
  is defined by the client implementation and pinned in its tests.
- `rev` — the writer's sync-run time in milliseconds since the Unix epoch, taken once per sync run
  and shared by every envelope in that run. It orders writes; it is not a server sequence number.
- `deviceId` — the writing device. Used only for tie-breaking.
- `deleted` — `true` for a tombstone. A tombstone's `payload` is `null`.
- `schemaVersion` — the schema version of the writing store at the time of export.
- `payload` — the record exactly as it appears in that section of the export bundle
  (`DataStore::export`'s JSON shape). Never contains secrets: the bundle excludes keychain
  credentials by design (`data_store.rs`).

## Client behaviour (normative for AI Job Hunter, informative for servers)

### Building the outgoing batch

1. Build the export bundle (`build_bundle`).
2. Hash each record in each section.
3. Compare with the **baseline**: the record hashes saved at the last successful sync, in the
   local sync-state store. Sync state (baseline and cursor) is keyed by the server URL **and** the
   sync token, because the token selects the namespace. When either changes, the client MUST
   discard the saved baseline and cursor and treat the next run as a first sync.
4. A record that is new or whose hash differs becomes an upsert envelope. A record present in the
   baseline but absent now becomes a tombstone.
5. **First sync:** with no baseline for this server URL and token, the client MUST NOT produce tombstones.
   It uploads every record as an upsert.

### Order of a sync run

1. `GET /health` and check `protocolVersion`.
2. Pull: `GET /changes?since=<saved cursor>` until `hasMore` is `false`, applying each page.
3. Push: build the outgoing batch (above) and `POST /changes`.
4. Save the new baseline only after the push succeeded.

Applying a pulled record also updates that record's baseline hash, so a record received from
another device is not echoed back as a local change on the next push.

### Acknowledgement and resuming

- The client saves the cursor of a page only after every envelope in it has been applied (or held,
  see [Schema versions](#schema-versions)). The saved cursor is the acknowledgement; the server
  keeps no per-device read position.
- If a run is interrupted (crash, network loss, app closed), the next run resumes from the last
  saved cursor. Re-reading envelopes that were already applied is harmless.
- If the push fails, the baseline is not advanced, so the same changes are computed and sent again
  next time.

### Applying changes

For each pulled envelope the client compares it with what it last applied or wrote for the same
`(store, recordId)`. A record changed locally since the baseline (and not yet pushed) counts as
written with the current run's `rev` and this device's `deviceId`, so the comparison below decides
between it and the incoming change:

- **Last sync wins.** The envelope with the greater `rev` wins. If `rev` is equal, the greater
  `deviceId` (byte-wise comparison) wins. A losing envelope is ignored.
- **Never downgrade a record's schema.** The client records the `schemaVersion` each record was
  last written or applied at. An envelope with a lower `schemaVersion` than that MUST NOT replace
  the record, even with a greater `rev`; it is ignored, so newer fields are never lost. On the
  sending side, a client that holds an unapplied newer-schema envelope for a record (see
  [Schema versions](#schema-versions)) MUST NOT push its own change for that record until it has
  been updated and applied the held envelope.
- **Whole record.** A winning upsert replaces the whole local record with `payload`. There is no
  field-level merge. Known ceiling: if two different fields of one record were edited on two
  devices between syncs, one edit is lost.
- **Append-only data unions.** History-style data (application status events, spend rows,
  pipeline-run events) is stored as separate records with their own ids, so records from different
  devices never collide and are all kept.
- **Tombstones are final.** A winning tombstone deletes the local record, and the record id stays
  deleted: a later upsert for the same `(store, recordId)` is ignored, so a deleted record cannot
  come back from a device that still holds a stale copy.
- **Per record, inside the store's transaction.** Each record is applied through its owning
  store inside that store's own transaction
  ([ADR-022](decision-records/adr-022-atomic-store-transactions-and-centralized-db.md)). A sync
  run never clears or wholesale-replaces a store.
- **Idempotent.** Applying the same envelope twice has the same effect as applying it once.

### Schema versions

- An envelope whose `schemaVersion` is newer than the local store's is **held, not applied**: the
  client keeps it locally and applies it after the app is updated to a version that understands
  it. Holding counts as handled for cursor acknowledgement.
- A client never down-converts a record to an older schema.
- An envelope from an older schema is applied through the store's normal import path for that
  shape.

### Failure handling

- **Server unreachable or `5xx`:** skip the run quietly and keep the last-synced time. Local data is
  untouched and the app keeps working.
- **`401`:** stop syncing and tell the user the token was rejected.
- **Malformed response** (invalid JSON, missing fields, wrong types): reject the whole response,
  apply nothing from it and do not advance the cursor.
- **Unsupported `protocolVersion`:** do not sync; report the mismatch.

## Security and privacy

- The token is a device-local secret kept in the OS keychain. It is never logged, never sent to
  crash reporting and never written to the export bundle.
- Envelopes carry only export-bundle data; keychain credentials and the other excluded stores
  never leave the device through sync.
- The server operator can read every payload in v1. Run the server yourself, or only on a host you
  trust.
- Egress rules: the self-hosted sync egress class in [ADR 0005](decision-records/0005-network-egress-privacy-boundary.md).

## Running your own server

The project does not provide a server. A compatible one only needs to:

1. serve the endpoints above over HTTPS (a reverse proxy such as Caddy or nginx can
   terminate TLS), or over `http://` only on a link that is already private;
2. map bearer tokens to namespaces and reject unknown tokens with `401`;
3. append envelopes atomically to a per-namespace log that survives restarts (a single SQLite
   table is enough) and hand out a cursor that only moves forward;
4. return changes in stored order, paginated with `hasMore`;
5. report `protocolVersion` on `/health`;
6. be backed up like any other personal data store.

Typical hosts are a home server or old laptop reachable over Tailscale, a NAS, a Raspberry Pi, a
VPS or any Docker host. Community-maintained reference servers may be linked here as unofficial.
