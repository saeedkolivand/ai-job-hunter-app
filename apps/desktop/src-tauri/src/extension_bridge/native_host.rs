//! Native-messaging transport host — a near-dumb byte relay between a browser's
//! native-messaging Port (stdio) and the running desktop app's existing loopback
//! `ws://127.0.0.1` bridge ([`super`]).
//!
//! ## Why this exists
//! Firefox's HTTPS-Only Mode silently upgrades a background script's
//! `ws://127.0.0.1` → `wss://`, which the plain loopback bridge can't serve (no
//! TLS on loopback). A native-messaging host is a real spawned process, immune to
//! that upgrade: the browser launches our own exe (in `--native-host` mode), and
//! this relay forwards frames over plain `ws://` to the bridge. Same wire
//! envelope; only the transport changes.
//!
//! ```text
//! Browser ⇄ (stdio: u32-len-prefix + JSON) ⇄ this host ⇄ (ws loopback) ⇄ app
//! ```
//!
//! ## Not a protocol parser
//! The host does NOT understand the bridge protocol. The v2 mutual-handshake
//! frames (`hello`/`challenge`/`auth`/`auth.ok` — never a per-frame token) and
//! every session-authorized frame after relay through **verbatim**, 1:1, so
//! bridge auth ([`super::auth`], [`super::handshake`]) + pairing UX are
//! untouched. The ONE thing it speaks itself is a transport-local readiness
//! control frame (`bridge.ready`) so the extension can tell "app reachable" from
//! "app down" — that frame never reaches the bridge and is NOT part of the
//! `@ajh/shared` bridge protocol.
//!
//! Invoked from `main()` BEFORE Tauri boots (see `lib::run_native_host_if_invoked`),
//! so there is no ambient Tokio runtime — [`run`] builds its own.

use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio::io::{stdin, stdout, AsyncReadExt, AsyncWriteExt, Stdin, Stdout};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Message};

use super::auth::NATIVE_HOST_ORIGIN;
use super::{MAX_FRAME_BYTES, PORT_RANGE};

/// `--native-host` entry point. Builds its own current-thread runtime (no Tauri
/// reactor exists at this point in `main`) and blocks on the relay. Never panics
/// out — a build failure just logs and returns so the process exits cleanly.
pub fn run() {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::warn!("[native_host] failed to build runtime: {e}");
            return;
        }
    };
    rt.block_on(relay());
}

/// Connect to the first live bridge port, announce readiness, then relay
/// frames in BOTH directions between stdin and the ws until either side
/// closes.
///
/// ## Two independent tasks, not a `select!` duplex pump
/// This used to race a stdin-read branch against a ws-read branch in one
/// `tokio::select!` loop (needed since extension roadmap PR 10 added a
/// stream's multiple `assist.chunk`/`assist.done` frames — a single
/// request/reply cycle was no longer enough). That raced
/// [`read_stdin_frame`]'s `read_exact` calls against ws activity:
/// `AsyncReadExt::read_exact` is NOT cancellation-safe — when the ws branch
/// became ready first, `select!` dropped the still-in-flight stdin read,
/// discarding whatever bytes it had already pulled off stdin (already
/// consumed from the pipe, but never forwarded). That desyncs the
/// length-prefixed native-messaging frame stream for every read after it —
/// the next 4-byte length prefix gets read from the middle of the dropped
/// frame's body.
///
/// The fix: each direction now owns its stdio handle and its ws half
/// EXCLUSIVELY, on its OWN task ([`pump_stdin_to_ws`] / [`pump_ws_to_stdout`])
/// — nothing ever races (`select!`s) against either task's `read_exact`, so
/// neither is ever dropped mid-read. `relay` itself only races the two
/// tasks' `JoinHandle`s (waiting for whichever direction closes first);
/// dropping a `JoinHandle` future detaches rather than aborts the task, so
/// the other direction's in-flight read (if any) just keeps running,
/// harmlessly, until the process exits right after this function returns
/// (native-host mode never falls through to booting Tauri — see
/// `run_native_host_if_invoked`). This host still understands nothing about
/// `reqId` correlation or multiple concurrent requests — that multiplexing
/// lives in `bridge.ts` and the desktop's own connection loop; the host
/// stays a dumb byte relay in both directions.
async fn relay() {
    let ws = match connect_bridge().await {
        Some(ws) => ws,
        None => {
            // App not running / no bridge port answered. Tell the extension, then
            // exit cleanly — the browser Port disconnects and the extension shows
            // app_not_running (it will re-spawn us on the next attempt).
            write_ready(false).await;
            return;
        }
    };
    write_ready(true).await;

    let (ws_write, ws_read) = ws.split();
    let stdin_to_ws = tokio::spawn(pump_stdin_to_ws(stdin(), ws_write));
    let ws_to_stdout = tokio::spawn(pump_ws_to_stdout(ws_read, stdout()));

    // Wait for whichever direction ends first (EOF, malformed frame, or a
    // send/write failure) — see the doc above for why racing the
    // `JoinHandle`s here, unlike racing the reads themselves, is safe.
    tokio::select! {
        _ = stdin_to_ws => {}
        _ = ws_to_stdout => {}
    }

    // On any exit (stdin EOF or ws closed) flip the extension to app_not_running.
    write_ready(false).await;
}

