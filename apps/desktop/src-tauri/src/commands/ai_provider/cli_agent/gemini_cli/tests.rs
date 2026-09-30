use super::*;

#[test]
fn streams_plain_text_lines_with_breaks() {
    assert_eq!(
        GeminiCliAgent.parse_stream_line("Hello"),
        Some(CliEvent::Delta("Hello\n".to_string()))
    );
    // Blank lines are preserved as paragraph breaks.
    assert_eq!(
        GeminiCliAgent.parse_stream_line(""),
        Some(CliEvent::Delta("\n".to_string()))
    );
}

#[test]
fn parse_complete_trims_output() {
    assert_eq!(
        GeminiCliAgent.parse_complete("  the answer\n\n").unwrap(),
        "the answer"
    );
    assert!(GeminiCliAgent.parse_complete("   ").is_err());
}

#[test]
fn credential_noise_is_filtered_from_both_paths() {
    // The streaming path drops the notice line but keeps the answer line.
    assert_eq!(
        GeminiCliAgent.parse_stream_line("Loaded cached credentials."),
        None
    );
    assert_eq!(
        GeminiCliAgent.parse_stream_line("Dear Team,"),
        Some(CliEvent::Delta("Dear Team,\n".to_string()))
    );
    // The one-shot path strips the notice and returns only the answer body.
    let out = "Loaded cached credentials.\nDear Team,\n\nThanks.\n";
    assert_eq!(
        GeminiCliAgent.parse_complete(out).unwrap(),
        "Dear Team,\n\nThanks."
    );
}

#[test]
fn argv_has_isolation_and_model_only_prompt_on_stdin() {
    let inv = GeminiCliAgent.stream_invocation("gemini-2.5-flash", "system text", None);
    // Prompt delivered on stdin — the CVE-2024-24576 fix: no untrusted JD text
    // in argv / `cmd.exe`.
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    // The `-p` value flag is gone; argv is the trusted isolation flags plus the
    // model selector.
    assert!(!inv.args.iter().any(|a| a == "-p"));
    assert_eq!(
        inv.args,
        vec![
            "-e",
            "none",
            "--allowed-mcp-server-names",
            "__ajh_none__",
            "-m",
            "gemini-2.5-flash",
        ]
    );
}

#[test]
fn argv_is_isolation_only_when_no_model() {
    // No model → the isolation flags remain, model flag dropped; the entire
    // prompt still goes on stdin.
    let inv = GeminiCliAgent.stream_invocation("", "", None);
    assert_eq!(
        inv.args,
        vec!["-e", "none", "--allowed-mcp-server-names", "__ajh_none__"]
    );
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
}
