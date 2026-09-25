//! `agent mcp [flags]` — argv parsing, `--help`, and the entrypoint that builds the server and
//! its dispatch closure and then hands both to whichever transport `--http` selected.

use super::*;

/// `agent mcp [--allow-reversible] [--allow-irreversible] [--http <port>] [--help]` argv — any
/// subset of the flags, in any order; `--help`/`-h`/`help` anywhere short-circuits everything
/// else. Anything not in this set is a hard failure (MUST FIX — security review round 2: argv is
/// the only path to any gate, env vars are never consulted, and this parser must never grow a
/// fuzzy/prefix match that could nudge a typo into an elevated launch).
///
/// `--http` takes exactly one following token, parsed as a bare `u16` — nothing else is a valid
/// shape for it. This is also the WHOLE non-loopback-bind refusal (issue #1173): there is no flag
/// that can express a host or address at all, so `--http 0.0.0.0:9000`, `--http=9000`, and a bare
/// `--http` with nothing after it are all a parse-time `Err(())` here, before a socket is ever
/// touched — never a runtime validation `http::run` has to perform on a value that already parsed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct LaunchArgs {
    pub(super) help: bool,
    pub(super) allow_reversible: bool,
    pub(super) allow_irreversible: bool,
    pub(super) http: Option<u16>,
}

pub(super) fn parse_launch_args(args: &[String]) -> Result<LaunchArgs, ()> {
    let mut parsed = LaunchArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" | "help" => parsed.help = true,
            "--allow-reversible" => parsed.allow_reversible = true,
            "--allow-irreversible" => parsed.allow_irreversible = true,
            "--http" => {
                let port = args.get(i + 1).ok_or(())?.parse::<u16>().map_err(|_| ())?;
                parsed.http = Some(port);
                i += 1;
            }
            _ => return Err(()),
        }
        i += 1;
    }
    Ok(parsed)
}

/// `agent mcp --help`: pure local text, exactly like the top-level `--help` — this runs BEFORE the
/// JSON-RPC loop starts, so a human-readable stdout line here breaks no protocol discipline. The
/// default tool list is DERIVED from [`tools`] itself, never a second hand-typed name list.
pub(super) fn mcp_help_text() -> String {
    let default_tools = tools(Tier::Read);
    let default_names: Vec<&str> = default_tools
        .iter()
        .map(|t| t["name"].as_str().unwrap_or_default())
        .collect();
    format!(
        "ajh-tauri agent mcp [--allow-reversible] [--allow-irreversible] [--http <port>]\n\n\
         Run as an MCP (Model Context Protocol) server for Claude Code/Codex/any client, over \
         stdio by default; the desktop app must be running for any tool except `commands`.\n\n\
         FLAGS:\n\
         \x20 --allow-reversible     expose call-reversible (mutates state, undoable via the app)\n\
         \x20 --allow-irreversible   expose call-irreversible too (implies --allow-reversible)\n\
         \x20 --http <port>          serve MCP Streamable HTTP on 127.0.0.1:<port> instead of \
           stdio (no other bind shape is accepted); prints one \
           {{\"transport\":\"http\",\"url\":...,\"token\":...}} line to stdout, once, before \
           serving\n\
         \x20 --help, -h, help       show this help and exit (works even if the app is closed)\n\n\
         Default (no flags): {}.\n",
        default_names.join(", "),
    )
}

/// Writes [`mcp_help_text`] to `out` without an extra trailing blank line (LOW fix, review round
/// 3 — the text already ends in exactly one `\n`; `writeln!` doubled it). `write!`, never
/// `writeln!`.
pub(super) fn print_help(out: &mut impl Write) -> std::io::Result<()> {
    write!(out, "{}", mcp_help_text())
}

/// `agent mcp [flags]` entrypoint — called from [`super::run`]'s own argv sentinel, before
/// [`super::parse_verb`], exactly like `--help`. Never wrapped in [`super::run_verb_within`]'s
/// whole-invocation [`super::INVOCATION_TIMEOUT`] (that would kill a long-lived server after
/// 90s); each `tools/call` gets its own budget via the SAME constant instead. A fresh
/// [`super::query`] call — one HMAC handshake — runs per tool call rather than holding one socket
/// open, so token freshness, `token.revoked` handling, and the shared `BridgeState` throttle all
/// behave exactly as they do for the plain CLI, for free.
pub(crate) fn run(args: &[String]) -> i32 {
    let Ok(launch) = parse_launch_args(args) else {
        // Pre-protocol: no JSON-RPC frame exists yet, so stdout must stay silent — stderr only.
        // Never echoes the actual bad token (path privacy — a stray path-like argument must not
        // be reflected back).
        let _ = writeln!(
            std::io::stderr(),
            "unknown argument to `agent mcp` (expected: --allow-reversible, \
             --allow-irreversible, --http <port>, --help)"
        );
        return 2;
    };

    let out = stdout();
    if launch.help {
        let mut lock = out.lock();
        let _ = print_help(&mut lock);
        return 0;
    }

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => {
            let _ = writeln!(std::io::stderr(), "could not start an async runtime");
            return 2;
        }
    };
    let server = Server::new(launch.allow_reversible, launch.allow_irreversible);
    // Moves onto the dispatch thread WITH the runtime it owns, so `block_on` still runs from a
    // plain sync context (never inside the reactor) — just not on the thread that writes.
    let dispatch = move |verb: &Verb| -> Result<Value, &'static str> {
        rt.block_on(async {
            match timeout(INVOCATION_TIMEOUT, query(verb)).await {
                Ok(result) => result,
                Err(_) => Err(ERR_TIMEOUT),
            }
        })
    };
    // `--http` swaps the WIRE, never the handler: `http::run` shares `server` and `dispatch` with
    // the stdio path below verbatim — same tiers, same per-call bridge round trip, same
    // `handle_message` classifier (module doc's "Two transports, one handler" section).
    if let Some(port) = launch.http {
        return http::run(port, &server, dispatch);
    }
    // Never `stdin().lock()`/`out.lock()`: a `StdinLock`/`StdoutLock` is not `Send`, and reading
    // and writing now happen on different threads. `Stdin` itself is `Read` but not `BufRead`,
    // hence the `BufReader`; both handles lock internally per call, so the one-frame-per-line
    // discipline is unchanged (module doc).
    serve(
        BufReader::new(stdin()),
        out,
        &server,
        dispatch,
        INVOCATION_TIMEOUT,
    )
}
