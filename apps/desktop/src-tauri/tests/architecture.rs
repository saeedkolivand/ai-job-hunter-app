//! Architecture boundary tests — the machine-enforced contract for the Rust core.
//!
//! Derived from the June 2026 architecture discovery analysis (in git history) and codified in
//! `docs/architecture-rules.md` (the layer model + rule IDs R1–R8). This is a
//! **standalone integration test**: it uses only `std` and scans the source tree
//! under `CARGO_MANIFEST_DIR/src` as TEXT, deliberately without linking the crate —
//! a rule about which module may import which cannot be checked from inside a
//! build that has already resolved those imports. (`tests/eval.rs` does the
//! opposite and links `ajh_tauri`, because it runs the validators.) The crate is a
//! thin binary (`main.rs`) over a library (`lib.rs`, which holds the app + the
//! Tauri builder); both are L3 shell.
//!
//! Each rule has an explicit allowlist of *current* exceptions so the suite is green
//! today while blocking **new** violations (drift prevention). Allowlists are debt,
//! not absolution: an allowlisted file that no longer trips its rule makes the
//! corresponding `*_allowlist_has_no_dead_entries` check fail, so they cannot rot.
//!
//! Run: `cargo test --test architecture`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

// ── Layer model (single source of truth; mirrors docs/architecture-rules.md) ────────
// L0 = shared infra, L1 = domain, L2 = application, L3 = shell/IPC. Dependencies flow
// downward only (higher layer may use lower; never the reverse).
const L0: &[&str] = &[
    "error",
    "observability",
    "performance",
    "db",
    "data_store",
    "net",
    "platform",
    // Process-local anti-abuse limiter (in-memory rate/concurrency); depends only on `error`.
    "limits",
    // Pure vector math (cosine similarity), shared by `commands::ai_provider`
    // (embedding compare) and `scraping::cluster` (cross-board dedup) so the L1
    // cluster module reuses it without an upward import (R7). Depends on nothing.
    "vector",
    // ADR-010 prompt-injection fencing primitives (`fenced`/`FENCE_TAG_PATTERNS`/
    // `neutralize_transcript_boundaries`/`JOB_CAP`/`RESUME_CAP`) — PR-5 step 1
    // relocated them out of the L3 `agent` module (dependency-free string
    // transforms, no Tauri) so `pipeline`, `commands::ai_provider::structured`,
    // `extension_bridge`, `autopilot_helpers` and `agent` itself can all reach
    // them downward. This is what clears the `pipeline -> agent` and
    // `autopilot_helpers -> agent` R7_ALLOW exceptions below.
    "prompt_fence",
];
const L1: &[&str] = &[
    "scraping",
    "extraction",
    "export",
    "documents",
    "jobs",
    "postings",
    // Cross-board dedup verdict store (ADR-029): a per-domain SQLite store of
    // user "not a duplicate" pair tombstones. Tauri-free; imports only db/error/
    // data_store (L0), same posture as the other L1 stores.
    "dedup",
    // Passively-harvested ATS company slug store (ADR-030): a per-domain SQLite
    // store of discovered `(ats, slug)` refs + watched-company stars. Tauri-free
    // (its `harvest_ats_refs` seam is AppHandle-free — the L3 command handlers
    // resolve the store via `try_state` and pass it in), same posture as `dedup`
    // and the other L1 stores.
    "discovered",
    "credentials",
    "job_preferences",
    "contact_profile",
    "ai_generations",
    "applications",
    "referrals",
    "profile_import",
    "model",
    "layout",
    "measure",
    "validate",
    "locale",
    "theme",
    // AI-spend visibility store (real per-call token usage + estimated cost).
    // Same layer as ai_generations: a per-domain SQLite store, Tauri-free.
    "spend",
    // Backend-owned active AI provider store (task #16): the single source of
    // truth for generation routing. A per-domain SQLite store, Tauri-free.
    "ai_config",
    // Email-confirmation watching (task #23): the account/dedupe SQLite store
    // + the IMAP connector + the pure parser/matcher/poller (PR B). Stays
    // Tauri-free — non-test code here imports only db/error (L0) + itself
    // (the command layer, L3, is the one that reaches credentials). PR B's
    // decision on the poller's own upward reach into
    // `commands::notifications::push_and_notify`: a SEPARATE L2 module
    // (`email_watch_scheduler`, mirroring the `autopilot`/`autopilot_scheduler`
    // split) owns that exception — see its R7_ALLOW entry below — rather than
    // growing an R7_ALLOW on this L1 store.
    "email_watch",
    // Seed of the roadmap's "relocate ai_provider out of commands/" item: ONE
    // pure value type (`SearchBackend`) shared by two L2 siblings (`pipeline`,
    // `cover_letter`), split out here specifically because it has zero Tauri/
    // credential-reading logic — that stays L3 (`commands::ai_provider`),
    // which re-exports this type so its existing consumers are unaffected.
    "ai_provider",
];
const L2: &[&str] = &[
    "pipeline",
    "cover_letter",
    "salary_research",
    "autopilot",
    "autopilot_scheduler",
    "autopilot_helpers",
    "recommend",
    // Email-watch background poller (task #23, PR B) — mirrors the
    // `autopilot`/`autopilot_scheduler` split: the L1 `email_watch` store/
    // connector/parser/matcher/poller stay Tauri-free, and this is the one
    // module in the family that spawns from setup and reaches up into
    // `commands::notifications::push_and_notify`.
    "email_watch_scheduler",
    // Follow-up reminder sweep — same split for the same reason: the L1
    // `applications` aggregate owns the data (`follow_up_candidates` /
    // `mark_next_action_notified`) and stays Tauri-free, while this module
    // spawns from setup and pushes the notification.
    "reminder_scheduler",
];
const L3: &[&str] = &[
    "commands",
    "ipc_contracts",
    // Centralized Tauri-event emit layer (one helper + generated channel consts).
    "events",
    "main", // thin binary launcher
    "lib",  // shell entry point: holds the Tauri builder (`run()`); `main` just calls it
    "updater",
    "tray",
    "deeplink",
    // Loopback WS bridge for the browser extension. Shell-role: holds an
    // AppHandle, emits Tauri events, and reaches down into L1 (applications,
    // scraping) — never the reverse.
    "extension_bridge",
    // Persisted notification store (Notification Center, Phase 1). Shell-role:
    // its `manage` holds a `tauri::App` to register managed state + the
    // factory-reset hook, exactly like `extension_bridge`. The store body itself
    // is pure data + disk (AppHandle-free); push orchestration is Phase 4.
    "notifications",
    // Crash reporting (ADR-0020). Shell-role for two reasons: it is constructed
    // by `lib::run()` before the Tauri builder exists (the minidump supervisor
    // forks there), and it reaches DOWN into `commands::support::redact_lines` to
    // reuse the diagnostics redactor rather than growing a second, weaker one —
    // never the reverse. Its consent state is a plain file precisely because no
    // store or WebView exists that early.
    "crash_reporting",
    // The agentic controller foundation (Phase 1) that used to live here —
    // `agent` — was deleted in its entirety (PR-5 step 2): the "prep this
    // application" flow, the human-in-the-loop confirm gate, and the
    // tool-calling loop it drove. See `prompt_fence` (L0) for what survived
    // the deletion.
];

