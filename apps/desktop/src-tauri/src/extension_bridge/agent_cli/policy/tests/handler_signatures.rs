//! The generated catalogue pinned against the Rust `#[tauri::command]` signatures
//! `agent_call::validate` gates dispatch on — every arg NAME and `required` flag, by a
//! byte-scan of the crate's own handler sources, plus that scan's hand-rolled helpers.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// --- TR-03 MEDIUM (test-author round) -------------------------------------------------------
//
// Nothing before this pinned the generated `CATALOGUE` against the Rust `#[tauri::command]`
// handler signatures `agent_call::validate::check_input` gates dispatch on. `CATALOGUE` is
// generated from `apps/desktop/src/tauri-client/namespaces/**/*.ts` — a DIFFERENT source of
// truth from the handler being gated. Every other test in this file pins MEMBERSHIP
// (`policy_table_matches_generate_handler_exactly`) or wrapper CLASS
// (`EXPECTED_RESOLVED_WRAPPER_ARGS`/`EXPECTED_UNRESOLVED_WRAPPER_ARGS`); none compares an
// argument's NAME or `required` flag against the handler that actually reads it. Drift there
// silently converts a valid agent call into `invalid_input`, or drops a required-key refusal
// (an `Option<T>` field the catalogue wrongly marks `required: true` would refuse a perfectly
// valid omitted key; the reverse would let a truly-required key through as `None`, panicking or
// misbehaving deeper in the handler).

/// One handler's own non-injected parameter, hand-parsed from its `#[tauri::command]` signature
/// — name already converted to Tauri's wire convention (snake_case Rust ident -> camelCase JSON
/// key) so it compares directly against a [`CatalogueArg::name`].
struct HandlerParam {
    name: String,
    required: bool,
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// `snake_case` -> `camelCase`, matching Tauri's own default wire-key convention (the same
/// conversion `gen-agent-catalogue.ts` relies on when it reads the TS side of the same contract).
fn snake_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Strip `//` line comments from a parameter list — a handler that documents one param inline
/// (e.g. `ai_lookup_salary`'s `country`/`currency`/`effort`) would otherwise have its comment
/// TEXT treated as literal parameter source, corrupting the top-level comma split below (a
/// comment's own prose commas would be read as param separators).
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| line.find("//").map_or(line, |i| &line[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split a parameter list on top-level commas only — a comma nested inside `<...>` (a generic
/// like `Option<ScrapeListFilter>` or `tauri::State<'_, T>`) does not end a param.
fn split_top_level_params(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in src.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&src[start..]);
    parts
}

/// A Tauri-injected parameter (`AppHandle`/`tauri::State<..>`) carries no wire key at all — not
/// a [`HandlerParam`], the same exemption `entry_for`'s own catalogue never lists one for.
fn is_injected_param_type(ty: &str) -> bool {
    let ty = ty.trim();
    ty.contains("AppHandle") || ty.starts_with("State<") || ty.starts_with("tauri::State<")
}

fn parse_param(raw: &str) -> Option<HandlerParam> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let colon = raw.find(':')?;
    let name = raw[..colon].trim();
    let name = name.strip_prefix("mut ").unwrap_or(name).trim();
    let ty = raw[colon + 1..].trim();
    if is_injected_param_type(ty) {
        return None;
    }
    Some(HandlerParam {
        name: snake_to_camel(name.trim_start_matches('_')),
        required: !ty.starts_with("Option<"),
    })
}

/// Byte-scan `text` for every `#[tauri::command]`/`#[command]`-annotated fn and return its
/// `(name, non-injected params)`. No `syn`/regex dependency (neither is a `[dependencies]` of
/// this crate in this shape) — this mirrors `registered_command_paths`'s own hand-rolled
/// `include_str!` extraction above rather than adding one for a test-only need.
fn command_handler_params(text: &str) -> Vec<(String, Vec<HandlerParam>)> {
    const MARKERS: [&str; 2] = ["#[tauri::command]", "#[command]"];
    let mut results = Vec::new();
    let mut pos = 0usize;
    while pos < text.len() {
        let Some((idx, marker)) = MARKERS
            .iter()
            .filter_map(|m| text[pos..].find(m).map(|i| (pos + i, *m)))
            .min_by_key(|(i, _)| *i)
        else {
            break;
        };
        let after = idx + marker.len();
        let Some(fn_rel) = text[after..].find("fn ") else {
            pos = after;
            continue;
        };
        let between = &text[after..after + fn_rel];
        // Only whitespace/`pub`/`pub(...)`/`async` may sit between the marker and `fn ` — guards
        // against this exact literal marker string ever appearing somewhere unrelated to the fn
        // it is meant to annotate.
        if !between
            .split_whitespace()
            .all(|tok| tok == "pub" || tok == "async" || tok.starts_with("pub("))
        {
            pos = after;
            continue;
        }
        let fn_start = after + fn_rel + "fn ".len();
        let name_end = text[fn_start..]
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map_or(text.len(), |i| fn_start + i);
        let name = text[fn_start..name_end].to_string();
        let Some(paren_rel) = text[name_end..].find('(') else {
            pos = name_end;
            continue;
        };
        let paren_start = name_end + paren_rel;
        let bytes = text.as_bytes();
        let mut depth = 0i32;
        let mut close = paren_start;
        for (i, &b) in bytes[paren_start..].iter().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = paren_start + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let params_src = strip_line_comments(&text[paren_start + 1..close]);
        let params = split_top_level_params(&params_src)
            .into_iter()
            .filter_map(parse_param)
            .collect();
        results.push((name, params));
        pos = close + 1;
    }
    results
}

#[test]
fn catalogue_arg_names_and_required_flags_match_every_command_handler_signature() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);

    let mut handlers: HashMap<String, Vec<HandlerParam>> = HashMap::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (name, params) in command_handler_params(&text) {
            handlers.insert(name, params);
        }
    }
    assert!(
        handlers.len() >= 167,
        "expected to find at least the 167 #[tauri::command]/#[command] handlers this crate \
         registers, found {} — the byte-scan above likely drifted from the real signature shape \
         (fix the scan, don't loosen this bound)",
        handlers.len()
    );

    let mut mismatches = Vec::new();
    for entry in super::super::super::catalogue::CATALOGUE.iter() {
        let Some(params) = handlers.get(entry.command) else {
            mismatches.push(format!(
                "{}: catalogued but no #[tauri::command] handler found by this scan",
                entry.command
            ));
            continue;
        };
        let catalogue_names: HashSet<&str> = entry.args.iter().map(|a| a.name).collect();
        let handler_names: HashSet<&str> = params.iter().map(|p| p.name.as_str()).collect();
        if catalogue_names != handler_names {
            mismatches.push(format!(
                "{}: catalogue args {catalogue_names:?} != handler params {handler_names:?}",
                entry.command
            ));
            continue;
        }
        for arg in entry.args {
            let handler = params
                .iter()
                .find(|p| p.name == arg.name)
                .expect("checked above");
            if handler.required != arg.required {
                mismatches.push(format!(
                    "{}.{}: catalogue required={} but the handler param is {}",
                    entry.command,
                    arg.name,
                    arg.required,
                    if handler.required {
                        "non-Option"
                    } else {
                        "Option<..>"
                    }
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "CATALOGUE drifted from its Rust #[tauri::command] handler signature:\n{}",
        mismatches.join("\n")
    );
}
