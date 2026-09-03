//! Cross-OS stdio smoke for `ajh-tauri agent mcp` — ADR-040's two recorded
//! sessions, run against the REAL binary instead of by hand.
//!
//! **What this proves and what it does not.** It drives the shipped
//! executable over real pipes: the argv intercept short-circuits above
//! `run()`, so `agent mcp` never boots Tauri, a window or the single-instance
//! plugin, and the whole session is one process reading stdin and writing
//! newline-delimited JSON to stdout. What it proves is the WIRE: one frame per
//! request in request order, a notification producing none, stdout carrying
//! nothing but frames, stderr staying empty, and exit 0. ADR-040 recorded
//! exactly these two sessions as ad-hoc manual runs and named macOS/Linux
//! smoke as the open gap; this is that gap closed for every platform CI runs.
//!
//! It is NOT a test of the release Windows binary's stdout path. A debug build
//! is console-subsystem (`#![cfg_attr(not(debug_assertions), windows_subsystem
//! = "windows")]` in `main.rs`), which is precisely the case
//! `platform::windows_console` exists to handle — so the discipline proven
//! here is the console-subsystem one.
//!
//! **Offline, and app-state independent.** Nothing here needs the network. The
//! bridge-backed tools try a loopback connect to the running app; with the app
//! CLOSED that fails and the tool result says so, and with the app OPEN the
//! same call returns real data. Session 2 asserts only what holds either way
//! — see [`the_app_not_running_session_still_answers_on_the_wire`].
//!
//! Locally: `cargo test --test mcp_smoke -- --nocapture`.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// How long one whole session may take before the child is killed and the
/// test fails with whatever it captured.
///
/// Generous on purpose. A bridge-backed tool call with the app closed scans
/// the app's six loopback ports, and on Windows each refused connect costs
/// seconds rather than returning instantly — a measured ~12 s for one call
/// against a sub-second figure elsewhere. This is a watchdog against a WEDGED
/// child, not a performance assertion; a regression that makes a session
/// slower is not what it is here to catch.
const SESSION_DEADLINE: Duration = Duration::from_secs(90);

/// Poll interval while waiting for the child to exit after its stdout closes.
const REAP_POLL: Duration = Duration::from_millis(25);

/// The tools the DEFAULT (read-only) tier must expose. Hand-written, not read
/// back from the binary's own reply: a list compared with itself would pass
/// for any list at all.
const READ_TIER_TOOLS: &[&str] = &[
    "best-matches",
    "job",
    "profile",
    "automations",
    "commands",
    "call-read",
];

/// One completed session: every stdout frame in order, all of stderr, and the
/// exit status.
struct Session {
    frames: Vec<Value>,
    stderr: String,
    code: Option<i32>,
}

/// Feed `lines` to `ajh-tauri agent mcp` on stdin, close stdin, and collect
/// everything it wrote.
///
/// stdout and stderr are drained on their own threads: a child that fills a
/// pipe buffer nobody is reading parks forever, and this server's own
/// [`emit`]-parks-on-a-full-pipe note says so explicitly. The deadline is
/// enforced on the stdout drain and again on the reap, so neither a wedged
/// writer nor a child that never exits can hang the suite.
fn run_session(lines: &[Value]) -> Session {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ajh-tauri"))
        .args(["agent", "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the ajh-tauri binary spawns");

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");

    let (out_tx, out_rx) = mpsc::channel::<Vec<String>>();
    std::thread::spawn(move || {
        let collected: Vec<String> = BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .collect();
        // A send failure means the test already gave up and dropped the
        // receiver — nothing left to report to.
        let _ = out_tx.send(collected);
    });
    let (err_tx, err_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let mut stderr = stderr;
        let _ = stderr.read_to_string(&mut buf);
        let _ = err_tx.send(buf);
    });

    {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        for line in lines {
            // A write failure is the child having exited early; the
            // assertions below report what it actually did, which is far more
            // useful than panicking here on a broken pipe.
            if writeln!(stdin, "{line}").is_err() {
                break;
            }
        }
        // Dropped here, closing stdin — the EOF that ends the server's loop.
    }

    let raw = match out_rx.recv_timeout(SESSION_DEADLINE) {
        Ok(lines) => lines,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "`agent mcp` wrote nothing more for {SESSION_DEADLINE:?} and was killed; \
                 stderr so far: {:?}",
                err_rx.try_recv().unwrap_or_default()
            );
        }
    };

    // stdout is closed, so the child is on its way out — but bound the reap
    // anyway rather than trusting it.
    let code = loop {
        match child.try_wait().expect("try_wait must not fail") {
            Some(status) => break status.code(),
            None if started.elapsed() >= SESSION_DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("`agent mcp` closed stdout but never exited; frames were {raw:#?}");
            }
            None => std::thread::sleep(REAP_POLL),
        }
    };

    let frames = raw
        .iter()
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|e| {
                panic!("stdout must carry ONLY JSON frames, got {line:?} ({e})")
            })
        })
        .collect();

    Session {
        frames,
        stderr: err_rx.recv_timeout(SESSION_DEADLINE).unwrap_or_default(),
        code,
    }
}