fn layer_of(module: &str) -> Option<u8> {
    if L0.contains(&module) {
        Some(0)
    } else if L1.contains(&module) {
        Some(1)
    } else if L2.contains(&module) {
        Some(2)
    } else if L3.contains(&module) {
        Some(3)
    } else {
        None
    }
}

// ── Source-tree access ──────────────────────────────────────────────────────────────

struct RsFile {
    /// Path relative to `src/`, always forward-slashed (e.g. `cover_letter/mod.rs`).
    rel: String,
    /// First path segment (= top-level module), or the file stem for crate-root files.
    module: String,
    content: String,
    is_test: bool,
}

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<RsFile>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let module = match rel.split_once('/') {
                Some((head, _)) => head.to_string(),
                None => rel.trim_end_matches(".rs").to_string(),
            };
            let is_test = rel.ends_with("test.rs") || rel.ends_with("tests.rs");
            let content = fs::read_to_string(&path).unwrap_or_default();
            out.push(RsFile {
                rel,
                module,
                content,
                is_test,
            });
        }
    }
}

fn sources() -> Vec<RsFile> {
    let root = src_root();
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    assert!(
        !out.is_empty(),
        "no .rs files found under {}",
        root.display()
    );
    out
}

/// True for lines that are purely a comment (`//`, `///`, `//!`, block-comment body).
/// Scans operate on real code so doc comments mentioning `tauri::`/`crate::…` for
/// explanation never trigger a false violation.
fn is_comment_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with('*') || t.starts_with("/*")
}

