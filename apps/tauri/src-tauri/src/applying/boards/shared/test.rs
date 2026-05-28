use super::*;

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
