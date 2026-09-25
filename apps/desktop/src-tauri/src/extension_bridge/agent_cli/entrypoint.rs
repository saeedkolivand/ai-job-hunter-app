//! `ajh-tauri agent <verb>`: argv sentinel dispatch, the `--help` text, the
//! whole-invocation deadline, and the stdout/exit-code translation every reply goes through. Split
//! out of `agent_cli.rs` under R8's LOC cap.

use super::*;
// ── entrypoint + output ─────────────────────────────────────────────────────
/// The exit-2 usage-error body — pulled out as its own pure fn (MINOR fix —
/// security review round 2) so it's directly unit-testable, and so its shape
/// stays byte-for-byte in sync with [`emit_cli_error`]'s: both always carry
/// `resource` (this one `null` — no `Verb` exists yet at the point a usage
/// error is raised), matching the module doc's exit-2 table. `detail` is
/// this branch's own extra, human/agent-useful context, not part of the
/// shape the doc pins.
pub(super) fn usage_error_value(detail: &str) -> Value {
    json!({ "ok": false, "resource": Value::Null, "error": ERR_USAGE, "detail": detail })
}

/// Print a synthesized CLI-level error (exit 2) and return that code. Never
/// echoes a path or a raw I/O error string — only fixed sentinels (see the
/// module doc's exit-code table).
fn emit_cli_error(resource: Option<&str>, sentinel: &str) -> i32 {
    println!(
        "{}",
        json!({ "ok": false, "resource": resource, "error": sentinel })
    );
    2
}
async fn run_verb(verb: Verb) -> i32 {
    let resource = verb.resource_name();
    match query(&verb).await {
        Ok(payload) => {
            println!("{payload}");
            exit_code_for_reply(&verb, &payload)
        }
        Err(sentinel) => emit_cli_error(Some(resource), sentinel),
    }
}

/// The reply's own truth field decides the exit code, and it differs BY
/// TIER (ADR-038 §2/§5): the curated tier keeps a truthful `ok` (0 on
/// `true`, 1 — "the app replied with a refusal" — on `false`). The generic
/// `call` tier never claims `ok`; its `dispatched` means only "did
/// `Webview::on_message` run", so `false` there is normally a REFUSAL BEFORE
/// dispatch (unknown command, wrong effect class, rate-limited) — the SAME
/// class as a usage error, hence exit 2, not 1. ONE `dispatched:false` cause
/// is its own distinct exit code (ADR-038 §4, Phase 3): an `Irreversible`
/// command called with no `--confirm` is `confirmation_required`, which
/// exits 4 rather than 2 — "needs confirmation" is a different outcome from
/// a refusal, never collapsed into it (the payload's own `error` field is
/// what names every cause; this fn only routes the ONE that gets a
/// different process exit code).
pub(super) fn exit_code_for_reply(verb: &Verb, payload: &Value) -> i32 {
    match verb {
        Verb::Call { .. } => {
            if payload.get("error").and_then(Value::as_str)
                == Some(agent_call::ERR_CONFIRMATION_REQUIRED)
            {
                return 4;
            }
            let dispatched = payload
                .get("dispatched")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if dispatched {
                0
            } else {
                2
            }
        }
        _ => {
            let ok = payload.get("ok").and_then(Value::as_bool).unwrap_or(false);
            i32::from(!ok)
        }
    }
}

/// Whether `args`' first token requests help. `-h`/`--help` are checked
/// anywhere the flag would normally sit as the FIRST argument (this CLI has
/// no other flags before a verb); a bare `help` verb is also accepted.
fn is_help_request(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("--help") | Some("-h") | Some("help")
    )
}

/// Whether `args`' first token requests the MCP server mode
/// (`agent mcp [--allow-irreversible]`) — a MODE like `--help`, never a
/// `Verb`/`VERB_TABLE` row (see [`mcp`]'s own module doc for why).
fn is_mcp_mode(args: &[String]) -> bool {
    args.first().map(String::as_str) == Some("mcp")
}

/// Human-readable usage text — derived ENTIRELY from [`VERB_TABLE`] and
/// [`ERROR_SENTINELS`], never a hand-typed second copy of either (see both
/// constants' own docs). Pure, allocation-only: no `AppHandle`, no pointer
/// file, no token, no socket — safe to print with the app not running at all
/// (the owner's hard requirement for `--help`).
fn help_text() -> String {
    let mut out = String::from(
        "ajh-tauri agent <verb> [args]\n\n\
         A thin CLI client over the AI Job Hunter desktop app's loopback bridge.\n\
         The desktop app must already be running for any verb below EXCEPT --help.\n\n\
         VERBS:\n",
    );
    for v in VERB_TABLE {
        out.push_str(&format!("  {:<16}{:<16}{}\n", v.name, v.args, v.returns));
    }
    out.push_str(
        "  --help, -h, help                Show this help and exit (works even if the app is not running).\n\
         \x20\x20mcp [--allow-reversible] [--allow-irreversible]\n\
                                  Run as an MCP (Model Context Protocol) stdio server for Claude \
           Code/Codex; read tier + `commands` only by default, --allow-reversible adds \
           mutating-but-undoable tools, --allow-irreversible adds the rest (implies \
           --allow-reversible). `agent mcp --help` shows its own usage.\n\n\
         EXIT CODES:\n\
         \x20 0   Success — the reply is printed as JSON on stdout.\n\
         \x20 1   The app replied with a refusal (rate-limited, validation, not found, autofill off, ...) \
           — still printed as JSON on stdout.\n\
         \x20 2   No result was delivered — the round trip failed, the usage was invalid, or the \
           app refused/discarded the reply; \"error\" names which, and result_too_large means the \
           command itself may already have run.\n\
         \x20 4   `call` only: an Effect::Irreversible command needs --confirm '<value>' — the \
           reply's \"detail\" names which OTHER read command/resource to read the proof from, \
           and never the value itself (ADR-038 §4).\n\n\
         ERROR SENTINELS this CLI synthesizes itself (the \"error\" field when no reply arrived):\n",
    );
    for (sentinel, meaning) in ERROR_SENTINELS {
        out.push_str(&format!("  {sentinel:<26}{meaning}\n"));
    }
    // The app's OWN refusal names also land in `error` on an exit-2 reply and
    // are deliberately NOT added to the table above: that table is derived
    // from `ERROR_SENTINELS`, this CLI's client-side set, and hand-typing the
    // app-side names here is exactly the second copy that drifts. One
    // sentence + a pointer to where they're defined instead.
    out.push_str(
        "\n\x20\x20App-side refusal names (result_too_large, invalid_cursor, ...) reach that same \
         \"error\" field from the app and are not listed above — they are the variants of \
         agent_call::Refusal.\n",
    );
    out
}