/// First-segment idents from `crate::<ident>` references (covers `use` + inline paths),
/// ignoring comment lines.
fn crate_refs(content: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for line in content.lines().filter(|l| !is_comment_line(l)) {
        let mut rest = line;
        while let Some(pos) = rest.find("crate::") {
            rest = &rest[pos + "crate::".len()..];
            let seg: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !seg.is_empty() {
                refs.insert(seg);
            }
        }
    }
    refs
}

/// Report a rule failure listing every offending file (with the first matching line).
fn fail_if_any(rule: &str, desc: &str, violations: &[(String, usize, String)]) {
    if violations.is_empty() {
        return;
    }
    let mut msg = format!("\n{rule} FAILED — {desc}\n");
    for (rel, line, text) in violations {
        msg.push_str(&format!("  src/{rel}:{line}  {}\n", text.trim()));
    }
    msg.push_str(&format!(
        "\nSee docs/architecture-rules.md ({rule}). If this is a deliberate, \n"
    ));
    msg.push_str(
        "documented exception, add it to the rule's allowlist in tests/architecture.rs.\n",
    );
    panic!("{msg}");
}

/// First real-code line (1-indexed) in `content` containing any of `needles`.
/// Comment lines are skipped (see `is_comment_line`).
fn first_hit(content: &str, needles: &[&str]) -> Option<(usize, String)> {
    for (i, line) in content.lines().enumerate() {
        if !is_comment_line(line) && needles.iter().any(|n| line.contains(n)) {
            return Some((i + 1, line.to_string()));
        }
    }
    None
}

// ── Meta: every module must be classified ───────────────────────────────────────────

#[test]
fn every_module_is_classified() {
    let mut unknown: Vec<String> = sources()
        .iter()
        .filter(|f| layer_of(&f.module).is_none())
        .map(|f| format!("{} (from src/{})", f.module, f.rel))
        .collect();
    unknown.sort();
    unknown.dedup();
    assert!(
        unknown.is_empty(),
        "Unclassified top-level module(s): {unknown:?}\n\
         Add each to L0/L1/L2/L3 in tests/architecture.rs AND docs/architecture-rules.md."
    );
}

// ── R1: `#[tauri::command]` only in the shell command surfaces ───────────────────────

fn is_command_surface(rel: &str) -> bool {
    rel.starts_with("commands/") || rel.starts_with("export/commands/") || rel == "updater/mod.rs"
}

#[test]
fn r1_tauri_command_only_in_command_surfaces() {
    let v: Vec<_> = sources()
        .iter()
        .filter(|f| !f.is_test && !is_command_surface(&f.rel))
        .filter_map(|f| {
            first_hit(&f.content, &["#[tauri::command]"]).map(|(l, t)| (f.rel.clone(), l, t))
        })
        .collect();
    fail_if_any(
        "R1",
        "`#[tauri::command]` may only live in commands/**, export/commands/**, or updater/mod.rs",
        &v,
    );
}

// ── R2: no Tauri coupling in L0/L1/L2 (shell-role files exempt) ───────────────────────
// Debt allowlist: modules that currently use `emit`/`AppHandle` for progress streaming
// or resource resolution. Target: inject an emitter/resource port (TODO(arch)).
const R2_ALLOW: &[&str] = &[
    "autopilot_helpers/mod.rs",
    "autopilot_scheduler.rs",
    // Email-watch scheduler (L2, task #23 PR B) — needs `tauri::AppHandle`/
    // `async_runtime::spawn` to run its own background loop, exactly like
    // `autopilot_scheduler.rs` above.
    "email_watch_scheduler.rs",
    // Follow-up reminder sweep (L2) — same shape and same reason as the two
    // schedulers above: `tauri::AppHandle` + `async_runtime::spawn` for its own
    // background loop. Its decision logic (`should_notify`/`due_follow_ups`) is
    // pure and AppHandle-free.
    "reminder_scheduler.rs",
    "cover_letter/research/mod.rs",
    "documents/mod.rs",
    "pipeline/mod.rs",
    "platform/config.rs", // sole owner: resolves the data dir from the AppHandle at bootstrap
    "platform/accent_watcher.rs", // Windows live-accent watcher: holds the AppHandle + emits SYSTEM_ACCENT_CHANGED from the WinRT ColorValuesChanged callback (bootstrap shell-reach, like platform/config.rs). TODO(arch): inject an emitter port.
];

