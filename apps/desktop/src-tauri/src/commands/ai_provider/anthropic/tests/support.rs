//! Shared fixtures for the `anthropic` adapter's test topics.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::super::super::{AiGenerateRequest, AiProvider, ModelCapabilities, SamplingProfile};
use super::super::AnthropicClient;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;

pub(super) fn base_request(model: &str) -> AiGenerateRequest {
    AiGenerateRequest {
        model: model.to_string(),
        messages: vec![AiGenerateRequestMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }],
        locale: "en".to_string(),
        temperature: Some(0.8),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        repeat_penalty: None,
        max_tokens: None,
        context_window: None,
        effort: None,
        intent: None,
    }
}

/// The real capability matrix for `model`, exactly as every trait method
/// computes it — tests must exercise the same `caps.supports_temperature`
/// gate the adapter actually uses, not a hand-rolled stand-in.
pub(super) fn caps_for(model: &str) -> ModelCapabilities {
    AnthropicClient.capabilities(model)
}

/// Mirrors what `AnthropicClient::chat_stream` does: resolve this adapter's
/// own profile for `req.model` + `req.intent`, merged with the request's
/// explicit numeric overrides.
pub(super) fn sampling_for(req: &AiGenerateRequest) -> SamplingProfile {
    AnthropicClient
        .sampling_profile(&req.model, super::super::super::resolve_intent(req))
        .resolve(req)
}

/// A minimal raw-socket HTTP/1.1 server that writes each response's status
/// line + headers immediately, then sleeps `body_delay` BEFORE writing that
/// response's body. `wiremock::ResponseTemplate::set_delay` cannot build
/// this: its delay elapses entirely BEFORE any bytes (headers included)
/// reach the socket, so it only ever exercises `req.send()`'s own timeout —
/// never a `resp.text()`/`resp.json()` blocked on a body that's slow to
/// arrive AFTER `send()` already resolved, which is the actual gap a
/// cumulative deadline has to cover. Serves `bodies` in order, one per
/// accepted connection (`Connection: close`, so each page fetch opens a new
/// one) — `bodies[i].1` is that page's body-write delay.
pub(super) async fn slow_body_server(bodies: Vec<(String, Duration)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for (body, body_delay) in bodies {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            // Drain the request up to the blank line ending its headers —
            // contents don't matter, only connection ORDER distinguishes pages.
            let mut buf = [0u8; 4096];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") => break,
                    Ok(_) => continue,
                }
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(headers.as_bytes()).await;
            let _ = socket.flush().await;
            if !body_delay.is_zero() {
                tokio::time::sleep(body_delay).await;
            }
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    format!("http://{addr}")
}

/// A server that accepts the connection immediately (so a CONNECT check would
/// succeed) but writes NOTHING — not even the status line — until `delay` has
/// passed. This is `.send()`'s own timeout, not [`slow_body_server`]'s
/// already-resolved-headers case: it is what a backend that computes its
/// whole answer before responding at all (Ollama's non-streaming
/// `/api/chat`, generating for minutes with nothing on the wire until it is
/// done) actually looks like on the socket.
pub(super) async fn wholly_slow_server(delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = [0u8; 4096];
        loop {
            match socket.read(&mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(n) if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") => break,
                Ok(_) => continue,
            }
        }
        tokio::time::sleep(delay).await;
        let body = "{}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.flush().await;
    });
    format!("http://{addr}")
}

/// Bind an ephemeral loopback port, free it, then connect to the same port
/// and return the resulting refusal — bounded-retried against a fresh port
/// each time, because a sibling test can reclaim the just-freed port before
/// this one reconnects. Generous on the per-attempt timeout on purpose: a
/// loopback refusal is normally sub-millisecond, but a sandboxed/virtualized
/// network stack can add real latency before the RST arrives (observed: ~2s
/// in one such environment) — this only needs to clear that, not race it. A
/// GENUINE timeout would still fail the caller's `is_timeout()` assertion
/// regardless of this bound, since that assertion checks the classification,
/// not the wall-clock.
pub(super) async fn refused_connection_error() -> reqwest::Error {
    for _ in 0..5 {
        let refused_addr = {
            let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
            probe.local_addr().unwrap()
            // `probe` drops here — the port is released with nothing bound to it.
        };
        let url = reqwest::Url::parse(&format!("http://{refused_addr}/v1/messages")).unwrap();
        match crate::net::http::shared()
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Err(e) => return e,
            // Another test's listener claimed the freed port in the gap
            // between drop and connect — retry with a fresh one rather than
            // let an accidental success pass (or panic for the wrong reason).
            Ok(_) => continue,
        }
    }
    panic!("could not observe a refused loopback connection after 5 attempts");
}
