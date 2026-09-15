use super::*;
use crate::ai_generations::{AiGenerationRecord, ApplicationAnswer, InterviewQuestion};
use crate::export::types::{DocumentType, ExportFormat, LetterLayout, TemplateId};

/// A minimal, fully-populated `AiGenerationRecord` fixture — every field must be given (the
/// struct has no `Default`), so this centralizes the boilerplate the way
/// `agent_call::tests::ai_generation_record_struct_fixture_fences_the_posting_derived_fields`
/// already does for its own module.
fn generation_record(resume_text: &str, cover_letter_text: &str) -> AiGenerationRecord {
    AiGenerationRecord {
        id: "gen-1".to_string(),
        created_at: 1_700_000_000_000,
        candidate_name: "  Jane Candidate  ".to_string(),
        job_title: "Staff Engineer".to_string(),
        company_name: "Example Corp".to_string(),
        resume_language: "en".to_string(),
        job_ad_language: "en".to_string(),
        target_language: " de ".to_string(),
        mismatch: false,
        top_requirements: vec![],
        mode: "text".to_string(),
        resume_text: resume_text.to_string(),
        cover_letter_text: cover_letter_text.to_string(),
        job_ad: String::new(),
        job_url: "https://example.com/job/1".to_string(),
        board: "linkedin".to_string(),
        application_answers: Vec::<ApplicationAnswer>::new(),
        company_brief: String::new(),
        interview_questions: Vec::<InterviewQuestion>::new(),
        email_subject: String::new(),
        email_body: String::new(),
        application_id: None,
        quality_report: String::new(),
    }
}

// ── parse_request (pure) ─────────────────────────────────────────────────────

#[test]
fn parse_request_reads_a_generation_source() {
    let payload = json!({
        "source": { "kind": "generation", "url": "https://example.com/job/1" },
        "kind": "resume",
        "format": "pdf",
        "templateId": "classic",
    });
    let parsed = parse_request(&payload).expect("well-formed request must parse");
    assert_eq!(
        parsed.source,
        Source::Generation("https://example.com/job/1".to_string())
    );
    assert_eq!(parsed.document_type, DocumentType::Resume);
    assert_eq!(parsed.format, ExportFormat::Pdf);
    assert_eq!(parsed.template_id, TemplateId::Classic);
    assert_eq!(parsed.letter_layout, LetterLayout::Classic);
    assert!(!parsed.ats_mode);
}

#[test]
fn parse_request_reads_a_document_source_with_optional_fields() {
    let payload = json!({
        "source": { "kind": "document", "id": "doc-1" },
        "kind": "cover-letter",
        "format": "docx",
        "templateId": "regent",
        "letterLayoutId": "banded",
        "atsMode": true,
    });
    let parsed = parse_request(&payload).expect("well-formed request must parse");
    assert_eq!(parsed.source, Source::Document("doc-1".to_string()));
    assert_eq!(parsed.document_type, DocumentType::CoverLetter);
    assert_eq!(parsed.format, ExportFormat::Docx);
    assert_eq!(parsed.template_id, TemplateId::Regent);
    assert_eq!(parsed.letter_layout, LetterLayout::Banded);
    assert!(parsed.ats_mode);
}

#[test]
fn parse_request_falls_back_on_an_unknown_template_or_layout_id() {
    // Mirrors `documents_export_document`'s own tolerance (`export::types`'s custom
    // `Deserialize` impls) — an unrecognized id degrades to Classic rather than refusing the
    // whole request.
    let payload = json!({
        "source": { "kind": "generation", "url": "https://example.com/job/1" },
        "kind": "resume",
        "format": "pdf",
        "templateId": "some-removed-template",
        "letterLayoutId": "some-removed-layout",
    });
    let parsed = parse_request(&payload).expect("an unknown template/layout id must not refuse");
    assert_eq!(parsed.template_id, TemplateId::Classic);
    assert_eq!(parsed.letter_layout, LetterLayout::Classic);
}