const TAURI_MARKERS: &[&str] = &["tauri::", "tauri_plugin", "AppHandle", ".emit("];

fn r2_in_scope(f: &RsFile) -> bool {
    // Skip tests, the shell layer (L3), and the export command surface (shell-role code
    // physically nested under the L1 `export/` tree).
    !f.is_test && layer_of(&f.module) != Some(3) && !f.rel.starts_with("export/commands/")
}

#[test]
fn r2_no_tauri_in_lower_layers() {
    let v: Vec<_> = sources()
        .iter()
        .filter(|f| r2_in_scope(f) && !R2_ALLOW.contains(&f.rel.as_str()))
        .filter_map(|f| first_hit(&f.content, TAURI_MARKERS).map(|(l, t)| (f.rel.clone(), l, t)))
        .collect();
    fail_if_any(
        "R2",
        "Tauri types (tauri::/AppHandle/.emit) must not appear below the shell layer",
        &v,
    );
}

#[test]
fn r2_allowlist_has_no_dead_entries() {
    let files = sources();
    let mut stale = Vec::new();
    for &rel in R2_ALLOW {
        let still_needed = files
            .iter()
            .find(|f| f.rel == rel)
            .map(|f| first_hit(&f.content, TAURI_MARKERS).is_some())
            .unwrap_or(false);
        if !still_needed {
            stale.push(rel);
        }
    }
    assert!(
        stale.is_empty(),
        "R2 allowlist entries no longer needed (remove them): {stale:?}"
    );
}

// ── R3: `rusqlite::` only in the DB handle + per-domain stores ───────────────────────
const R3_ALLOW: &[&str] = &[
    "db.rs",    // sole owner of the SQLite handle
    "error.rs", // From<rusqlite::Error> conversion
    "ai_generations/mod.rs",
    "spend/mod.rs",
    "applications/mod.rs",
    // Same store, split only to stay under R8's LOC cap: the follow-up reminder
    // read + its atomic claim. Persistence still lives entirely inside the
    // `applications` domain store — see applications::reminders.
    "applications/reminders.rs",
    // Same store, same reason: the legacy `ai_generations` backfill + its
    // durable one-shot marker (own doc on `ApplicationStore::
    // backfill_from_generations`). Persistence still lives entirely inside
    // the `applications` domain store, on the SAME connection.
    "applications/migrations.rs",
    "documents/mod.rs",
    // Same store, split only to stay under R8's LOC cap: the
    // `repair_pre_pdf_text_string_mojibake` migration body. Persistence
    // still lives entirely inside the `documents` domain store.
    "documents/mojibake_repair.rs",
    // Same store, same reason: the connection-bound SQL of the hot match path,
    // split out so `documents/mod.rs` stays under R8's LOC cap. Every query it
    // holds was moved verbatim from that file.
    "documents/sql.rs",
    "job_preferences/mod.rs",
    "contact_profile/mod.rs",
    "ai_config/mod.rs",
    // Same store, split for COHESION rather than for the LOC cap (unlike the
    // `documents`/`applications` splits above — `ai_config/mod.rs` has ample
    // room): the per-stage override table's SQL, its validation and its stage
    // vocabulary are one subject, and interleaving them with the active-provider
    // config would make neither readable. Persistence still lives entirely
    // inside the `ai_config` domain store, on the SAME connection.
    "ai_config/stage_overrides.rs",
    "referrals/mod.rs",
    "dedup/mod.rs",
    "discovered/mod.rs",
    "email_watch/mod.rs",
    "jobs/mod.rs",
    "pipeline/cache/mod.rs",
    // The pipeline/agent run store: its own `pipeline_runs.db` (ADR-022 shape —
    // `db::open` + position-indexed migrations), backed up via `DataStore` and
    // wiped via `Resettable`, exactly like the per-domain stores above. It sits
    // under `pipeline/` rather than at the top level because `kind` — not a
    // separate module — is what separates a résumé run from an agent run, so a
    // sibling store would have been a second copy of the same two tables.
    "pipeline/runs/mod.rs",
    // Reads the installed browser's EXTERNAL Cookies SQLite (read-only, copied to
    // temp) for session import — not our app DB, so it has no domain store. R3
    // confines OUR persistence; reading a foreign SQLite legitimately needs
    // rusqlite at the read site. See scraping::board_login::import.
    "scraping/board_login/import.rs",
];

