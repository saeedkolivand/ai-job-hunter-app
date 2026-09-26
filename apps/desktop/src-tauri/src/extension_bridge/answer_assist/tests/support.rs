//! Shared test fixtures used by several `answer_assist` test topics.

use crate::applications::{Application, ApplicationStatus};
use crate::error::{AppError, AppResult};

use super::super::compose::{charge_compose_budget, DraftComposer};

/// A synthetic Application carrying only the salary-adjacent fields under
/// test (title "Rust Engineer", company "Acme") — also doubles as the
/// role/company source for `fetch_web_notes`/`resolve_salary_range` tests.
pub(super) fn app_with_salary(
    min: Option<f64>,
    max: Option<f64>,
    currency: Option<&str>,
) -> Application {
    Application {
        id: "a1".to_string(),
        status: ApplicationStatus::Saved,
        applied_at: None,
        created_at: 0,
        updated_at: 0,
        job_url: "https://example.com/job/1".to_string(),
        board: "adzuna".to_string(),
        company: "Acme".to_string(),
        title: "Rust Engineer".to_string(),
        candidate: String::new(),
        answers: Vec::new(),
        brief: String::new(),
        job_description: String::new(),
        notes: String::new(),
        next_action_at: None,
        next_action_notified_at: None,
        comp: String::new(),
        contact_name: String::new(),
        contact_email: String::new(),
        job_summary: String::new(),
        recipient_name: String::new(),
        recipient_email: String::new(),
        salary_min: min,
        salary_max: max,
        salary_currency: currency.map(str::to_string),
    }
}

/// A `JobCanceller` implementor that just discards `job_id` — several topics
/// only need `cancel` to actually remove an entry, not to inspect what got
/// cancelled.
pub(super) struct NoopCanceller;

impl crate::extension_bridge::assist_registry::JobCanceller for NoopCanceller {
    fn cancel_job(&self, _job_id: &str) {}
}

// ── compose_with_length_retry fixtures — shared by `compose_retry_core` and
// `compose_retry_guards` ──────────────────────────────────────────────────

/// A [`crate::extension_bridge::FrameSink`] recorder — a local copy of
/// `stream`'s own test-only sink.
#[derive(Default)]
pub(super) struct RecordingSink {
    pub(super) sent: Vec<String>,
}

#[async_trait::async_trait]
impl crate::extension_bridge::FrameSink for RecordingSink {
    async fn send_frame(&mut self, text: String) -> bool {
        self.sent.push(text);
        true
    }
}

/// The empty-answer error EXACTLY as the shared streaming loop produces it,
/// built through `commands::ai_provider`'s own message picker rather than
/// re-typed here — so a reworded constant can never make these fixtures stop
/// matching the classification under test.
pub(super) fn empty_answer(
    stop_reason: Option<crate::commands::ai_provider::StopReason>,
) -> AppError {
    crate::commands::ai_provider::stream::empty_answer_error_for_test(
        stop_reason,
        crate::commands::ai_provider::ProviderId::OllamaCloud,
    )
}

/// The specific failure this path retries: the model spent its whole output
/// budget reasoning and the provider ended the stream with
/// `finish_reason: length` and no answer text.
pub(super) fn length_cut() -> AppError {
    empty_answer(Some(crate::commands::ai_provider::StopReason::Length))
}

/// One scripted attempt: the visible deltas the provider emits for it, and
/// how it ends. Both halves matter — an attempt can BOTH forward text and end
/// as an empty length cut (a local model that spells its reasoning as
/// ordinary `<think>` prose gets it forwarded as visible deltas while the
/// provider's own answer accumulator strips it), which is the case the
/// per-attempt char budget and the tail slice both exist for.
pub(super) struct FakeAttempt {
    delta: String,
    outcome: AppResult<()>,
}

/// An attempt that streams `delta` and finishes normally.
pub(super) fn streams(delta: impl Into<String>) -> FakeAttempt {
    FakeAttempt {
        delta: delta.into(),
        outcome: Ok(()),
    }
}

/// An attempt that streams `delta` and then fails with `e` — the shape a
/// local model produces when it spells its reasoning as ordinary text and the
/// provider's answer accumulator strips it back to empty.
pub(super) fn streams_then_fails(delta: impl Into<String>, e: AppError) -> FakeAttempt {
    FakeAttempt {
        delta: delta.into(),
        outcome: Err(e),
    }
}

/// An attempt that streams nothing and fails with `e`.
pub(super) fn fails(e: AppError) -> FakeAttempt {
    streams_then_fails("", e)
}