/// Browser → host → bridge. Owns stdin and the ws write half exclusively for
/// its whole lifetime, so its `read_exact` (inside [`read_stdin_frame`]) is
/// never raced against anything — see [`relay`]'s doc.
async fn pump_stdin_to_ws(mut input: Stdin, mut ws_write: WsWrite) {
    loop {
        let bytes = match read_stdin_frame(&mut input).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => break, // clean EOF — Port closed
            Err(_) => break,   // malformed / over-cap length
        };
        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => continue, // not UTF-8 JSON — drop, keep the Port alive
        };
        if ws_write.send(Message::text(text)).await.is_err() {
            break;
        }
    }
}

/// Bridge → host → browser. Owns the ws read half and stdout exclusively for
/// its whole lifetime — see [`relay`]'s doc.
async fn pump_ws_to_stdout(mut ws_read: WsRead, mut output: Stdout) {
    while let Some(reply) = next_ws_payload(&mut ws_read).await {
        if write_stdout_frame(&mut output, reply.as_bytes())
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Probe [`PORT_RANGE`] in order; the first port whose TCP connect + ws handshake
/// both succeed wins. The handshake sends `Origin: ajh-native-host` (accepted by
/// [`super::auth::is_allowed_origin`]). Uses `client_async_with_config` over a
/// plain `TcpStream` — the crate has no TLS/`connect` feature, only `handshake`.
async fn connect_bridge() -> Option<WsStream> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));

    for port in PORT_RANGE {
        let tcp = match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(tcp) => tcp,
            Err(_) => continue,
        };
        let uri = match format!("ws://127.0.0.1:{port}/").parse() {
            Ok(uri) => uri,
            Err(_) => continue,
        };
        let request = ClientRequestBuilder::new(uri).with_header("Origin", NATIVE_HOST_ORIGIN);
        match tokio_tungstenite::client_async_with_config(request, tcp, Some(config)).await {
            Ok((ws, _resp)) => return Some(ws),
            Err(_) => continue,
        }
    }
    None
}

type WsStream = tokio_tungstenite::WebSocketStream<TcpStream>;
/// The write half of a split [`WsStream`] — owned exclusively by
/// [`pump_stdin_to_ws`]; see [`relay`]'s doc.
type WsWrite = futures::stream::SplitSink<WsStream, Message>;
/// The read half of a split [`WsStream`] — owned exclusively by
/// [`pump_ws_to_stdout`]; see [`relay`]'s doc.
type WsRead = futures::stream::SplitStream<WsStream>;

/// Read ws frames until the next Text/Binary payload (skipping Ping/Pong, which
/// tungstenite auto-answers); `None` on close/error. NOTE: the ws round-trip
/// can't be unit-tested here — it needs a live bridge server.
async fn next_ws_payload(ws: &mut WsRead) -> Option<String> {
    while let Some(frame) = ws.next().await {
        match frame {
            Ok(Message::Text(t)) => return Some(t.to_string()),
            Ok(Message::Binary(b)) => return String::from_utf8(b.to_vec()).ok(),
            Ok(Message::Close(_)) | Err(_) => return None,
            _ => continue, // ping/pong/other control — keep reading
        }
    }
    None
}

/// Write the transport-local readiness control frame. Best-effort: a write
/// failure (browser already gone) is ignored — we're exiting anyway.
async fn write_ready(ok: bool) {
    let frame = json!({ "type": "bridge.ready", "ok": ok }).to_string();
    let _ = write_stdout_frame(&mut stdout(), frame.as_bytes()).await;
}