#[test]
fn parse_request_rejects_malformed_shapes() {
    let base = json!({
        "source": { "kind": "generation", "url": "https://example.com/job/1" },
        "kind": "resume",
        "format": "pdf",
        "templateId": "classic",
    });

    let mut missing_source = base.clone();
    missing_source.as_object_mut().unwrap().remove("source");
    assert_eq!(
        parse_request(&missing_source).unwrap_err(),
        ERR_INVALID_REQUEST
    );

    let unknown_source_kind = json!({
        "source": { "kind": "bogus", "url": "https://example.com/job/1" },
        "kind": "resume", "format": "pdf", "templateId": "classic",
    });
    assert_eq!(
        parse_request(&unknown_source_kind).unwrap_err(),
        ERR_INVALID_REQUEST
    );

    let empty_url = json!({
        "source": { "kind": "generation", "url": "  " },
        "kind": "resume", "format": "pdf", "templateId": "classic",
    });
    assert_eq!(parse_request(&empty_url).unwrap_err(), ERR_INVALID_REQUEST);

    let mut missing_kind = base.clone();
    missing_kind.as_object_mut().unwrap().remove("kind");
    assert_eq!(
        parse_request(&missing_kind).unwrap_err(),
        ERR_INVALID_REQUEST
    );

    let unknown_kind = json!({
        "source": { "kind": "generation", "url": "https://example.com/job/1" },
        "kind": "bogus", "format": "pdf", "templateId": "classic",
    });
    assert_eq!(
        parse_request(&unknown_kind).unwrap_err(),
        ERR_INVALID_REQUEST
    );

    let mut missing_format = base.clone();
    missing_format.as_object_mut().unwrap().remove("format");
    assert_eq!(
        parse_request(&missing_format).unwrap_err(),
        ERR_INVALID_REQUEST
    );

    let unknown_format = json!({
        "source": { "kind": "generation", "url": "https://example.com/job/1" },
        "kind": "resume", "format": "bogus", "templateId": "classic",
    });
    assert_eq!(
        parse_request(&unknown_format).unwrap_err(),
        ERR_INVALID_REQUEST
    );

    let mut missing_template = base.clone();
    missing_template
        .as_object_mut()
        .unwrap()
        .remove("templateId");
    assert_eq!(
        parse_request(&missing_template).unwrap_err(),
        ERR_INVALID_REQUEST
    );
}

// ── resolve_generation (pure) ────────────────────────────────────────────────

#[test]
fn resolve_generation_returns_none_when_the_requested_kind_is_empty() {
    let record = generation_record("some résumé text", "");
    assert!(resolve_generation(&record, DocumentType::CoverLetter).is_none());
    assert!(resolve_generation(&record, DocumentType::Resume).is_some());
}

#[test]
fn resolve_generation_trims_and_omits_empty_meta_fields() {
    let record = generation_record("some résumé text", "some cover letter text");
    let (text, meta) =
        resolve_generation(&record, DocumentType::Resume).expect("resume text is present");
    assert_eq!(text, "some résumé text");
    assert_eq!(meta.candidate_name.as_deref(), Some("Jane Candidate"));
    assert_eq!(meta.target_language.as_deref(), Some("de"));

    let mut blank_meta = record;
    blank_meta.job_title = "   ".to_string();
    let (_, meta) =
        resolve_generation(&blank_meta, DocumentType::Resume).expect("resume text is present");
    assert_eq!(
        meta.job_title, None,
        "a whitespace-only field must be omitted, not sent blank"
    );
}

// ── cover_letter_market (pure) ───────────────────────────────────────────────

#[test]
fn cover_letter_market_maps_known_languages_and_falls_back_to_intl() {
    assert_eq!(cover_letter_market(Some("de")), "de");
    assert_eq!(
        cover_letter_market(Some(" DE ")),
        "de",
        "trims + lowercases"
    );
    assert_eq!(
        cover_letter_market(Some("de-AT")),
        "de",
        "first two chars only"
    );
    assert_eq!(cover_letter_market(Some("fr")), "fr");
    assert_eq!(cover_letter_market(Some("en")), "intl");
    assert_eq!(cover_letter_market(Some("nl")), "intl");
    assert_eq!(cover_letter_market(None), "intl");
    assert_eq!(cover_letter_market(Some("")), "intl");
    assert_eq!(cover_letter_market(Some("xx")), "intl", "unmapped language");
}

