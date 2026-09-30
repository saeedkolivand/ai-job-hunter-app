use super::*;

#[test]
fn streams_plain_text_and_filters_noise() {
    assert_eq!(
        AntigravityAgent.parse_stream_line("Hello"),
        Some(CliEvent::Delta("Hello\n".to_string()))
    );
    // Blank lines survive as paragraph breaks…
    assert_eq!(
        AntigravityAgent.parse_stream_line(""),
        Some(CliEvent::Delta("\n".to_string()))
    );
    // …credential noise does not.
    assert_eq!(
        AntigravityAgent.parse_stream_line("Loaded cached credentials."),
        None
    );
}

#[test]
fn parse_complete_strips_noise_and_trims() {
    let out = "Loaded cached credentials.\nDear Team,\n\nRegards.\n";
    assert_eq!(
        AntigravityAgent.parse_complete(out).unwrap(),
        "Dear Team,\n\nRegards."
    );
    assert!(AntigravityAgent.parse_complete("   ").is_err());
}

#[test]
fn argv_is_empty_prompt_on_stdin_and_never_auto_approves() {
    let inv = AntigravityAgent.stream_invocation("gemini-3-pro", "system text", None);
    // Prompt delivered on stdin — no untrusted JD text in argv / `cmd.exe`
    // (CVE-2024-24576).
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    // No prompt value flag…
    assert!(!inv.args.iter().any(|a| a == "-p"));
    // …and crucially NO `--yes`: never auto-approve tool actions on an untrusted
    // prompt (the HIGH finding). argv is empty — nothing but the piped prompt.
    assert!(!inv.args.iter().any(|a| a == "--yes"));
    assert!(inv.args.is_empty());
}

#[test]
fn complete_invocation_also_never_auto_approves() {
    let inv = AntigravityAgent.complete_invocation("gemini-3-pro", "system text", None);
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    assert!(!inv.args.iter().any(|a| a == "--yes"));
    assert!(inv.args.is_empty());
}

#[test]
fn inlines_system_prompt() {
    assert!(AntigravityAgent.inline_system());
}