// ── Native-messaging stdio framing ─────────────────────────────────────────────
// Each message = a 4-byte length prefix in NATIVE byte order, then that many
// bytes of UTF-8 JSON. An inbound length over the cap is rejected without
// allocating.

/// Read one length-prefixed stdin frame. `Ok(None)` on clean EOF (Port closed);
/// `Err` on a partial read or an over-cap length (caller exits).
async fn read_stdin_frame<R: AsyncReadExt + Unpin>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf).await {
        Ok(_) => {}
        // EOF at a frame boundary is the normal Port-closed shutdown.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_ne_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "native-messaging frame over size cap",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

/// Write one length-prefixed stdout frame (native-order u32 prefix + bytes) and
/// flush. Over-cap payloads are an internal bug — refuse rather than emit a frame
/// the browser will reject.
async fn write_stdout_frame<W: AsyncWriteExt + Unpin>(
    w: &mut W,
    payload: &[u8],
) -> std::io::Result<()> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "native-messaging frame over size cap",
        ));
    }
    let len = payload.len() as u32;
    w.write_all(&len.to_ne_bytes()).await?;
    w.write_all(payload).await?;
    w.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The ws round-trip can't be exercised here (it needs a live bridge server);
    // this pins only the stdio framing — the one bit of non-trivial byte logic.
    #[tokio::test]
    async fn stdio_frame_round_trips() {
        // v2 frames carry no token; the host is a dumb byte relay either way.
        let value = json!({ "type": "import.request", "reqId": "1", "payload": { "url": "x" } });
        let json = value.to_string();

        // Encode: native-order u32 length prefix + UTF-8 JSON.
        let mut encoded = (json.len() as u32).to_ne_bytes().to_vec();
        encoded.extend_from_slice(json.as_bytes());

        // Decode it back through the reader the relay uses.
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = read_stdin_frame(&mut cursor).await.unwrap().unwrap();
        assert_eq!(decoded, json.as_bytes());

        // A second read at EOF is the clean Port-closed signal, not an error.
        assert!(read_stdin_frame(&mut cursor).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rejects_over_cap_length() {
        // A length prefix above MAX_FRAME_BYTES is refused without allocating it.
        let over = (MAX_FRAME_BYTES as u32 + 1).to_ne_bytes().to_vec();
        let mut cursor = std::io::Cursor::new(over);
        assert!(read_stdin_frame(&mut cursor).await.is_err());
    }

    // Regression coverage for the cancellation-safety bug `relay`'s doc
    // describes: a length-prefixed frame delivered across MANY small
    // physical reads (a slow/chunked native-messaging pipe — the realistic
    // shape the old `select!` duplex pump could truncate) must reassemble
    // whole even while unrelated work is concurrently scheduled on the same
    // runtime. `pump_stdin_to_ws`/`pump_ws_to_stdout` themselves can't be
    // driven here without a live ws (see the module doc on `next_ws_payload`
    // — that half needs a real bridge server); this pins the one piece that
    // both of them — and the fix — depend on: `read_stdin_frame` correctly
    // resuming a partial `read_exact` across many polls, never losing bytes,
    // regardless of what else the executor is doing meanwhile.
    #[tokio::test]
    async fn read_stdin_frame_survives_fragmented_delivery_under_concurrent_activity() {
        let body = json!({
            "type": "import.request",
            "reqId": "1",
            "payload": { "note": "x".repeat(5_000) },
        })
        .to_string();
        let mut encoded = (body.len() as u32).to_ne_bytes().to_vec();
        encoded.extend_from_slice(body.as_bytes());

        // A small duplex buffer forces `read_exact` to resume across many
        // separate polls instead of completing in one.
        let (mut writer, mut reader) = tokio::io::duplex(64);

        // Unrelated concurrent activity on the SAME runtime, interleaved with
        // the frame delivery below — proves the read isn't corrupted by
        // whatever else gets scheduled around it.
        let busy = tokio::spawn(async {
            for _ in 0..200 {
                tokio::task::yield_now().await;
            }
        });
        let feeder = tokio::spawn(async move {
            for chunk in encoded.chunks(7) {
                writer.write_all(chunk).await.unwrap();
                tokio::task::yield_now().await;
            }
        });

        let decoded = read_stdin_frame(&mut reader).await.unwrap().unwrap();
        assert_eq!(decoded, body.as_bytes());

        feeder.await.unwrap();
        busy.await.unwrap();
    }
}
