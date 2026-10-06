//! Source guard (#1348): the crate has no mock app, so deleting the strip at a
//! seam fails no behavioural test. This scans the code instead.
//!
//! Every `self.provider.<method>(` in non-test `src/pipeline/**` must sit
//! inside a `strip_secrets(` call (same statement, comments removed first) or
//! be an allowlisted non-IO accessor. A new unwrapped call, or a new file under
//! `src/pipeline`, fails here until it is wrapped or deliberately allowlisted.

use std::path::{Path, PathBuf};

/// Methods on `self.provider` that perform no provider IO (identity, static
/// capability tables, or handing the provider to a callee that is itself
/// checked below).
const NON_IO: &[&str] = &["id", "effort_levels", "has_native_search", "as_ref"];

/// Free functions that receive `self.provider.as_ref()` and do the IO; their
/// result must be wrapped too.
const IO_CALLEES: &[&str] = &[
    "fetch_company_brief(",
    "searched_research_salary(",
    "searched_research_answer(",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                rust_files(&path, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(path);
        }
    }
}

/// Drop `//` comments, then ALL whitespace, so rustfmt's line breaks
/// (a call split after `self.provider`) cannot hide it, and prose can never
/// satisfy (or trip) the scan.
fn without_comments(src: &str) -> String {
    src.lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .flat_map(str::split_whitespace)
        .collect()
}

/// Whether the expression containing byte `at` is an argument of `strip_secrets(`:
/// walk back through enclosing parens until a statement/block boundary.
fn is_wrapped(src: &str, at: usize) -> bool {
    let mut depth = 0usize;
    for (i, c) in src[..at].char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' if depth > 0 => depth -= 1,
            '(' if src[..i].ends_with("strip_secrets") => return true,
            ';' | '{' | '}' if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

fn calls<'a>(src: &'a str, needle: &'a str) -> impl Iterator<Item = usize> + 'a {
    src.match_indices(needle).map(|(i, _)| i)
}

#[test]
fn every_provider_call_in_the_pipeline_is_inside_strip_secrets() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pipeline");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(files.len() > 5, "scan found too few files: {files:?}");

    let mut wrapped_seen = 0;
    for file in files {
        let src = without_comments(&std::fs::read_to_string(&file).unwrap());
        for at in calls(&src, "self.provider.") {
            let rest = &src[at + "self.provider.".len()..];
            let method: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !rest[method.len()..].starts_with('(') || NON_IO.contains(&method.as_str()) {
                continue;
            }
            assert!(
                is_wrapped(&src, at),
                "{}: self.provider.{method}( is not inside strip_secrets(",
                file.display()
            );
            wrapped_seen += 1;
        }
        for callee in IO_CALLEES {
            for at in calls(&src, callee) {
                if src[..at].ends_with("fn") {
                    continue;
                }
                assert!(
                    is_wrapped(&src, at),
                    "{}: {callee} is not inside strip_secrets(",
                    file.display()
                );
                wrapped_seen += 1;
            }
        }
    }
    // chat_stream x2, complete_with_usage, chat_with_tools, complete_structured,
    // research_salary, research_answer, plus the three IO callees.
    assert!(
        wrapped_seen >= 10,
        "guard matched only {wrapped_seen} sites"
    );
}

/// `embed_text` strips exactly the `embed_adaptive` result — after the halving
/// retry, which runs inside `embed_adaptive`.
#[test]
fn embed_text_strips_the_embed_adaptive_result() {
    let src = without_comments(include_str!("../../embeddings.rs"));
    let call = src
        .find("letresult=embed_adaptive(")
        .expect("embed_adaptive result binding");
    let strip = src[call..]
        .find("letvalues=result.map_err(")
        .expect("result is mapped through the strip");
    let stmt = &src[call + strip..];
    let end = stmt.find("})?;").expect("end of map_err");
    assert!(stmt[..end].contains("strip_provider_secrets("));
}