#[test]
fn r3_rusqlite_only_in_stores() {
    let v: Vec<_> = sources()
        .iter()
        .filter(|f| !f.is_test && !R3_ALLOW.contains(&f.rel.as_str()))
        .filter_map(|f| first_hit(&f.content, &["rusqlite::"]).map(|(l, t)| (f.rel.clone(), l, t)))
        .collect();
    fail_if_any(
        "R3",
        "`rusqlite::` must be confined to db.rs/error.rs and per-domain stores",
        &v,
    );
}

// ── R4: env access only in `platform/**` ─────────────────────────────────────────────
// Env access is fully centralized in platform::config (ollama_host, env_override,
// extension_dev_origins, data_dir), so no non-platform source needs an allowlist entry.
const R4_ALLOW: &[&str] = &[];

#[test]
fn r4_env_access_only_in_platform() {
    let v: Vec<_> = sources()
        .iter()
        .filter(|f| !f.is_test && f.module != "platform" && !R4_ALLOW.contains(&f.rel.as_str()))
        .filter_map(|f| {
            first_hit(&f.content, &["std::env::var", "AJH_DATA_DIR"])
                .map(|(l, t)| (f.rel.clone(), l, t))
        })
        .collect();
    fail_if_any(
        "R4",
        "`std::env::var`/`AJH_DATA_DIR` must only be read inside platform/**",
        &v,
    );
}

// ── R5: `reqwest::Client` construction only in `net/http.rs` ──────────────────────────

#[test]
fn r5_reqwest_client_only_in_net_http() {
    let v: Vec<_> = sources()
        .iter()
        .filter(|f| !f.is_test && f.rel != "net/http.rs")
        .filter_map(|f| {
            first_hit(
                &f.content,
                &["reqwest::Client::new(", "reqwest::Client::builder("],
            )
            .map(|(l, t)| (f.rel.clone(), l, t))
        })
        .collect();
    fail_if_any(
        "R5",
        "construct reqwest clients only via net::http (shared()/build_client())",
        &v,
    );
}

// ── R6: no stringly-typed `Result<_, String>` outside `error.rs` ─────────────────────

#[test]
fn r6_no_stringly_result() {
    let mut v = Vec::new();
    for f in sources()
        .iter()
        .filter(|f| !f.is_test && f.rel != "error.rs")
    {
        for (i, line) in f.content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue; // ignore doc/comment mentions
            }
            if line.contains("Result<") && line.contains(", String>") {
                v.push((f.rel.clone(), i + 1, line.to_string()));
                break;
            }
        }
    }
    fail_if_any(
        "R6",
        "use AppResult/AppError instead of Result<_, String> for fallible internals",
        &v,
    );
}