/// The defect this guards against: `handle_document_export` used to hardcode
/// `ExportRequest.locale = None` for every source, so a German cover letter's
/// body-only text got the English salutation/sign-off from
/// `complete_letter_text`'s `"intl"` default (`export::commands::mod::
/// validate_and_normalize`) instead of its own market's. Exercises the REAL
/// export pipeline (not a re-implementation) with the locale
/// `handle_document_export` now resolves via `cover_letter_market`, mirroring
/// `resolve_generation`'s own `GenerationMeta { target_language: Some("de"), .. }`
/// shape for a `DocumentType::CoverLetter` source.
#[tokio::test]
async fn cover_letter_export_with_resolved_locale_gets_the_letters_own_salutation_not_english() {
    use crate::export::types::ExportRequest;

    let body_only_de =
        "Ich schreibe Ihnen, um mein Interesse an der Stelle als Softwareentwickler auszudrücken.";
    let meta = crate::export::types::GenerationMeta {
        candidate_name: Some("Max Müller".to_string()),
        job_title: None,
        company_name: None,
        target_language: Some("de".to_string()),
    };
    let locale = cover_letter_market(meta.target_language.as_deref()).to_string();
    assert_eq!(
        locale, "de",
        "the exact resolution handle_document_export performs"
    );

    let request = ExportRequest {
        text: body_only_de.to_string(),
        format: ExportFormat::Txt,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: Some(meta),
        ats_mode: false,
        locale: Some(locale),
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = crate::export::commands::documents_export_document(request)
        .await
        .expect("txt export of a valid cover letter must succeed");
    let text = String::from_utf8(result.data).expect("txt export must be valid UTF-8");

    assert!(
        text.contains("Sehr geehrte Damen und Herren"),
        "expected the German salutation, got: {text:?}"
    );
    assert!(
        !text.contains("Dear Hiring Manager") && !text.contains("Sincerely,"),
        "must not carry the English salutation/sign-off for a German letter, got: {text:?}"
    );
}

// ── Throttle (mirrors match_live::MatchLiveThrottle's own test shape) ───────

#[test]
fn throttle_allows_a_burst_then_refuses() {
    let mut t = DocumentExportThrottle::new();
    let now = std::time::Instant::now();
    assert!(t.try_acquire_at(now), "1st request in the burst");
    assert!(t.try_acquire_at(now), "2nd request in the burst");
    assert!(t.try_acquire_at(now), "3rd request in the burst");
    assert!(
        !t.try_acquire_at(now),
        "a 4th immediate request must be throttled (burst exhausted)"
    );
}

#[test]
fn throttle_refills_one_token_per_interval_not_a_second_burst() {
    let mut t = DocumentExportThrottle::new();
    let t0 = std::time::Instant::now();
    for _ in 0..3 {
        assert!(t.try_acquire_at(t0));
    }
    assert!(!t.try_acquire_at(t0), "bucket is empty");
    assert!(
        t.retry_after_ms() > 0,
        "an exhausted bucket must report a positive wait"
    );

    let t1 = t0 + std::time::Duration::from_secs_f64(DOCUMENT_EXPORT_REFILL_SECS);
    assert!(
        t.try_acquire_at(t1),
        "exactly one interval later, one token must have refilled"
    );
    assert!(
        !t.try_acquire_at(t1),
        "only ONE token refilled — this must not re-open the full burst"
    );
}

#[test]
fn throttle_state_is_isolated_per_instance() {
    let mut a = DocumentExportThrottle::new();
    let mut b = DocumentExportThrottle::new();
    let now = std::time::Instant::now();
    for _ in 0..3 {
        assert!(a.try_acquire_at(now));
    }
    assert!(!a.try_acquire_at(now));
    assert!(
        b.try_acquire_at(now),
        "a second instance's bucket is untouched by the first's"
    );
}

// ── Reply builders ────────────────────────────────────────────────────────────

#[test]
fn origin_refused_reply_carries_the_fixed_sentinel() {
    let v: Value = serde_json::from_str(&origin_refused_reply("req-1")).unwrap();
    assert_eq!(v["type"], msg::DOCUMENT_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["error"], "origin_refused");
}

#[test]
fn extension_gate_reply_reuses_the_pr1_extension_read_gate_sentinel() {
    let v: Value = serde_json::from_str(&extension_gate_reply("req-2")).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(
        v["payload"]["error"],
        super::super::agent_call::ERR_EXTENSION_READ_GATE
    );
}

#[test]
fn throttled_reply_carries_rate_limited_and_retry_after_ms() {
    let v: Value = serde_json::from_str(&throttled_reply("req-3", 1234)).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(
        v["payload"]["error"],
        super::super::agent_call::ERR_RATE_LIMITED
    );
    assert_eq!(v["payload"]["retryAfterMs"], 1234);
}

// ── Frame-budget measurement + base64 round trip (real export, every template) ──

/// About 3 dense pages of résumé text — the worst-case export length this verb is likely to be
/// asked to render. Deliberately dense (long bullet lines, no filler) rather than merely long, so
/// this approximates a real busy résumé instead of padding with whitespace.
fn dense_sample_resume_text() -> String {
    let mut text =
        String::from("# Jordan Alexander Whitfield-Nakamura\nSenior Staff Software Engineer\n\n");
    for i in 0..40 {
        text.push_str(&format!(
            "## Senior Software Engineer, Globex Technologies International {i}\n\
             Jan 2015 - Dec 2020 | Remote\n\
             - Led cross-functional teams of 12+ engineers delivering distributed systems at \
             scale, reducing latency by 42% and improving reliability across a microservice \
             architecture spanning multiple regions and cloud providers.\n\
             - Architected and implemented event-driven pipelines processing 500M+ daily events \
             using Kafka, Rust, and PostgreSQL with sub-100ms p99 latency requirements under \
             sustained peak load.\n\
             - Mentored junior engineers, conducted code reviews, and established engineering \
             best practices that were adopted company-wide across a dozen teams.\n\n"
        ));
    }
    text
}

/// A synthetic [`crate::export::types::ExportResult`] with `size` raw bytes — large enough (with
/// base64's ~1.33× inflation) to blow [`super::super::MAX_FRAME_BYTES`] without a real Typst
/// compile, for [`success_or_capped_reply_refuses_an_oversized_export`] below.
fn oversized_result(size: usize) -> crate::export::types::ExportResult {
    crate::export::types::ExportResult {
        data: vec![0u8; size],
        mime_type: "application/pdf".to_string(),
        filename: "resume.pdf".to_string(),
        report: None,
    }
}

#[test]
fn success_or_capped_reply_refuses_an_oversized_export() {
    let result = oversized_result(super::super::MAX_FRAME_BYTES); // base64 alone already exceeds the cap
    let reply = success_or_capped_reply("req-cap", &result, "resume", "pdf", "classic");
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(
        v["payload"]["error"],
        super::super::agent_call::ERR_RESULT_TOO_LARGE
    );
    assert!(
        reply.len() <= super::super::MAX_FRAME_BYTES,
        "the refusal itself must fit the cap"
    );
}

#[test]
fn success_or_capped_reply_carries_the_real_export_bytes_when_it_fits() {
    let result = oversized_result(1024);
    let reply = success_or_capped_reply("req-small", &result, "resume", "pdf", "classic");
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["byteLength"], 1024);
    assert_eq!(v["payload"]["dataEncoding"], "base64");
}

