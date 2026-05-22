use crate::applying::form_filler::FormFiller;
use crate::applying::runtime::ApplySession;
use crate::applying::selectors::FormSelectors;
use crate::applying::types::{Applier, ApplyContext, ApplyResult, ApplyStep};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::Ordering;
use std::time::Duration;

pub struct LinkedInApplier;

#[async_trait]
impl Applier for LinkedInApplier {
    fn board_id(&self) -> &'static str {
        "linkedin"
    }
    fn display_name(&self) -> &'static str {
        "LinkedIn"
    }

    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult> {
        linkedin_easy_apply(posting_url, ctx).await
    }
}

/// LinkedIn Easy Apply automation.
///
/// Strategy:
/// 1. Open posting in headed Chromium with persistent profile
/// 2. Click "Easy Apply" button
/// 3. Loop through modal steps:
///    - If "Submit Application" exists and auto_submit=true, click and return
///    - Else if "Continue"/"Next"/"Review" exists, click and loop
///    - Upload resume if file input appears and resume_path is set
///    - Max 10 iterations, then fall back to manual completion
async fn linkedin_easy_apply(
    posting_url: String,
    ctx: ApplyContext,
) -> Result<ApplyResult> {
    let app_data_dir = resolve_data_dir();
    let selectors = FormSelectors::linkedin();

    emit_step(&ctx, "launching", true, Some("Opening LinkedIn posting…"));

    let session = ApplySession::open(&app_data_dir, "linkedin", &posting_url, false).await?;

    emit_step(&ctx, "navigated", true, Some("Loaded LinkedIn posting"));

    // CAPTCHA detection
    if has_captcha(&session.page, &selectors.captcha_detection).await {
        emit_step(
            &ctx,
            "captcha_required",
            true,
            Some("CAPTCHA detected — please solve it in the browser window."),
        );
    }

    // Click Easy Apply button
    let easy_apply_selectors = [
        "button[aria-label*='Easy Apply']",
        "button.jobs-apply-button",
        ".jobs-apply-button",
    ];
    let mut clicked = false;
    for selector in &easy_apply_selectors {
        if ctx.signal.is_cancelled() {
            break;
        }
        if let Ok(el) = session.page.find_element(*selector).await {
            let _ = el.click().await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            clicked = true;
            emit_step(&ctx, "easy_apply_clicked", true, Some("Clicked Easy Apply"));
            break;
        }
    }

    if !clicked {
        emit_step(
            &ctx,
            "manual",
            true,
            Some("Could not find Easy Apply button — please apply manually."),
        );
        return wait_for_user_completion(session, ctx, posting_url).await;
    }

    // Easy Apply modal loop
    let filler = FormFiller::new(selectors);
    let max_iterations = 10;
    let mut iteration = 0;

    while iteration < max_iterations && !ctx.signal.is_cancelled() {
        iteration += 1;
        tokio::time::sleep(Duration::from_millis(800)).await;

        // Check for Submit Application button
        let submit_selectors = [
            "button[aria-label*='Submit application']",
            "button[aria-label*='Submit']",
            "button:has-text('Submit application')",
        ];
        for selector in &submit_selectors {
            if let Ok(el) = session.page.find_element(*selector).await {
                if ctx.auto_submit {
                    let _ = el.click().await;
                    emit_step(&ctx, "submitted", true, Some("Clicked Submit Application"));
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    session.close().await;
                    return Ok(ApplyResult {
                        ok: true,
                        stage: "submitted".to_string(),
                        submitted: true,
                        url: posting_url,
                        note: Some("Easy Apply submitted automatically".to_string()),
                    });
                } else {
                    emit_step(
                        &ctx,
                        "ready_to_submit",
                        true,
                        Some("Submit button found — auto_submit=false, waiting for user"),
                    );
                    return wait_for_user_completion(session, ctx, posting_url).await;
                }
            }
        }

        // Upload resume if file input appears
        if let Some(ref resume_path) = ctx.resume_path {
            if let Ok(_) = filler.upload_resume(&session.page, resume_path).await {
                emit_step(&ctx, "resume_uploaded", true, Some("Uploaded resume"));
            }
        }

        // Check for Continue / Next / Review buttons
        let continue_selectors = [
            "button[aria-label*='Continue']",
            "button[aria-label*='Next']",
            "button[aria-label*='Review']",
            "button:has-text('Continue')",
            "button:has-text('Next')",
            "button:has-text('Review')",
        ];
        let mut found_continue = false;
        for selector in &continue_selectors {
            if let Ok(el) = session.page.find_element(*selector).await {
                let _ = el.click().await;
                emit_step(
                    &ctx,
                    "step_advanced",
                    true,
                    Some(&format!("Clicked Continue/Next (iteration {})", iteration)),
                );
                found_continue = true;
                break;
            }
        }

        if !found_continue {
            // No Continue/Next/Submit found — either done or stuck
            emit_step(
                &ctx,
                "manual",
                true,
                Some("No more buttons found — please finish manually if needed."),
            );
            break;
        }
    }

    if iteration >= max_iterations {
        emit_step(
            &ctx,
            "max_iterations",
            true,
            Some("Reached max iterations — please finish manually."),
        );
    }

    // Wait for user to close the window
    wait_for_user_completion(session, ctx, posting_url).await
}

async fn wait_for_user_completion(
    session: ApplySession,
    ctx: ApplyContext,
    posting_url: String,
) -> Result<ApplyResult> {
    let timeout = Duration::from_secs(15 * 60);
    let start = std::time::Instant::now();
    while !session.closed.load(Ordering::SeqCst)
        && !ctx.signal.is_cancelled()
        && start.elapsed() < timeout
    {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let user_closed = session.closed.load(Ordering::SeqCst);
    let cancelled = ctx.signal.is_cancelled();

    let stage = if cancelled {
        "cancelled"
    } else if user_closed {
        "completed"
    } else {
        "timeout"
    };

    session.close().await;

    let note = if cancelled {
        Some("cancelled by user".to_string())
    } else if !user_closed {
        Some("session timed out after 15 minutes".to_string())
    } else {
        Some("user closed the apply window".to_string())
    };

    Ok(ApplyResult {
        ok: user_closed && !cancelled,
        stage: stage.to_string(),
        submitted: false,
        url: posting_url,
        note,
    })
}

async fn has_captcha(page: &chromiumoxide::Page, selectors: &[String]) -> bool {
    for s in selectors {
        if page.find_element(s.as_str()).await.is_ok() {
            return true;
        }
    }
    false
}

fn emit_step(ctx: &ApplyContext, stage: &str, ok: bool, note: Option<&str>) {
    if let Some(ref on_step) = ctx.on_step {
        on_step(ApplyStep {
            stage: stage.to_string(),
            ok,
            note: note.map(str::to_string),
        });
    }
}

fn resolve_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("AJH_DATA_DIR") {
        return std::path::PathBuf::from(dir);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    std::path::PathBuf::from(home).join(".ajh")
}