fn request(id: u32, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn initialize(id: u32) -> Value {
    request(
        id,
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "mcp_smoke", "version": "0" },
        }),
    )
}

fn tool_call(id: u32, name: &str) -> Value {
    request(id, "tools/call", json!({ "name": name, "arguments": {} }))
}

/// Every frame's `id`, in the order the server wrote them.
fn ids(session: &Session) -> Vec<u64> {
    session
        .frames
        .iter()
        .map(|f| {
            f.get("id")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("every frame must carry its request id: {f}"))
        })
        .collect()
}

/// The concatenated `text` of a `tools/call` result's content blocks.
fn result_text(frame: &Value) -> String {
    frame
        .pointer("/result/content")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("a tools/call reply must carry result.content: {frame}"))
        .iter()
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

/// ADR-040 session 1: the full handshake plus three reads, one frame each,
/// in order — and the notification producing none, which is the half a
/// request-counting client would never notice was wrong.
#[test]
fn the_recorded_handshake_session_answers_one_frame_per_request_in_order() {
    let session = run_session(&[
        initialize(1),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        tool_call(2, "commands"),
        request(3, "ping", json!({})),
        request(4, "tools/list", json!({})),
    ]);

    assert_eq!(
        ids(&session),
        vec![1, 2, 3, 4],
        "exactly four frames, in request order — the notification must produce none: {:#?}",
        session.frames
    );

    let init = &session.frames[0];
    assert!(
        init.pointer("/result/capabilities/tools").is_some(),
        "initialize must advertise the tools capability: {init}"
    );

    let commands = &session.frames[1];
    assert_ne!(
        commands.pointer("/result/isError").and_then(Value::as_bool),
        Some(true),
        "the `commands` tool answers locally with no app running: {commands}"
    );

    assert_eq!(
        session.frames[2].get("result"),
        Some(&json!({})),
        "ping's result is the empty object: {}",
        session.frames[2]
    );

    let listed: Vec<&str> = session.frames[3]
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("tools/list must return an array: {}", session.frames[3]))
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(
        listed, READ_TIER_TOOLS,
        "the default tier exposes exactly the read tools, in this order"
    );

    assert_eq!(
        session.stderr, "",
        "stderr must stay empty — a client reading it as a failure signal would see one"
    );
    assert_eq!(session.code, Some(0), "a clean EOF is exit 0");
}

/// ADR-040 session 2: a bridge-backed call with stdin closed IMMEDIATELY
/// after it. The interesting property is that the reply is still written
/// during the EOF drain rather than being lost to the shutdown.
///
/// The reply's CONTENT depends on whether this machine happens to have the
/// app running: closed, the bridge scan fails and the tool result is an error
/// naming `app_not_running`; open, the same call returns a real profile. Both
/// are correct, so the assertions cover what holds either way, and the
/// error's text is only checked when the call actually errored.
#[test]
fn the_app_not_running_session_still_answers_on_the_wire() {
    let session = run_session(&[initialize(1), tool_call(2, "profile")]);

    assert_eq!(
        ids(&session),
        vec![1, 2],
        "the call in flight when stdin closed must still be answered: {:#?}",
        session.frames
    );

    let call = &session.frames[1];
    let text = result_text(call);
    if call.pointer("/result/isError").and_then(Value::as_bool) == Some(true) {
        assert!(
            text.contains("app_not_running"),
            "with the app closed, the refusal must name the reason: {text}"
        );
    } else {
        assert!(
            !text.is_empty(),
            "with the app running, the tool result must carry real content: {call}"
        );
    }

    assert_eq!(session.stderr, "", "stderr must stay empty");
    assert_eq!(session.code, Some(0), "a clean EOF is exit 0");
}
