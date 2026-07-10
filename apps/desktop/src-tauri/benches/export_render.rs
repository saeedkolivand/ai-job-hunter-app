//! Export hot-path benchmarks — the request → bytes render pipeline that backs
//! `documents_export_document` (the most performance-sensitive, user-visible
//! path in the app). Tracked over time by the advisory `benchmark.yml` workflow,
//! which alerts on regressions and keeps history on a gh-pages branch.
//!
//! These reach the engine through the public crate API (`ajh_tauri::export::…`),
//! which is why the crate exposes a library target (see `Cargo.toml` `[lib]`).
//! The in-crate test fixtures are `#[cfg(test)]` and therefore unreachable from
//! a separate bench compilation unit, so a representative fixture is inlined.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};

use ajh_tauri::export::docx::generate_docx;
use ajh_tauri::export::pdf::generate_pdf;
use ajh_tauri::export::types::{
    DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId,
};

/// Representative one-page résumé: header with contact links, summary, two
/// experience entries with bullets, education, and skills — enough to exercise
/// every block type the parser/renderer handles. Mirrors the in-crate test
/// fixture (`typst_engine::test::FIXTURE_RESUME`).
const FIXTURE_RESUME: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe | https://github.com/janedoe

SUMMARY
Experienced software engineer with a passion for building reliable systems.

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Designed distributed task scheduler reducing latency by 40 percent
- Led migration to Rust-based microservices across three product teams

Software Engineer | Beta Inc | 2018 – 2021
- Built real-time data pipeline processing one million events per day
- Mentored two junior engineers through onboarding

EDUCATION
B.Sc. Computer Science | State University | 2014 – 2018

SKILLS
Rust, Python, TypeScript, PostgreSQL, Kubernetes, AWS
";

fn request(format: ExportFormat, template_id: TemplateId) -> ExportRequest {
    ExportRequest {
        text: FIXTURE_RESUME.to_string(),
        format,
        document_type: DocumentType::Resume,
        template_id,
        meta: None,
        ats_mode: false,
        locale: Some("en".to_string()),
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    }
}

/// PDF render is the heaviest path (typst compile + PDF emit + link annotations).
/// Benchmarked across a single-column (Classic) and a two-column sidebar
/// (Atelier) template — the latter is the more expensive layout.
fn bench_pdf(c: &mut Criterion) {
    let mut group = c.benchmark_group("pdf");

    let classic = request(ExportFormat::Pdf, TemplateId::Classic);
    group.bench_function("classic", |b| {
        b.iter(|| generate_pdf(black_box(&classic)).expect("generate_pdf(classic) must succeed"));
    });

    let atelier = request(ExportFormat::Pdf, TemplateId::Atelier);
    group.bench_function("atelier_two_column", |b| {
        b.iter(|| generate_pdf(black_box(&atelier)).expect("generate_pdf(atelier) must succeed"));
    });

    group.finish();
}

/// DOCX render (docx-rs document tree → zipped .docx bytes).
fn bench_docx(c: &mut Criterion) {
    let req = request(ExportFormat::Docx, TemplateId::Classic);
    c.bench_function("docx_classic", |b| {
        b.iter(|| generate_docx(black_box(&req)).expect("generate_docx(classic) must succeed"));
    });
}

criterion_group!(benches, bench_pdf, bench_docx);
criterion_main!(benches);
