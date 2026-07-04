//! Architecture boundary tests — the machine-enforced contract for the Rust core.
//!
//! Derived from `docs/architecture-analysis.md` (discovery) and codified in
//! `docs/architecture-rules.md` (the layer model + rule IDs R1–R8). This is a
//! **standalone integration test**: it uses only `std` and scans the source tree
//! under `CARGO_MANIFEST_DIR/src` as text — it does not link the crate's internals
//! (same pattern as `tests/eval.rs`). The crate is a thin binary (`main.rs`) over a
//! library (`lib.rs`, which holds the app + the Tauri builder); both are L3 shell.
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
];
const L1: &[&str] = &[
    "scraping",
    "extraction",
    "export",
    "documents",
    "jobs",
    "postings",
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
];
const L2: &[&str] = &[
    "pipeline",
    "cover_letter",
    "salary_research",
    "autopilot",
    "autopilot_scheduler",
    "autopilot_helpers",
    "recommend",
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
    "cover_letter/research/mod.rs",
    "salary_research/mod.rs",
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
    "applications/mod.rs",
    "documents/mod.rs",
    "job_preferences/mod.rs",
    "contact_profile/mod.rs",
    "referrals/mod.rs",
    "jobs/mod.rs",
    "pipeline/cache/mod.rs",
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
    ("documents", "commands"),
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
const HARD_CAP_LOC: usize = 1400; // current ceiling: export/pdf_renderer/mod.rs (~1343)
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
