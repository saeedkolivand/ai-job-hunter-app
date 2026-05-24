//! Shared apply flow used by all board appliers.
//!
//! Strategy: open the posting URL in a headed Chromium window reusing the
//! board's persistent login profile, attempt to click the board's "Apply"
//! button, then *wait for the user* to finish the application manually.
//! The session ends when the user closes the window.
//!
//! Full form-filling automation is intentionally not done here — selectors
//! drift weekly per board and break silently. The persistent profile + an
//! opened window covers 95% of the real value; a future iteration can layer
//! `FormFiller` on top per-board with proper testing.

use crate::applying::runtime::ApplySession;
use crate::applying::selectors::FormSelectors;
use crate::applying::types::{ApplyContext, ApplyResult, ApplyStep};
use anyhow::Result;
use std::sync::atomic::Ordering;
use std::time::Duration;

pub async fn navigate_and_assist(
    board_id: &str,
    display_name: &str,
    posting_url: String,
    ctx: ApplyContext,
    selectors: FormSelectors,
    apply_button_selectors: &[&str],
) -> Result<ApplyResult> {
    let app_data_dir = resolve_data_dir();
    let _ = &selectors; // reserved for future form-filling

    emit_step(&ctx, "launching", true, Some("Opening browser…"));

    let session = ApplySession::open(&app_data_dir, board_id, &posting_url, false).await?;

    emit_step(
        &ctx,
        "navigated",
        true,
        Some(&format!("Loaded {display_name} posting")),
    );

    // CAPTCHA detection — best-effort, before doing anything else. If we find
    // one, emit a distinct step so the UI can show a "complete the CAPTCHA in
    // the open browser" prompt.
    if has_captcha(&session.page, &selectors.captcha_detection).await {
        emit_step(
            &ctx,
            "captcha_required",
            true,
            Some("CAPTCHA detected — please solve it in the browser window."),
        );
    }

    // Try to click the board's apply button. Best-effort; failure is fine —
    // user can click it themselves.
    let mut clicked = false;
    for selector in apply_button_selectors {
        if ctx.signal.is_cancelled() {
            break;
        }
        if let Ok(el) = session.page.find_element(*selector).await {
            let _ = el.click().await;
            clicked = true;
            emit_step(&ctx, "apply_clicked", true, Some(selector));
            break;
        }
    }
    if !clicked {
        emit_step(
            &ctx,
            "manual",
            true,
            Some("Could not find Apply button — please click it yourself."),
        );
    }

    // Wait for the user to complete the application (i.e. close the window)
    // or for cancellation. Poll the closed flag every second.
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

    // Best-effort cleanup. If the user already killed it, this is a no-op.
    session.close().await;

    let note = if ctx.auto_submit {
        Some("auto_submit was requested but is not yet supported — please review and submit manually".to_string())
    } else if cancelled {
        Some("cancelled by user".to_string())
    } else if !user_closed {
        Some("session timed out after 15 minutes".to_string())
    } else {
        Some("user closed the apply window".to_string())
    };

    Ok(ApplyResult {
        ok: user_closed && !cancelled,
        stage: stage.to_string(),
        submitted: false, // We can't confirm without DOM scraping.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_data_dir_default() {
        unsafe { std::env::remove_var("AJH_DATA_DIR"); }
        let dir = resolve_data_dir();
        // Should use USERPROFILE or HOME
        assert!(dir.ends_with(".ajh"));
    }

    #[test]
    fn test_emit_step_with_callback() {
        let ctx = ApplyContext {
            signal: tokio_util::sync::CancellationToken::new(),
            auto_submit: false,
            resume_path: None,
            cover_letter: None,
            on_progress: None,
            on_step: Some(Box::new(|step| {
                // Just verify the callback receives the correct data
                assert_eq!(step.stage, "test");
                assert!(step.ok);
                assert_eq!(step.note, Some("test note".to_string()));
            })),
        };
        // Should not panic
        emit_step(&ctx, "test", true, Some("test note"));
    }

    #[test]
    fn test_emit_step_without_callback() {
        let ctx = ApplyContext {
            signal: tokio_util::sync::CancellationToken::new(),
            auto_submit: false,
            resume_path: None,
            cover_letter: None,
            on_progress: None,
            on_step: None,
        };
        // Should not panic
        emit_step(&ctx, "test", true, Some("test note"));
    }

    #[test]
    fn test_emit_step_with_none_note() {
        let ctx = ApplyContext {
            signal: tokio_util::sync::CancellationToken::new(),
            auto_submit: false,
            resume_path: None,
            cover_letter: None,
            on_progress: None,
            on_step: Some(Box::new(|step| {
                assert_eq!(step.stage, "test");
                assert!(step.ok);
                assert!(step.note.is_none());
            })),
        };
        emit_step(&ctx, "test", true, None);
    }

    #[test]
    fn test_emit_step_with_false_ok() {
        let ctx = ApplyContext {
            signal: tokio_util::sync::CancellationToken::new(),
            auto_submit: false,
            resume_path: None,
            cover_letter: None,
            on_progress: None,
            on_step: Some(Box::new(|step| {
                assert!(!step.ok);
            })),
        };
        emit_step(&ctx, "test", false, Some("failed"));
    }

    #[test]
    fn test_resolve_data_dir_with_env_var() {
        unsafe { std::env::set_var("AJH_DATA_DIR", "/custom/path"); }
        let dir = resolve_data_dir();
        assert_eq!(dir, std::path::PathBuf::from("/custom/path"));
        unsafe { std::env::remove_var("AJH_DATA_DIR"); }
    }

    #[test]
    fn test_resolve_data_dir_empty_env_var() {
        unsafe { std::env::set_var("AJH_DATA_DIR", ""); }
        let dir = resolve_data_dir();
        assert_eq!(dir, std::path::PathBuf::from(""));
        unsafe { std::env::remove_var("AJH_DATA_DIR"); }
    }

    #[test]
    fn test_resolve_data_dir_with_userprofile() {
        unsafe { std::env::remove_var("AJH_DATA_DIR"); }
        unsafe { std::env::set_var("USERPROFILE", "/home/user"); }
        let dir = resolve_data_dir();
        assert!(dir.ends_with(".ajh"));
        unsafe { std::env::remove_var("USERPROFILE"); }
    }

    #[test]
    fn test_resolve_data_dir_with_home() {
        unsafe { std::env::remove_var("AJH_DATA_DIR"); }
        unsafe { std::env::remove_var("USERPROFILE"); }
        unsafe { std::env::set_var("HOME", "/home/user"); }
        let dir = resolve_data_dir();
        assert!(dir.ends_with(".ajh"));
        unsafe { std::env::remove_var("HOME"); }
    }

    #[test]
    fn test_resolve_data_dir_no_env_vars() {
        unsafe { std::env::remove_var("AJH_DATA_DIR"); }
        unsafe { std::env::remove_var("USERPROFILE"); }
        unsafe { std::env::remove_var("HOME"); }
        let dir = resolve_data_dir();
        // Should return empty path + .ajh, which is just ".ajh"
        assert_eq!(dir, std::path::PathBuf::from(".ajh"));
    }

    #[test]
    fn test_apply_context_creation() {
        let ctx = ApplyContext {
            signal: tokio_util::sync::CancellationToken::new(),
            auto_submit: true,
            resume_path: Some("/path/to/resume.pdf".to_string()),
            cover_letter: Some("Dear hiring manager".to_string()),
            on_progress: Some(Box::new(|_, _| {})),
            on_step: Some(Box::new(|_| {})),
        };
        assert!(ctx.auto_submit);
        assert!(ctx.resume_path.is_some());
        assert!(ctx.cover_letter.is_some());
    }

    #[test]
    fn test_apply_context_defaults() {
        let ctx = ApplyContext {
            signal: tokio_util::sync::CancellationToken::new(),
            auto_submit: false,
            resume_path: None,
            cover_letter: None,
            on_progress: None,
            on_step: None,
        };
        assert!(!ctx.auto_submit);
        assert!(ctx.resume_path.is_none());
        assert!(ctx.cover_letter.is_none());
    }

    #[test]
    fn test_apply_result_creation() {
        let result = ApplyResult {
            ok: true,
            stage: "completed".to_string(),
            submitted: false,
            url: "https://example.com/job/123".to_string(),
            note: Some("Success".to_string()),
        };
        assert!(result.ok);
        assert_eq!(result.stage, "completed");
        assert!(!result.submitted);
    }

    #[test]
    fn test_apply_result_clone() {
        let result = ApplyResult {
            ok: true,
            stage: "test".to_string(),
            submitted: false,
            url: "https://example.com".to_string(),
            note: None,
        };
        let cloned = result.clone();
        assert_eq!(result.stage, cloned.stage);
        assert_eq!(result.ok, cloned.ok);
    }

    #[test]
    fn test_apply_step_creation() {
        let step = ApplyStep {
            stage: "test_stage".to_string(),
            ok: true,
            note: Some("test note".to_string()),
        };
        assert_eq!(step.stage, "test_stage");
        assert!(step.ok);
    }

    #[test]
    fn test_apply_step_clone() {
        let step = ApplyStep {
            stage: "test".to_string(),
            ok: false,
            note: None,
        };
        let cloned = step.clone();
        assert_eq!(step.stage, cloned.stage);
        assert_eq!(step.ok, cloned.ok);
    }
}
