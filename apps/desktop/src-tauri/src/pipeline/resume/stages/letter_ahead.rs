//! Writing the cover letter BESIDE the draft.
//!
//! `cover_letter` reads nothing `draft` produces (source résumé, posting,
//! `ctx.strategy`, the company brief), so when the two can overlap, `draft`
//! starts the letter alongside its own call and parks the outcome in
//! [`QualityCtx::letter_ahead`](super::super::QualityCtx). The `cover_letter`
//! stage still runs at its place in the pipeline — its boundary checks,
//! `pipeline:stage` events, ledger record and error all unchanged — and only
//! finds the letter already written.
//!
//! ## When the two overlap
//!
//! [`can_overlap`], over the routing EACH stage resolves (`completer_for`, so a
//! per-stage override counts — never the run's default provider). Not when both
//! land on one local server that works a single request at a time: a second
//! request would only queue behind the draft and could trip the stream idle
//! timeout while it waited, and the GPU gains nothing.
//!
//! ## Failure semantics
//!
//! A draft error returns at once and drops the letter (cancelling its stream),
//! exactly as the run stopped at `draft` before. A letter error never touches
//! the draft: it is parked and returned by the `cover_letter` stage, as before.

use std::future::Future;

use crate::commands::ai_provider::call_trace::{CallLog, CallRecord};
use crate::commands::ai_provider::ProviderId;
use crate::error::AppResult;

use super::cover_letter::{write_letter, LetterJob, LetterOut};

/// A stage's resolved routing, as far as server contention is concerned.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Route<'a> {
    pub provider: ProviderId,
    pub base_url: Option<&'a str>,
}

/// Whether two stages' requests can run at the same time to any benefit.
pub(crate) fn can_overlap(a: Route<'_>, b: Route<'_>) -> bool {
    !(a.provider == b.provider && one_request_at_a_time(a))
}

/// Whether `route` is a server that works one request at a time. All Ollama
/// traffic goes to one host. An OpenAI-compatible server only is when it is
/// local (LM Studio, llama.cpp, vLLM) — a hosted gateway serves requests in
/// parallel. "Local" is a loopback/private address, a single-label host
/// (`gpu-box`) or a LAN-style suffix (see [`is_lan_name`]). The caller has checked both routes share the provider, so two
/// local servers on different ports are conservatively treated as one. CLI
/// agents and cloud APIs run a request per call, so they overlap.
fn one_request_at_a_time(route: Route<'_>) -> bool {
    match route.provider {
        ProviderId::Ollama => true,
        ProviderId::OpenAiCompatible => route
            .base_url
            .and_then(|url| reqwest::Url::parse(url).ok())
            .and_then(|url| url.host_str().map(str::to_string))
            .is_some_and(|host| {
                !crate::net::ssrf::is_safe_public_host(&host) || is_lan_name(&host)
            }),
        _ => false,
    }
}

/// A name that only resolves on the user's own network or machine.
fn is_lan_name(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    !host.contains('.')
        || [".lan", ".internal", ".home.arpa", ".ts.net", ".local"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
}

/// The letter written ahead, and the calls it made (traced under their own
/// log, so they are not mistaken for the draft's).
pub(crate) struct LetterAhead {
    pub(crate) result: AppResult<LetterOut>,
    pub(crate) calls: Vec<CallRecord>,
}

/// [`write_letter`] under its own [`CallLog`].
pub(crate) async fn write_traced(job: LetterJob<'_, '_>) -> LetterAhead {
    let log = CallLog::default();
    let result = log.scope(write_letter(job)).await;
    LetterAhead {
        result,
        calls: log.take(),
    }
}

/// Drive `draft` and `letter` together. A draft error returns immediately and
/// drops `letter`; whatever `letter` yields (error included) never interrupts
/// `draft`. When the draft finishes first the letter is awaited to the end.
pub(crate) async fn beside<D, T, L>(draft: D, letter: L) -> AppResult<(T, L::Output)>
where
    D: Future<Output = AppResult<T>>,
    L: Future,
{
    tokio::pin!(draft, letter);
    let mut written = None;
    let drafted = loop {
        tokio::select! {
            drafted = &mut draft => break drafted?,
            out = &mut letter, if written.is_none() => written = Some(out),
        }
    };
    let letter = match written {
        Some(out) => out,
        None => letter.await,
    };
    Ok((drafted, letter))
}