/// A fake compose round: replays a scripted attempt in order, records the
/// `(max_tokens, effort)` each one was driven with, counts the daily-ceiling
/// charges, and forwards each attempt's text through the REAL
/// `stream::forward_chunk`/`FrameSink` path into ONE shared buffer, under the
/// `cap_base` the production driver hands it — so "the retry's text reaches
/// the sink", "each attempt gets its own `DRAFT_CAP`" and "the draft is the
/// successful attempt's tail" are all assertions about the production
/// forwarding code and the production driver, not about the fake. `finish`
/// likewise emits the REAL `assist.done` frame.
pub(super) struct FakeComposer<'a> {
    /// One entry per attempt, consumed in order.
    script: Vec<FakeAttempt>,
    /// `(max_tokens, effort)` recorded per attempt, in order.
    pub(super) attempts: Vec<(u32, Option<String>)>,
    /// Successful `charge` calls — `Cell` because `charge` takes `&self`,
    /// exactly as the production trait does.
    pub(super) charges: std::cell::Cell<usize>,
    /// Attempt number (1-based) whose charge the daily ceiling refuses.
    pub(super) refuse_charge_at: Option<usize>,
    /// What `still_wanted` answers — `false` stands in for an `assist.cancel`
    /// (or the whole connection dropping) landing between the two attempts,
    /// which takes this request's registry entry away.
    pub(super) wanted: bool,
    /// `finish` calls — the terminal-frame count, cross-checked against the
    /// `assist.done` frames the sink actually received.
    pub(super) finishes: usize,
    /// Answer chars forwarded across ALL attempts — the fake's stand-in for
    /// `stream::ComposeStream::forwarded`, shared for the same reason.
    forwarded: String,
    limiter: crate::limits::Limiter,
    sink: &'a mut RecordingSink,
}

impl DraftComposer for FakeComposer<'_> {
    fn charge(&self) -> AppResult<()> {
        let n = self.charges.get() + 1;
        if self.refuse_charge_at == Some(n) {
            return Err(super::super::errors::to_draft_failed(
                "daily budget exceeded before compose",
                AppError::RateLimited("daily ceiling reached".to_string()),
            ));
        }
        // The REAL charge, against a real limiter, so this fake can never
        // diverge from what one production round-trip actually costs.
        charge_compose_budget(&self.limiter, "ollama-cloud")?;
        self.charges.set(n);
        Ok(())
    }

    fn still_wanted(&self) -> bool {
        self.wanted
    }

    fn drafted(&self) -> &str {
        &self.forwarded
    }

    async fn compose(
        &mut self,
        max_tokens: u32,
        effort: Option<&str>,
        cap_base: usize,
    ) -> AppResult<()> {
        self.attempts.push((max_tokens, effort.map(str::to_string)));
        let index = self.attempts.len() - 1;
        let slot = self
            .script
            .get_mut(index)
            .expect("the composer must never be driven more times than the script allows");
        let delta = std::mem::take(&mut slot.delta);
        // Moved out (not cloned) so the error keeps its EXACT `AppError`
        // variant — the classification under test is structural.
        let outcome = std::mem::replace(&mut slot.outcome, Ok(()));

        if !delta.is_empty() {
            let chunk = crate::events::AiStreamChunk {
                job_id: "job-1".to_string(),
                delta,
                done: false,
                error: None,
                thinking: None,
            };
            crate::extension_bridge::stream::forward_chunk(
                &chunk,
                "req-1",
                self.sink,
                &mut self.forwarded,
                cap_base,
            )
            .await;
        }
        outcome
    }

    async fn finish(&mut self) {
        use crate::extension_bridge::FrameSink as _;

        self.finishes += 1;
        self.sink
            .send_frame(crate::extension_bridge::stream::assist_done_frame("req-1"))
            .await;
    }
}

impl<'a> FakeComposer<'a> {
    pub(super) fn new(script: Vec<FakeAttempt>, sink: &'a mut RecordingSink) -> Self {
        Self {
            script,
            attempts: Vec::new(),
            charges: std::cell::Cell::new(0),
            refuse_charge_at: None,
            wanted: true,
            finishes: 0,
            forwarded: String::new(),
            limiter: crate::limits::Limiter::new(),
            sink,
        }
    }
}

/// How many of `sent` are terminal `assist.done` frames — parsed off the
/// wire text, so this counts what the CLIENT would see.
pub(super) fn done_frames(sent: &[String]) -> usize {
    sent.iter()
        .filter(|f| {
            serde_json::from_str::<serde_json::Value>(f)
                .ok()
                .and_then(|v| v["type"].as_str().map(str::to_string))
                .as_deref()
                == Some(crate::extension_bridge::msg::ASSIST_DONE)
        })
        .count()
}

/// Chars the CLIENT was actually sent, summed off the `assist.chunk` frames —
/// the wire total, not the fake's own bookkeeping.
pub(super) fn forwarded_chars(sent: &[String]) -> usize {
    sent.iter()
        .filter_map(|f| serde_json::from_str::<serde_json::Value>(f).ok())
        .filter_map(|v| v["payload"]["delta"].as_str().map(|d| d.chars().count()))
        .sum()
}
