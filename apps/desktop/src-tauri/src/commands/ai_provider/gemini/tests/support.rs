//! Shared fixtures for the `gemini` adapter's test topics.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::super::super::{AiGenerateRequest, AiProvider, SamplingProfile};
use super::super::body::StructuredCall;
use super::super::GeminiClient;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;

pub(super) fn base_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: "gemini-1.5-flash".to_string(),
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

/// Mirrors exactly what `GeminiClient::chat_stream` does: resolve this
/// adapter's own profile for `req.model` + `req.intent`, then merge with the
/// request's explicit numeric overrides.
pub(super) fn sampling_for(req: &AiGenerateRequest) -> SamplingProfile {
    GeminiClient
        .sampling_profile(&req.model, super::super::super::resolve_intent(req))
        .resolve(req)
}

/// The structured half of a `generateContent` call with nothing set — the
/// per-test variations override the one field they are about.
pub(super) fn structured_call() -> StructuredCall<'static> {
    StructuredCall {
        json: true,
        schema: None,
        effort: None,
        max_tokens: None,
    }
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
