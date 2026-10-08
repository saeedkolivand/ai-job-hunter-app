//! The cover letter written beside the draft (#1355): the decision to overlap,
//! the join's failure semantics, and a source guard on the wiring (a
//! `QualityCtx` needs a live `Completer`, so `Draft::run` cannot be driven).

use std::future::pending;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::commands::ai_provider::ProviderId;
use crate::error::AppError;
use crate::pipeline::resume::stages::{beside, can_overlap, Route};

const LOCAL: &str = "http://localhost:1234/v1";
const LAN: &str = "http://192.168.1.20:8000/v1";
const HOSTED: &str = "https://openrouter.ai/api/v1";

fn route(provider: ProviderId, base_url: Option<&str>) -> Route<'_> {
    Route { provider, base_url }
}

/// Mutation check (executed): make `one_request_at_a_time` return `false` for
/// `Ollama` and the first assertion fails; make it return `true` for every
/// provider and the cloud / CLI / hosted-gateway assertions fail.
#[test]
fn two_requests_to_one_local_server_do_not_overlap() {
    let ollama = route(ProviderId::Ollama, None);
    assert!(!can_overlap(ollama, ollama), "one Ollama host, one slot");

    let local = route(ProviderId::OpenAiCompatible, Some(LOCAL));
    assert!(!can_overlap(local, local));
    let lan = route(ProviderId::OpenAiCompatible, Some(LAN));
    assert!(!can_overlap(lan, lan));
    assert!(!can_overlap(
        route(ProviderId::OpenAiCompatible, Some(LOCAL)),
        lan
    ));
    // Names that only resolve on the user's own network are local too.
    for url in [
        "http://gpu-box:1234/v1",
        "http://mac-studio.lan:1234/v1",
        "http://box.internal/v1",
        "http://host.docker.internal:11434/v1",
        "http://x.tail1234.ts.net/v1",
        "http://x.home.arpa/v1",
        "http://x.local/v1",
    ] {
        let r = route(ProviderId::OpenAiCompatible, Some(url));
        assert!(!can_overlap(r, r), "{url}");
    }
}

#[test]
fn hosted_and_per_call_backends_overlap() {
    for provider in [
        ProviderId::Anthropic,
        ProviderId::OpenAi,
        ProviderId::ClaudeCode,
    ] {
        let r = route(provider, None);
        assert!(can_overlap(r, r), "{provider:?}");
    }
    let hosted = route(ProviderId::OpenAiCompatible, Some(HOSTED));
    assert!(can_overlap(hosted, hosted));
    // An unparseable or missing base URL is not evidence of a local server.
    let none = route(ProviderId::OpenAiCompatible, None);
    assert!(can_overlap(none, none));
}

/// The routing each stage RESOLVES decides, so a per-stage override counts.
#[test]
fn different_providers_overlap_even_when_one_is_local() {
    let ollama = route(ProviderId::Ollama, None);
    let cloud = route(ProviderId::Anthropic, None);
    assert!(can_overlap(ollama, cloud));
    assert!(can_overlap(cloud, ollama));
    let local = route(ProviderId::OpenAiCompatible, Some(LOCAL));
    assert!(can_overlap(ollama, local));
}

/// Mutation check (executed): replace the `select!` with
/// `draft.await?` followed by `letter.await` (sequential) and this hangs on the
/// never-ready letter, so the test fails by timeout; propagate a letter error
/// with `?` and `a_letter_failure_never_stops_the_draft` fails.
#[tokio::test(start_paused = true)]
async fn a_draft_failure_returns_at_once_and_drops_the_letter() {
    let draft = async { Err::<(), _>(AppError::Message("draft failed".into())) };
    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let flag = Dropped(Arc::clone(&dropped));
    let letter = async move {
        let _flag = flag;
        pending::<()>().await
    };
    let out = tokio::time::timeout(std::time::Duration::from_secs(1), beside(draft, letter)).await;
    assert!(matches!(out, Ok(Err(AppError::Message(m))) if m == "draft failed"));
    assert!(
        dropped.load(Ordering::SeqCst),
        "the letter future must be dropped"
    );
}

#[tokio::test(start_paused = true)]
async fn a_letter_failure_never_stops_the_draft() {
    let draft = async {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        Ok::<_, AppError>("draft")
    };
    let letter = async { Err::<&str, _>(AppError::Message("letter failed".into())) };
    let (drafted, letter) = beside(draft, letter).await.unwrap();
    assert_eq!(drafted, "draft");
    assert!(letter.is_err());
}

#[tokio::test(start_paused = true)]
async fn the_letter_is_awaited_when_it_is_the_slower_one() {
    let draft = async { Ok::<_, AppError>("draft") };
    let letter = async {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        "letter"
    };
    assert_eq!(beside(draft, letter).await.unwrap(), ("draft", "letter"));
}

#[tokio::test(start_paused = true)]
async fn both_run_at_once() {
    let started = tokio::time::Instant::now();
    // Async blocks, not bare `sleep(..)` futures: a `Sleep` fixes its deadline
    // when it is CREATED, so a sequential run would still read 10 s.
    let ten_seconds = || async { tokio::time::sleep(std::time::Duration::from_secs(10)).await };
    let draft = async {
        ten_seconds().await;
        Ok::<_, AppError>(())
    };
    beside(draft, ten_seconds()).await.unwrap();
    assert_eq!(started.elapsed().as_secs(), 10, "sequential would take 20");
}

/// Drop `//` comments and ALL whitespace, so line breaks cannot hide a call.
fn code_only(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pipeline/resume/stages");
    std::fs::read_to_string(path.join(rel))
        .unwrap()
        .lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .flat_map(str::split_whitespace)
        .collect()
}

/// Mutation check (executed): delete the `can_overlap(` condition, or the
/// `beside(` call, or the `ctx.letter_ahead = ` store in `draft.rs`, or the
/// `letter_ahead.take()` read in `cover_letter.rs`, and the matching assertion
/// fails.
#[test]
fn draft_and_cover_letter_wire_the_overlap() {
    let draft = code_only("draft.rs");
    assert!(
        draft.contains("can_overlap(route(completer),route(letter_completer))"),
        "draft must decide on the routing EACH stage resolves"
    );
    assert!(draft.contains("letter_completer=ctx.completer_for(LETTER_STAGE)"));
    assert!(draft.contains("beside(drafting,write_traced(LetterJob::new(ctx,brief)))"));
    assert!(draft.contains("ctx.letter_ahead=Some(letter);"));
    assert!(
        draft.contains("ctx.input.include_cover_letter&&!ctx.deadline.passed()&&can_overlap("),
        "a run with no letter, or out of time, must not start one"
    );

    let letter = code_only("cover_letter.rs");
    assert!(letter.contains("matchctx.letter_ahead.take(){"));
    assert!(letter.contains("call_trace::merge(ahead.calls);"));
}

/// The two documents must never share an `ai:stream` id. Mutation check
/// (executed): call `stream_captured` with the run's id and this fails.
#[test]
fn the_letter_streams_under_its_own_id_not_the_runs() {
    let letter = code_only("cover_letter.rs");
    assert!(letter.contains(".stream_captured_child(ctx.input.job_id,\"letter\",req)"));
    assert!(!letter.contains(".stream_captured("));
    assert!(code_only("draft.rs").contains("run_draft_attempt(env,input.job_id,"));
}
