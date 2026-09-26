//! The tail of `generate_handler!`: the loopback bridge's own
//! commands, and email watch.
//!
//! One contiguous shard of `POLICY`, split out under R8's LOC cap and
//! concatenated back in `lib.rs`'s `generate_handler!` order by the parent.

use super::*;

pub(super) const BRIDGE_AND_EMAIL_WATCH: &[PolicyEntry] = &[
    // commands/extension_bridge.rs
    //
    // MCP HIGH fix (security critique on the MCP server pass): reclassified
    // NotExposed from `Read` — the command returns `token: state.token()`
    // verbatim (`commands/extension_bridge.rs`), the bridge's ONLY secret.
    // `Effect::Read` on this table has always meant "safe for the generic
    // tier to hand back raw" (ADR-038 §3's amendment of ADR-0005), which was
    // sound for a caller who already read the same token off disk to even
    // reach this dispatcher — but an MCP client is a DIFFERENT recipient: it
    // sends every tool result to its own cloud model provider and persists
    // it in an on-disk transcript, neither of which ever touched the token
    // file. No `ProofSource` in this table reads this row (grep
    // `read_command: "extension_bridge_status"`: zero hits), so no ceremony
    // depends on it — the Settings UI's own status check is unaffected,
    // only this CLI/MCP dispatch surface loses the ability to name it.
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_status",
        effect: Effect::NotExposed(
            "returns the plaintext pairing token verbatim — the bridge's only secret; a \
             generic-tier or MCP caller must never receive it",
        ),
    },
    // Rotates the pairing token, which REVOKES every currently-paired
    // browser session. NotExposed (not `Irreversible`) — no proof reachable
    // from this table can ever bind the ceremony to anything the caller
    // didn't already have: `port`/`token` are values this exact CLI
    // connection already needed to possess to authenticate at all, and
    // `extension_bridge_status`'s own `connected` field is now itself
    // NotExposed (see the row immediately above) as well as being a
    // guaranteed-true boolean for the whole call regardless (this VERY
    // socket's own authentication is what increments that counter). No
    // other Read row in this namespace binds to the pairing session this
    // revokes either, and the command's only real-world effect — breaking
    // the user's OWN browser pairing, a UI Settings self-service action —
    // has no job-hunting workflow motivating an autonomous agent to drive
    // it. A ceremony that can never fail is worse than an honest refusal.
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_regenerate_token",
        effect: Effect::NotExposed(
            "rotating the pairing token has no reachable non-vacuous proof: port/token are \
             values this exact connection already had to possess to authenticate, and \
             connected reads true only because THIS socket's own authentication is what \
             increments it — see the row's own comment for the full argument",
        ),
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_autofill_enabled",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_set_autofill_enabled",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_ai_assist_enabled",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_set_ai_assist_enabled",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_auto_track_enabled",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_set_auto_track_enabled",
        effect: Effect::Reversible,
    },
    // commands/email_watch.rs
    PolicyEntry {
        path: "commands::email_watch::email_watch_status",
        effect: Effect::Read,
    },
    // Writes an IMAP app-password secret into the OS keychain —
    // `credentials:*`. A fresh connect has nothing to compare against; the
    // strongest available signal is whether an account is ALREADY connected
    // — WEAK (boolean, mostly `false` on the common first-connect path),
    // flagged.
    PolicyEntry {
        path: "commands::email_watch::email_watch_connect",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "email_watch_status",
            path: &["connected"],
        }),
    },
    // Removes the stored secret; also clears the auto-write opt-in + every
    // seen-mail dedupe row (verified — `EmailWatchStore::clear`'s own doc).
    // Proof is the CONNECTED ACCOUNT'S own address, read via
    // `email_watch_status` — real, user-owned data; genuinely requires
    // having read it first.
    PolicyEntry {
        path: "commands::email_watch::email_watch_disconnect",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "email_watch_status",
            path: &["address"],
        }),
    },
    PolicyEntry {
        path: "commands::email_watch::email_watch_set_enabled",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::email_watch::email_watch_set_auto_write_enabled",
        effect: Effect::Reversible,
    },
    // Rate-limited (60s) fetch+parse+match+notify pass; may flip a tracked
    // Application's status when auto-write is on — reversible via
    // applications_set_status, nothing destroyed.
    PolicyEntry {
        path: "commands::email_watch::email_watch_check_now",
        effect: Effect::Reversible,
    },
    // export/commands/mod.rs
    // Verified: renders and returns bytes only — no filesystem write.
    PolicyEntry {
        path: "export::commands::documents_export_document",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "export::commands::documents_export_and_save",
        effect: Effect::NotExposed(
            "renders the document then blocks on a native OS save-file dialog \
             (tauri_plugin_dialog::blocking_save_file); no argv/JSON equivalent for a \
             non-interactive caller",
        ),
    },
    PolicyEntry {
        path: "export::commands::documents_render_preview_images",
        effect: Effect::Read,
    },
    // updater/mod.rs
    // A network probe of the release feed (`updater.check().await`). It
    // writes `UpdaterState` (`pending_version`/`pending_update`/clears
    // `downloaded_bytes`), and that write IS observable through another
    // command: it is exactly what `updater_download` reads to decide which
    // artifact to transfer, so this row selects the install target for the
    // rest of the check→download→install flow, not just for its own next
    // call. It also emits `updater:status` to the renderer (`checking`,
    // then `available`/`not-available`/`error`), which changes what the
    // user sees. `Reversible`, not `Read` (reverted from a Read
    // reclassification, issue #1165): `Read`'s "no state change" promise
    // covers the whole `call-read` TOOL, not one row, so making this row
    // Read would have forced `readOnlyHint` to `false` for every other
    // `Read` row on this surface too. `updater::updater_status` below is
    // the read-only alternative.
    PolicyEntry {
        path: "updater::updater_check",
        effect: Effect::Reversible,
    },
    // The read-only counterpart of `updater_check` above: reports whatever
    // `updater_check` or the automatic silent check already found, with no
    // network call and no `updater:status` emission — genuinely `Read`
    // (issue #1165's follow-up).
    PolicyEntry {
        path: "updater::updater_status",
        effect: Effect::Read,
    },
    // Downloads the update artifact into memory/state — not yet applied, nothing destroyed.
    PolicyEntry {
        path: "updater::updater_download",
        effect: Effect::Reversible,
    },
    // Installs the downloaded update and force-restarts the app
    // (`app.restart()`, never returns) — replaces the running binary with
    // no undo path. `UpdaterState.pending_version` otherwise lives only in
    // memory behind `updater_download`, which stays `Reversible`, so it is
    // not eligible as a proof source. `updater_status`'s reply carries the
    // PENDING version at `version` — the one about to be installed, not the
    // currently running one — so the proof below reads it there (issue
    // #1171), replacing the former `system_get_version` proof, which named
    // the wrong version and was vacuous besides. The real safety boundary
    // here is still `updater_download`'s minisign signature check, not this
    // ceremony; this proof only confirms the caller is targeting the
    // version that will actually be installed.
    PolicyEntry {
        path: "updater::updater_install",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "updater_status",
            path: &["version"],
        }),
    },
    PolicyEntry {
        path: "updater::updater_changelog",
        effect: Effect::Read,
    },
];
