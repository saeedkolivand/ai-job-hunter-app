//! Company research for the cover letter, started the moment its last input
//! exists instead of inline in the letter stage.
//!
//! The brief needs the company (known before the run), the job ad (known
//! before the run) and the analysed role title (known once `analyze_job` ends).
//! The letter stage is the only reader and sits behind `strategy` and `draft`,
//! so the lookup is raced against those stages instead of added to them.
//!
//! ## Shape
//!
//! A `Completer` is not `'static`, so the lookup cannot be `tokio::spawn`ed.
//! It is a plain future ([`QualityCtx::start_early_research`]) that the run
//! driver polls alongside the pipeline via [`race_background`], and two
//! one-shot channels link it to the ctx: `analyze_job` publishes the role
//! ([`QualityCtx::publish_role`]), the letter stage awaits the brief.
//!
//! * **Cancellation:** the lookup future is dropped when the pipeline future
//!   finishes (success, error or cancel), which drops the in-flight provider
//!   request; it also races the run's own cancel token.
//! * **Never fatal:** the lookup yields a `String` (`""` = no brief), same as
//!   the inline path; a dropped channel reads as `""` too.
//! * **Not started at all** unless the run writes a letter AND asked for
//!   research, so a letter-less run never admits or pays for a lookup.

use std::future::Future;

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::stages::{research_brief, LETTER_STAGE};
use super::QualityCtx;

/// The ctx-side ends of the two channels. Each end is consumed once.
pub struct EarlyResearch {
    role_tx: Option<oneshot::Sender<String>>,
    brief_rx: Option<oneshot::Receiver<String>>,
}

impl EarlyResearch {
    /// The ctx half plus the two ends the lookup future owns.
    fn channel() -> (Self, oneshot::Receiver<String>, oneshot::Sender<String>) {
        let (role_tx, role_rx) = oneshot::channel();
        let (brief_tx, brief_rx) = oneshot::channel();
        let early = Self {
            role_tx: Some(role_tx),
            brief_rx: Some(brief_rx),
        };
        (early, role_rx, brief_tx)
    }

    pub(crate) fn publish_role(&mut self, role: &str) {
        if let Some(tx) = self.role_tx.take() {
            let _ = tx.send(role.to_string());
        }
    }

    /// The brief's receiver, once. `Err` on it means the lookup was dropped
    /// before answering, which the caller reads as "no brief".
    ///
    /// Also drops the role sender: the stage reading the brief is downstream
    /// of `analyze_job`, so a role still unpublished here never will be, and
    /// keeping the sender alive would hang both the lookup and the letter.
    pub(crate) fn take_brief(&mut self) -> Option<oneshot::Receiver<String>> {
        self.role_tx.take();
        self.brief_rx.take()
    }
}

/// Wait for the role, run `research` on it, hand the brief over. Returns
/// without researching when the role never arrives (analyze failed).
pub(crate) async fn drive<F, Fut>(
    role_rx: oneshot::Receiver<String>,
    brief_tx: oneshot::Sender<String>,
    research: F,
) where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = String>,
{
    let Ok(role) = role_rx.await else { return };
    let _ = brief_tx.send(research(role).await);
}

/// Poll `background` alongside `main`, but return as soon as `main` does,
/// dropping `background` (and whatever request it has in flight). `None` is a
/// plain `main.await`.
pub(crate) async fn race_background<T>(
    main: impl Future<Output = T>,
    background: Option<impl Future<Output = ()>>,
) -> T {
    tokio::pin!(main);
    let Some(background) = background else {
        return main.await;
    };
    tokio::pin!(background);
    let mut background_done = false;
    loop {
        tokio::select! {
            out = &mut main => return out,
            _ = &mut background, if !background_done => background_done = true,
        }
    }
}

/// Research is armed only for a run that writes a letter AND asked for it.
fn should_research(include_cover_letter: bool, research_company: bool) -> bool {
    include_cover_letter && research_company
}

/// `research`, or `""` (no brief) the moment `cancel` fires; the dropped
/// future takes its in-flight provider request with it.
async fn cancellable(cancel: &CancellationToken, research: impl Future<Output = String>) -> String {
    tokio::select! {
        brief = research => brief,
        () = cancel.cancelled() => String::new(),
    }
}

impl<'a> QualityCtx<'a> {
    /// Arm the early lookup and return its future for the run driver to race
    /// against the pipeline. `None` (nothing armed, nothing admitted) unless
    /// the run writes a letter and asked for research. The future borrows
    /// nothing from `self`.
    pub fn start_early_research(
        &mut self,
        cancel: CancellationToken,
    ) -> Option<impl Future<Output = ()> + use<'a>> {
        if !should_research(self.input.include_cover_letter, self.input.research_company) {
            return None;
        }
        let (early, role_rx, brief_tx) = EarlyResearch::channel();
        self.early_research = Some(early);
        let completer = self.completer_for(LETTER_STAGE);
        let (job_ad, company, effort) = (
            self.input.job_ad,
            self.input.company_name,
            self.input.effort,
        );
        Some(drive(role_rx, brief_tx, move |role| async move {
            cancellable(
                &cancel,
                research_brief(completer, job_ad, company, &role, effort),
            )
            .await
        }))
    }

    /// Hand the analysed role to the armed lookup. Called by `analyze_job`
    /// after it sets `analysis`; a no-op when nothing is armed.
    pub(crate) fn publish_role(&mut self) {
        if let Some(early) = self.early_research.as_mut() {
            early.publish_role(self.analysis.role_title.trim());
        }
    }
}

#[cfg(test)]
mod tests;