/// Race `fut` (in production, [`run_verb`]'s own future) against `budget` —
/// [`run`]'s outer, WHOLE-INVOCATION deadline (MAJOR fix — security review
/// round 2; see [`INVOCATION_TIMEOUT`]'s own doc for why this exists
/// alongside, not instead of, the per-step timeouts already inside
/// [`super::handshake_client::attempt_port`]/[`super::query::send_agent_query_within`]). `resource` is a plain
/// `&str` rather than a `Verb` so the caller can hand this a `Verb`'s
/// `resource_name()` BEFORE moving the `Verb` itself into `fut` — a `Verb`
/// doesn't survive being consumed by the future this races.
///
/// Generic over `F` (rather than `run_verb`'s own concrete future) so this
/// race's OUTCOME is directly unit-testable against a controllable budget
/// and a controllable inner future, without a live pointer file/token/socket
/// and without waiting out the real [`INVOCATION_TIMEOUT`] — mirrors
/// [`super::query::send_agent_query_within`]'s existing "explicit budget parameter, prod
/// wraps it" pattern one section up.
async fn run_verb_within<F>(resource: &str, budget: Duration, fut: F) -> i32
where
    F: std::future::Future<Output = i32>,
{
    match timeout(budget, fut).await {
        Ok(code) => code,
        Err(_) => emit_cli_error(Some(resource), ERR_TIMEOUT),
    }
}

/// `ajh-tauri agent <verb>` entrypoint. `args` excludes the program name AND
/// the `agent` sentinel itself. Called from `lib::run_agent_cli_if_invoked`,
/// itself called from `main()` BELOW the native-host short-circuit and ABOVE
/// `ajh_tauri::run()` — see that function's doc for why the ordering matters.
/// Builds its OWN current-thread Tokio runtime (mirrors
/// [`super::super::native_host::run`]): this path runs before Tauri boots, so there
/// is no ambient reactor. Never panics out.
pub fn run(args: &[String]) -> i32 {
    // MUST run first — `--help` is the single most likely command a human
    // types interactively on Windows, precisely the NULL-stdout case this
    // probe exists for (`platform::windows_console`'s own doc).
    crate::platform::windows_console::ensure_console_output();

    if is_help_request(args) {
        // No pointer, no token, no socket, no network — pure local text, per
        // the owner's requirement that `--help` work with the app NOT
        // running.
        println!("{}", help_text());
        return 0;
    }
    if is_mcp_mode(args) {
        // A MODE, not a `Verb` — intercepted before `parse_verb` exactly
        // like `--help` above, so it forces no nonsense `wire_type`/
        // `payload` match arms and touches neither `VERB_TABLE` nor its own
        // drift tests (see `mcp`'s own module doc).
        return mcp::run(&args[1..]);
    }
    if args.is_empty() {
        // A bare `ajh-tauri agent` is far more likely a human looking for
        // guidance than a scripted caller depending on today's terse JSON
        // usage error, so it gets the SAME help text `--help` prints — to
        // stderr (this is still an error exit), never stdout, so a script
        // that only reads stdout for the JSON reply sees nothing new.
        eprintln!("{}", help_text());
        return 2;
    }

    let verb = match parse_verb(args) {
        Ok(v) => v,
        Err(e) => {
            // See `usage_error_value`'s doc (MINOR fix — security review
            // round 2): this branch runs before a `Verb` exists, but the
            // exit-2 shape must still carry `resource` (null here), the same
            // as every other exit-2 reply on this surface.
            println!("{}", usage_error_value(&e.to_string()));
            return 2;
        }
    };

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return emit_cli_error(Some(verb.resource_name()), ERR_RUNTIME_UNAVAILABLE),
    };
    // `resource_name()` is read BEFORE `verb` moves into `run_verb` below —
    // see `run_verb_within`'s own doc.
    let resource = verb.resource_name();
    rt.block_on(run_verb_within(
        resource,
        INVOCATION_TIMEOUT,
        run_verb(verb),
    ))
}

#[cfg(test)]
mod tests;
