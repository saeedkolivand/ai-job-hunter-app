//! The WebSocket handshake acceptance for one accepted TCP connection — split from `mod.rs`
//! (R8 relief; pure code motion). Validates `Origin` (defense-in-depth only — see `mod.rs`'s own
//! module doc for the real boundary, the v2 mutual HMAC handshake driven AFTER this returns) and
//! caps the message/frame size before any byte is buffered.

use parking_lot::Mutex;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

use super::caller_gate::CallerClass;
use super::{auth, origin_log, MAX_FRAME_BYTES};

pub(super) type WsStream = tokio_tungstenite::WebSocketStream<TcpStream>;

/// Perform the WS handshake, resolving this socket's [`CallerClass`] from its `Origin` header
/// along the way (finding #5, security review; extended for the extension caller in PR1) —
/// nothing else threads the handshake `Origin` this far. `None` on ANY handshake failure
/// (including a disallowed origin, refused with 403 inside the callback itself).
pub(super) async fn accept_ws(stream: TcpStream) -> Option<(WsStream, CallerClass)> {
    use tokio_tungstenite::tungstenite::handshake::server::ErrorResponse;
    use tokio_tungstenite::tungstenite::http::StatusCode;

    let dev_origins = crate::platform::config::extension_dev_origins();
    // Captured by the callback below (it only borrows `req`, which does not
    // outlive the handshake) and read once the handshake resolves — this is
    // how `advance_authenticated` learns THIS socket's `CallerClass`
    // (finding #5, security review; extended for the extension caller in
    // PR1), since nothing else threads the handshake `Origin` this far.
    let caller_class = std::sync::Arc::new(Mutex::new(CallerClass::Other));
    let caller_class_out = caller_class.clone();
    // Origin allowlist enforced IN the handshake: a disallowed `Origin` is
    // refused with 403 before the socket upgrades, so a non-extension page never
    // reaches the frame loop. The closure's `Result<_, ErrorResponse>` is the
    // signature tungstenite's `Callback` trait mandates; `ErrorResponse`
    // (http::Response<Option<String>>) is inherently large, so the
    // `result_large_err` lint is unavoidable here — scoped-allow with reason.
    #[allow(clippy::result_large_err)] // API-imposed Callback signature (tungstenite)
    let callback = move |req: &Request, res: Response| {
        let origin = req
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        *caller_class_out.lock() = CallerClass::resolve(origin, &dev_origins);
        if auth::is_allowed_origin(origin, &dev_origins) {
            Ok(res)
        } else {
            origin_log::warn_rejected_origin_once(origin);
            let resp = ErrorResponse::new(Some("forbidden origin".to_string()));
            let (mut parts, body) = resp.into_parts();
            parts.status = StatusCode::FORBIDDEN;
            Err(ErrorResponse::from_parts(parts, body))
        }
    };

    // Cap both message + frame size at the handshake so an oversized frame is
    // rejected by the protocol layer before we ever buffer it. `WebSocketConfig`
    // is `#[non_exhaustive]`; its setters are consuming builders.
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));

    let ws =
        match tokio_tungstenite::accept_hdr_async_with_config(stream, callback, Some(ws_config))
            .await
        {
            Ok(ws) => ws,
            Err(e) => {
                origin_log::log_handshake_failure(&e);
                return None;
            }
        };
    let caller_class = *caller_class.lock();
    Some((ws, caller_class))
}
