use super::*;
use crate::export::types::{DocumentType, ExportFormat, LetterLayout};

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
        ats_view: None,
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
fn bounded_reply_reports_whether_the_reply_fits() {
    // The caller records the export's template only when it fits.
    let big = oversized_result(super::super::MAX_FRAME_BYTES);
    assert!(matches!(
        bounded_reply("r", &big, "resume", "pdf", "classic"),
        Bounded::TooLarge(_)
    ));
    let small = oversized_result(1024);
    assert!(matches!(
        bounded_reply("r", &small, "resume", "pdf", "classic"),
        Bounded::Fits(_)
    ));
}

fn parsed(format: ExportFormat) -> ParsedRequest {
    ParsedRequest {
        source: Source::Generation("https://acme.com/job".into()),
        document_type: DocumentType::Resume,
        kind_wire: "resume".into(),
        format,
        format_wire: "pdf".into(),
        template_id: crate::export::types::TemplateId::Atelier,
        template_id_wire: "atelier".into(),
        letter_layout: LetterLayout::Classic,
        ats_mode: false,
    }
}

fn finish(size: usize, format: ExportFormat, id: Option<&str>) -> (Value, Vec<(String, String)>) {
    let mut recorded = Vec::new();
    let reply = finish_export("r", &oversized_result(size), &parsed(format), id, |i, t| {
        recorded.push((i.to_string(), t.to_string()));
    });
    (serde_json::from_str(&reply).unwrap(), recorded)
}

#[test]
fn a_refused_oversized_export_records_nothing() {
    let (v, recorded) = finish(super::super::MAX_FRAME_BYTES, ExportFormat::Pdf, Some("g1"));
    assert_eq!(v["payload"]["ok"], false);
    assert!(recorded.is_empty());
}

#[test]
fn a_fitting_pdf_records_the_template_on_the_given_generation_id() {
    let (v, recorded) = finish(1024, ExportFormat::Pdf, Some("g1"));
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(recorded, [("g1".to_string(), "atelier".to_string())]);
}

#[test]
fn a_fitting_txt_or_an_export_without_a_generation_records_nothing() {
    assert!(finish(1024, ExportFormat::Txt, Some("g1")).1.is_empty());
    assert!(finish(1024, ExportFormat::Pdf, None).1.is_empty());
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

            // The wire value MUST come from the loop's own `format`, not a fixed literal — a
            // hardcoded "pdf" here would let the DOCX iteration silently carry wrong metadata
            // (and slightly understate the DOCX envelope size) without this test ever noticing.
            let format_wire = match format {
                ExportFormat::Pdf => "pdf",
                ExportFormat::Docx => "docx",
                ExportFormat::Txt => "txt",
            };

            // Base64 round trip: the reply's `data` field must decode back to exactly the raw
            // export bytes `documents_export_document` produced. Built through the REAL
            // `success_or_capped_reply` — not a re-implementation of its shape.
            let reply =
                success_or_capped_reply("req-budget", &result, "resume", format_wire, "classic");
            let v: Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(
                v["payload"]["format"], format_wire,
                "template {template_id:?} format {format:?}: the reply must echo the requested format"
            );
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
