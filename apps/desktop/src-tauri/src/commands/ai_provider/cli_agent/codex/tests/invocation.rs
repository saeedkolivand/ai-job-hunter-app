//! Building the `codex exec` invocation (argv/isolation/effort) and parsing
//! `codex debug models`'s live-discovery output.

use super::super::*;

// ── `codex debug models` (live discovery) ──────────────────────────────────

/// Trimmed real shape from `codex debug models` (Codex CLI 0.144.6) — full
/// entries also carry a `base_instructions` block, omitted here.
const DEBUG_MODELS_JSON: &str = r#"{"models":[
    {"slug":"gpt-reserve","display_name":"GPT-Reserve","visibility":"hide"},
    {"slug":"gpt-5.6-terra","display_name":"GPT-5.6-Terra","visibility":"list"},
    {"slug":"gpt-5.6-luna","display_name":"GPT-5.6-Luna","visibility":"list"},
    {"slug":"gpt-5.5","display_name":"GPT-5.5","visibility":"list"},
    {"slug":"codex-auto-review","display_name":"Codex Auto Review","visibility":"hide"}
]}"#;

#[test]
fn parse_debug_models_keeps_only_list_visibility_entries() {
    let entries = parse_debug_models(DEBUG_MODELS_JSON).unwrap();
    assert_eq!(
        entries,
        vec![
            json!({ "name": "gpt-5.6-terra", "displayName": "GPT-5.6-Terra" }),
            json!({ "name": "gpt-5.6-luna", "displayName": "GPT-5.6-Luna" }),
            json!({ "name": "gpt-5.5", "displayName": "GPT-5.5" }),
        ]
    );
}

#[test]
fn parse_debug_models_none_on_malformed_json() {
    assert_eq!(parse_debug_models("not json"), None);
    assert_eq!(parse_debug_models(r#"{"nope":true}"#), None);
}

/// Every real entry happened to be `"hide"` (or the catalog is genuinely
/// empty) — `None`, same as a parse failure, so the caller falls back.
#[test]
fn parse_debug_models_none_when_nothing_is_listable() {
    let out = r#"{"models":[{"slug":"x","display_name":"X","visibility":"hide"}]}"#;
    assert_eq!(parse_debug_models(out), None);
}

/// One row missing a required field (`DebugModel` has no default for
/// `display_name`) must be skipped, not abort the whole parse — the
/// `filter_map(...ok())` behind it silently drops just that row.
#[test]
fn parse_debug_models_skips_a_malformed_row_but_keeps_the_rest() {
    let out = r#"{"models":[
        {"slug":"broken","visibility":"list"},
        {"slug":"gpt-5.5","display_name":"GPT-5.5","visibility":"list"}
    ]}"#;
    assert_eq!(
        parse_debug_models(out).unwrap(),
        vec![json!({ "name": "gpt-5.5", "displayName": "GPT-5.5" })]
    );
}

#[test]
fn exec_args_include_sandbox_and_model() {
    let inv = CodexAgent.stream_invocation("o4-mini", "", None);
    // Prompt is delivered on stdin, never as a positional argv element — so no
    // untrusted JD text can reach `cmd.exe` on Windows (CVE-2024-24576).
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--sandbox" && w[1] == "read-only"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--model" && w[1] == "o4-mini"));
    // No effort → no reasoning-effort override.
    assert!(!inv
        .args
        .iter()
        .any(|a| a.starts_with("model_reasoning_effort=")));
    // Runs outside a git repo (temp cwd) — the check must be skipped.
    assert!(inv.args.iter().any(|a| a == "--skip-git-repo-check"));
}

#[test]
fn argv_is_only_static_flags_never_the_prompt() {
    // The full argv is a fixed, trusted set of exec flags (+ resolved model) —
    // it never contains prompt/JD-derived text. That is the property that clears
    // the command-injection CRITICAL: with `Stdin` delivery the harness pipes the
    // prompt to the child, so nothing untrusted ever reaches argv / `cmd.exe`.
    let inv = CodexAgent.stream_invocation("o4-mini", "system text here", None);
    assert_eq!(
        inv.args,
        vec![
            "exec",
            "--json",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "-c",
            "features.hooks=false",
            "-c",
            "features.plugins=false",
            "-c",
            "features.remote_plugin=false",
            "-c",
            "features.apps=false",
            "-c",
            "features.memories=false",
            "-c",
            "features.goals=false",
            "-c",
            "features.multi_agent=false",
            "-c",
            "features.workspace_dependencies=false",
            "-c",
            "project_doc_max_bytes=0",
            "--model",
            "o4-mini",
        ]
    );
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
}

/// Isolation flags must use the `-c key=value` override form — never `--disable`,
/// which hard-errors on an unknown feature name and would break every call on out-
/// of-band CLI drift. Also assert the full disable set is present.
#[test]
fn isolation_flags_use_the_c_form_and_never_disable() {
    let inv = CodexAgent.stream_invocation("o4-mini", "", None);
    assert!(!inv.args.iter().any(|a| a == "--disable"));
    for flag in ["--ignore-user-config", "--ignore-rules", "--ephemeral"] {
        assert!(inv.args.iter().any(|a| a == flag), "missing {flag}");
    }
    for key in [
        "features.hooks=false",
        "features.plugins=false",
        "features.remote_plugin=false",
        "features.apps=false",
        "features.memories=false",
        "features.goals=false",
        "features.multi_agent=false",
        "features.workspace_dependencies=false",
        "project_doc_max_bytes=0",
    ] {
        assert!(
            inv.args.windows(2).any(|w| w[0] == "-c" && w[1] == key),
            "missing {key}"
        );
    }
}

#[test]
fn effort_adds_reasoning_config_override() {
    let inv = CodexAgent.stream_invocation("o4-mini", "", Some("high"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "-c" && w[1] == "model_reasoning_effort=high"));
    // Blank effort is treated as none.
    let blank = CodexAgent.stream_invocation("o4-mini", "", Some("  "));
    assert!(!blank
        .args
        .iter()
        .any(|a| a.starts_with("model_reasoning_effort=")));
}

#[test]
fn inlines_system_prompt() {
    assert!(CodexAgent.inline_system());
}