#[tokio::test]
async fn document_export_reply_stays_well_under_the_frame_cap_for_every_template() {
    use crate::export::types::ExportRequest;

    let long_text = dense_sample_resume_text();
    let mut largest = 0usize;
    for &template_id in crate::export::templates::CANONICAL_TEMPLATE_IDS.iter() {
        for format in [ExportFormat::Pdf, ExportFormat::Docx] {
            let request = ExportRequest {
                text: long_text.clone(),
                format,
                document_type: DocumentType::Resume,
                template_id,
                meta: None,
                ats_mode: false,
                locale: None,
                contact: None,
                accent: None,
                letter_layout: LetterLayout::Classic,
            };
            let result = crate::export::commands::documents_export_document(request)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "export must succeed for template {template_id:?} format {format:?}: {e}"
                    )
                });

            // Base64 round trip: the reply's `data` field must decode back to exactly the raw
            // export bytes `documents_export_document` produced. Built through the REAL
            // `success_or_capped_reply` — not a re-implementation of its shape.
            let reply = success_or_capped_reply("req-budget", &result, "resume", "pdf", "classic");
            let v: Value = serde_json::from_str(&reply).unwrap();
            let decoded = {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(v["payload"]["data"].as_str().unwrap())
                    .expect("data must be valid base64")
            };
            assert_eq!(
                decoded, result.data,
                "base64 round trip must reproduce the raw bytes"
            );

            largest = largest.max(reply.len());
            assert!(
                reply.len() < super::super::MAX_FRAME_BYTES / 2,
                "template {template_id:?} format {format:?}: reply ({} B) exceeds 50% of the \
                 frame cap ({} B) — growth here should be caught early, before it threatens the \
                 hard cap",
                reply.len(),
                super::super::MAX_FRAME_BYTES
            );
        }
    }
    println!(
        "largest document.export reply observed: {largest} B (cap {} B)",
        super::super::MAX_FRAME_BYTES
    );
}