// ── R7: no upward layer imports (the only blessed exceptions are W-1 + W-9) ───────────
// (source_module, target_module) pairs allowed to point "up". See docs/architecture-rules.md.
const R7_ALLOW: &[(&str, &str)] = &[
    // W-9: error's From<DomainError> impls reference these domain error enums.
    ("error", "extraction"),
    // W-1: ai_provider lives under commands/ today; consumers reach up until it is
    // relocated to a top-level module. autopilot_scheduler invokes the autopilot command.
    ("pipeline", "commands"),
    // …the GENERATED cross-language constants: the `pipeline:stage`
    // sectionKey grammar (`is_pipeline_section_key`), the depth vocabulary, and
    // the run-deadline terms. Pure compile-time `&str`/`usize` literals emitted
    // by `pnpm gen:ipc`, exactly like the `scraping -> ipc_contracts` edge
    // below. Same TODO(arch): host the cross-language consts in an L0 module.
    ("pipeline", "ipc_contracts"),
    ("documents", "commands"),
    // ai_config (L1) reads ProviderId/validate_model from commands::ai_provider,
    // exactly like documents::embed — same W-1 exception until ai_provider is
    // relocated out of commands/.
    ("ai_config", "commands"),
    // …and the GENERATED stage vocabulary (`ipc_contracts::events::PIPELINE_STAGES`),
    // which is what makes an `ai_stage_overrides` row's `stage` a closed set at
    // WRITE and IMPORT time rather than only at the command boundary. A pure
    // compile-time `&[&str]` emitted by `pnpm gen:ipc` — identical to the
    // `scraping -> ipc_contracts` and `pipeline -> ipc_contracts` edges below/above,
    // and carrying the same TODO(arch): host the cross-language consts in an L0
    // module and all three clear.
    ("ai_config", "ipc_contracts"),
    ("postings", "commands"),
    ("autopilot_scheduler", "commands"),
    // Centralized event emit: autopilot_helpers (L2) streams scrape progress via
    // the L3 `events` helper (crate::events::emit_event + channel consts), the same
    // shell-reach it already has for `commands`. R2 likewise allowlists this file.
    ("autopilot_helpers", "events"),
    // accent_watcher (L0 platform) emits via the L3 events helper; same shell-reach as
    // autopilot_helpers->events. TODO(arch): emitter port.
    ("platform", "events"),
    // The aggregator (L1 scraping) reads the generated credential-slot consts from
    // ipc_contracts::provider_slots — pure compile-time `&str` literals (the single
    // cross-language source of truth, like the L3 events channel consts that L0/L2
    // already reach up for). No runtime/layer coupling. TODO(arch): host the
    // cross-language consts in an L0 module so this exception clears.
    ("scraping", "ipc_contracts"),
    // Email-watch scheduler (L2, task #23 PR B) invokes
    // `commands::notifications::push_and_notify` to deliver a match — the
    // same upward shell-reach `autopilot_scheduler` has for
    // `commands::autopilot::autopilot_run`, deliberately kept OFF the L1
    // `email_watch` store (see that module's L1 comment above).
    ("email_watch_scheduler", "commands"),
    // Same call site also builds the `NewNotification`/`NotificationRoute`
    // payload types `push_and_notify` takes — both live in the L3
    // `notifications` module (the persisted Notification Center store).
    ("email_watch_scheduler", "notifications"),
    // Follow-up reminder sweep (L2): identical shell-reach to the email-watch
    // scheduler above — `commands::notifications::push_and_notify` plus the
    // `NewNotification`/`NotificationRoute` payload types — deliberately kept
    // OFF the L1 `applications` store.
    ("reminder_scheduler", "commands"),
    ("reminder_scheduler", "notifications"),
];

#[test]
fn r7_no_upward_layer_imports() {
    let mut v = Vec::new();
    for f in sources().iter().filter(|f| !f.is_test) {
        let Some(src_layer) = layer_of(&f.module) else {
            continue;
        };
        for dep in crate_refs(&f.content) {
            if dep == f.module {
                continue;
            }
            let Some(dep_layer) = layer_of(&dep) else {
                continue;
            };
            if dep_layer > src_layer && !R7_ALLOW.contains(&(f.module.as_str(), dep.as_str())) {
                let needle = format!("crate::{dep}");
                let (line, text) =
                    first_hit(&f.content, &[needle.as_str()]).unwrap_or((0, dep.clone()));
                v.push((
                    f.rel.clone(),
                    line,
                    format!(
                        "L{src_layer} {} -> L{dep_layer} {dep}: {}",
                        f.module,
                        text.trim()
                    ),
                ));
            }
        }
    }
    v.sort();
    v.dedup();
    fail_if_any(
        "R7",
        "a lower layer must not depend on a higher one (no upward crate:: imports)",
        &v,
    );
}

#[test]
fn r7_allowlist_has_no_dead_entries() {
    let files = sources();
    let stale: Vec<_> = R7_ALLOW
        .iter()
        .filter(|(src, dst)| {
            !files
                .iter()
                .filter(|f| !f.is_test && f.module == *src)
                .any(|f| crate_refs(&f.content).contains(*dst))
        })
        .collect();
    assert!(
        stale.is_empty(),
        "R7 allowlist edges no longer present (remove them): {stale:?}"
    );
}

// ── R8: oversized-module watch (hard cap prevents new mega-files) ────────────────────
const HARD_CAP_LOC: usize = 1400; // current ceiling: extension_bridge/mod.rs (~1398) — split it before growing it
const SOFT_LOC: usize = 600;

