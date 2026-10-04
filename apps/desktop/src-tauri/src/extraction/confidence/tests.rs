use super::*;

fn resume_like() -> &'static str {
    "Jane Doe  jane.doe@example.com  +31 6 12345678\n\
         Summary\nExperienced software engineer with 8 years of experience.\n\
         Experience\nSenior Engineer at Acme Corp 2020-2025\n\
         Education\nBSc Computer Science, University of Amsterdam 2016\n\
         Skills\nRust, Python, TypeScript, SQL\n\
         Languages\nEnglish (fluent), Dutch (intermediate)"
}

#[test]
fn high_for_rich_direct_pdf() {
    assert_eq!(
        score(resume_like(), SourceFormat::PdfText),
        Confidence::High
    );
}

#[test]
fn medium_for_ocr_source() {
    // Same content but via OCR source — base starts at Medium.
    let c = score(resume_like(), SourceFormat::PdfScanned);
    assert!(matches!(c, Confidence::Medium | Confidence::High));
}

#[test]
fn low_for_empty() {
    assert_eq!(score("", SourceFormat::PdfText), Confidence::Low);
}

#[test]
fn low_for_garbage() {
    let garbage = "§§§ ¶¶¶ ©©©ˆˆˆ ≈≈≈ ∆∆∆ ∑∑∑".repeat(20);
    assert_eq!(score(&garbage, SourceFormat::PdfText), Confidence::Low);
}
