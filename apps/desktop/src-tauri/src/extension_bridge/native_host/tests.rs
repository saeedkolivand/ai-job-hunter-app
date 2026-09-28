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

// This test does NOT exercise cancellation — see the note below for why
// a genuine "cancel `read_stdin_frame` mid-frame, then read again" test
// isn't included here. What IS pinned: a length-prefixed frame delivered
// across MANY small physical reads (a slow/chunked native-messaging pipe
// — the realistic shape the old `select!` duplex pump could truncate)
// must reassemble whole even while unrelated work is concurrently
// scheduled on the same runtime. `pump_stdin_to_ws`/`pump_ws_to_stdout`
// themselves can't be driven here without a live ws (see the module doc
// on `next_ws_payload` — that half needs a real bridge server); this
// pins the one piece both of them depend on: `read_stdin_frame`
// correctly resuming a partial `read_exact` across many polls, never
// losing bytes, regardless of what else the executor is doing meanwhile.
//
// Why no cancellation test: `read_exact` (and therefore
// `read_stdin_frame`) is NOT cancellation-safe by design — an in-flight
// call, if dropped (e.g. by losing a `select!` race), silently loses
// whatever bytes it already pulled off the reader. That's the ORIGINAL
// bug `relay`'s module doc describes. A test that races a live
// `read_stdin_frame` call inside a `select!` against an
// immediately-ready future would be flaky — whether bytes are lost
// depends on `select!`'s branch-poll order, which tokio deliberately
// leaves unspecified — and would only re-demonstrate
// `AsyncReadExt::read_exact`'s own documented limitation, not anything
// about this codebase. The guarantee we actually rely on is
// architectural, not a property of this helper: `relay` gives each
// direction (`pump_stdin_to_ws` / `pump_ws_to_stdout`) EXCLUSIVE,
// never-raced ownership of its reader for its whole lifetime (see
// `relay`'s doc), so `read_stdin_frame` is never called from inside a
// `select!` in production.
#[tokio::test]
async fn read_stdin_frame_reassembles_fragmented_delivery_under_concurrent_scheduling() {
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