#[test]
fn r8_no_oversized_modules() {
    let mut over_hard = Vec::new();
    let mut watch = Vec::new();
    for f in sources().iter().filter(|f| !f.is_test) {
        let loc = f.content.lines().count();
        if loc > HARD_CAP_LOC {
            over_hard.push((
                f.rel.clone(),
                loc,
                format!("{loc} LOC > hard cap {HARD_CAP_LOC}"),
            ));
        } else if loc > SOFT_LOC {
            watch.push((f.rel.clone(), loc));
        }
    }
    watch.sort_by_key(|&(_, loc)| std::cmp::Reverse(loc));
    if !watch.is_empty() {
        eprintln!("R8 watchlist (>{SOFT_LOC} LOC — split candidates, not a failure):");
        for (rel, loc) in &watch {
            eprintln!("  src/{rel}: {loc}");
        }
    }
    fail_if_any(
        "R8",
        "module exceeds the hard LOC cap — split it before it grows further",
        &over_hard,
    );
}

// ── R15: no `.display()` inside a `log::*!`/`tracing::*!` call ───────────────────────
// `.display()` (and equivalent path formatting) renders the OS-native absolute path —
// under the user's home directory on every platform — directly into a log line. Those
// lines land in `crashes.log` and the diagnostics bundle a user might send us, so this is
// a privacy leak, not a style nit (AGENTS.md: never output an absolute path anywhere,
// logs explicitly included).
//
// A `.display()` OUTSIDE a log/tracing macro — e.g. building an `AppError` message the
// renderer shows the user about their OWN file — is a different, legitimate case and is
// deliberately not scanned here; only the log-macro argument span is in scope.
const R15_ALLOW: &[&str] = &[];

/// Both the fully-qualified call form and the bare one. The bare markers are NOT
/// redundant: `extraction/{mod,pdf,registry}.rs` really do `use tracing::warn;`
/// and then call `warn!(…)`, which a qualified-only marker list cannot see — so
/// the rule silently exempted three modules. A bare marker also matches the
/// qualified form (`log::warn!(` contains `warn!(`), which is harmless: the
/// span scan below is idempotent per line. Over-matching a lookalike macro
/// (`my_error!(`) is likewise harmless — a `.display()` inside ANY macro
/// argument that reaches a log sink is the leak this rule exists to stop.
const LOG_MACRO_MARKERS: &[&str] = &["error!(", "warn!(", "info!(", "debug!(", "trace!("];

/// The lines spanning one macro call starting at `lines[start]`, up to and including its
/// closing `);`. Bounded so a malformed/unusually long call can't scan the rest of the
/// file.
///
/// Relies on this codebase's consistent `rustfmt` output, with one correction: "the first
/// line ending in `);`" is NOT always the outer call's close. A macro argument containing a
/// block or closure can hold a *statement* that ends in `);` at a DEEPER indent, e.g.
///
/// ```ignore
/// log::info!("{}", {
///     let p = compute_path();      // <- ends in `);`, but is not the close
///     p.display()                  // <- the real leak, one line further down
/// });
/// ```
///
/// Ending the span at that inner statement hid the leak entirely. So the close must also be
/// at an indent no deeper than the macro's own line — which is exactly how rustfmt formats
/// the outer `);`. Falls back to the first `);` at any indent if none matches, so a call
/// rustfmt has not touched still gets a bounded (if imperfect) span rather than none.
fn macro_call_span<'a>(lines: &'a [&'a str], start: usize) -> &'a [&'a str] {
    const MAX_SPAN: usize = 20;
    let indent = |l: &str| l.len() - l.trim_start().len();
    let open_indent = indent(lines[start]);
    let last = start + MAX_SPAN.min(lines.len() - start) - 1;
    let closes = |i: usize| lines[i].trim_end().ends_with(");");
    let end = (start..=last)
        .find(|&i| closes(i) && indent(lines[i]) <= open_indent)
        .or_else(|| (start..=last).find(|&i| closes(i)))
        .unwrap_or(start);
    &lines[start..=end]
}

/// `(1-indexed line, matched line text)` for every `.display()` found inside a
/// `log::*!`/`tracing::*!` call's argument span in `content`. Pure text-scan, no `sources()`
/// dependency, so it is directly unit-testable against synthetic snippets below.
fn find_display_leaks(content: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut hits = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !is_comment_line(lines[i]) && LOG_MACRO_MARKERS.iter().any(|m| lines[i].contains(m)) {
            let span = macro_call_span(&lines, i);
            if let Some(offset) = span
                .iter()
                .position(|l| !is_comment_line(l) && l.contains(".display()"))
            {
                hits.push((i + 1 + offset, span[offset].to_string()));
            }
            i += span.len();
            continue;
        }
        i += 1;
    }
    hits
}

#[test]
fn r15_no_display_in_log_macros() {
    let mut v = Vec::new();
    for f in sources()
        .iter()
        .filter(|f| !f.is_test && !R15_ALLOW.contains(&f.rel.as_str()))
    {
        for (line, text) in find_display_leaks(&f.content) {
            v.push((f.rel.clone(), line, text));
        }
    }
    fail_if_any(
        "R15",
        "`.display()` must not appear inside a log::*!/tracing::*! call — it leaks an \
         absolute, username-bearing path into logs and diagnostics bundles; log a file \
         name (`.file_name()`) or a stable caller-supplied label instead",
        &v,
    );
}

#[cfg(test)]
mod r15_detection_logic {
    use super::find_display_leaks;

    /// True positive: the exact multi-line shape both real violations this rule guards
    /// against had (the `.display()` argument on a line separate from the macro-opening
    /// line) — a same-line-only scan would have missed both.
    #[test]
    fn flags_display_on_a_later_line_of_a_multiline_log_call() {
        let src = r#"
fn f() {
    log::error!(
        "[postings] failed to write {}: {e}",
        tmp.display()
    );
}
"#;
        let hits = find_display_leaks(src);
        assert_eq!(hits.len(), 1, "must flag exactly one leak; got {hits:?}");
        assert!(hits[0].1.contains(".display()"));
    }

    /// True positive: a single-line call is also covered (opening line == closing line).
    #[test]
    fn flags_display_on_a_single_line_log_call() {
        let src = r#"log::warn!("mkdir {} failed: {e}", parent.display());"#;
        let hits = find_display_leaks(src);
        assert_eq!(hits.len(), 1, "single-line call must still be flagged");
    }

    /// True positive: a BARE macro call (`use tracing::warn;` then `warn!(…)`), which the
    /// original fully-qualified-only marker list could not see at all. Three real modules
    /// (`extraction/{mod,pdf,registry}.rs`) import the macro exactly this way, so the rule
    /// silently exempted them.
    #[test]
    fn flags_display_in_a_bare_imported_macro_call() {
        let src = r#"
use tracing::warn;
fn f() {
    warn!("could not read {}: {e}", path.display());
}
"#;
        let hits = find_display_leaks(src);
        assert_eq!(
            hits.len(),
            1,
            "a bare `warn!(…)` call must be flagged; got {hits:?}"
        );
    }

    /// True positive: an interposed statement ending in `);` at a DEEPER indent must not
    /// end the span early. This is the documented risk in `macro_call_span`'s heuristic —
    /// before the indent guard, the scan stopped at `compute_path();` and the real
    /// `.display()` one line further down was never seen.
    #[test]
    fn flags_display_after_an_inner_statement_that_ends_in_a_paren_semicolon() {
        let src = r#"
fn f() {
    log::info!("{}", {
        let p = compute_path();
        p.display()
    });
}
"#;
        let hits = find_display_leaks(src);
        assert_eq!(
            hits.len(),
            1,
            "an inner `);` must not truncate the span before the leak; got {hits:?}"
        );
        assert!(hits[0].1.contains(".display()"));
    }

    /// True negative: a `.display()` used to build a user-facing `AppError` — never
    /// wrapped in a log/tracing macro — is the documented legitimate case and must not
    /// be flagged.
    #[test]
    fn does_not_flag_display_outside_a_log_macro() {
        let src = r#"
fn f() -> AppResult<()> {
    Err(AppError::Storage(format!("could not read {}", path.display())))
}
"#;
        assert!(
            find_display_leaks(src).is_empty(),
            "a user-facing AppError is not a log call and must not be flagged"
        );
    }

    /// True negative: a `.display()` mentioned only in a comment (e.g. explaining why it
    /// was removed) must not trip the rule.
    #[test]
    fn does_not_flag_display_inside_a_comment() {
        let src = r#"
fn f() {
    log::error!(
        // a `.display()` here would leak the path
        "[postings] save skipped"
    );
}
"#;
        assert!(find_display_leaks(src).is_empty());
    }
}
