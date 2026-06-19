---
name: resume-export-standards
description: ATS-safe + accessible resume/CV export standards (PDF/DOCX) — real ATS parsing limits, single-column rules, PDF/UA accessibility, country/industry CV norms (US/UK/DE), keyword hygiene. Load for changes under export/, model/, theme/, templates/, locale/, fonts, layout/.
---

# Resume/CV export standards — ATS-safe & accessible

External best-practices for generating resumes that real ATS parse and that meet accessibility law. Load with `author-contract` (pdf-docx-generator) / `token-efficiency` (resume-export-expert). Pairs with the repo's `docs/knowledge/resume-domain.md` (the export contract).

## ATS-safe formatting (verified 2026-06)

Parsers read **linearly, top→bottom, left→right** — design for that.

- **Single-column, reverse-chronological.** Multi-column / side-by-side scrambles reading order into "word salad." https://www.jobscan.co/blog/resume-tables-columns-ats/
- **No tables/text-boxes/grids for layout.** Modern ATS tolerate a _simple_ one-cell table but tab stops are safer; skills as commas/bullets/`|`, never a grid.
- **Contact info in the BODY, never headers/footers** — ATS dropped header/footer contact info **~25%** of the time. https://www.jobscan.co/blog/ats-formatting-mistakes/
- **Standard fonts** (Arial, Calibri, Helvetica, Times New Roman, Georgia, Garamond, Cambria, Verdana), 10–12pt; custom display fonts → fallback-render breakage.
- **Standard `•`/`-` bullets only** — icon glyphs/dingbats parse as `[NULL]`; replace icons with text labels ("Phone:" not 📞).
- **No graphics/charts/logos/photos/skill-bars** (US/UK/CA — see country norms); images are invisible to parsers.
- **Standard section headings** ("Work Experience", "Education", "Skills") — creative titles ("My Journey") aren't recognized.
- **PDF vs DOCX:** text-selectable PDF now parses ≈ DOCX in modern systems (Workday/Greenhouse/Lever); DOCX safer for older/stricter ATS; default PDF unless the posting asks for Word; **never image-only/scanned PDF.** https://www.resumemate.io/blog/pdf-vs-word-for-resume-2026-which-format-ats-actually-prefers/
- **File name** `FirstName-LastName-Role.pdf` (descriptive, no spaces/special chars).
- **Dates** consistent `Mon YYYY – Mon YYYY`; no apostrophes (`Jan '21`), no year-only when months exist.
- **Keywords** mirror JD terminology _inside achievement bullets with metrics_ — don't repeat; stuffing drops scores ~30% (parsers detect repetition + understand synonyms). https://blog.theinterviewguys.com/ats-resume-optimization/

## Document accessibility (PDF/UA = ISO 14289 · WCAG 2.2 AA)

- **Tagged structure** (most important), **logical reading order**, real heading hierarchy (not visual sizing). https://accessibility.build/guides/pdf-accessibility
- Declare document **language** + **Title** metadata; **embed fonts** with ToUnicode; bookmarks for 20+ pages.
- **Alt text** on meaningful images; decorative → artifact; data tables get header-cell markup.
- **Accessible DOCX** (built-in Heading styles + real lists/tables + alt text) maps to PDF tags on export. https://daisy.org/guidance/info-help/guidance-training/content-creation/accessible-pdf/
- **2026 driver:** EU **European Accessibility Act** applied **2025-06-28**, enforcement ramping through 2026 (EN 301 549 → WCAG 2.1 AA + PDF/UA). https://www.levelaccess.com/compliance-overview/european-accessibility-act-eaa/

## Country / industry CV norms (2026)

| Element       | US / UK / CA / AU              | Germany / EU                               |
| ------------- | ------------------------------ | ------------------------------------------ |
| Photo         | **Omit**                       | **Expected** (formal headshot, top-right)  |
| Date of birth | **Omit** (anti-discrimination) | Often included                             |
| Address       | City/country only              | Full address standard                      |
| Length        | 1–2 pages                      | 1–2 pages (Lebenslauf), often signed/dated |

- German AGG means a photo/DOB **cannot be legally required**, yet most DE employers still expect a photo. https://www.topcv.io/blog/cv-for-germany-complete-guide-2026
- Creative/design roles tolerate portfolios + visuals; finance/legal/gov/academic and any ATS-screened pipeline demand the plain single-column rules.

## Common ATS-failure mistakes (quick reject list)

1. Multi-column/table layout → scrambled reading order. 2. Contact info in header/footer → ~25% dropped. 3. Icons/photos/skill-bars/charts → invisible/`[NULL]`. 4. Custom display fonts → fallback breakage. 5. Image-only/scanned PDF → zero text. 6. Creative section headings → section unrecognized. 7. Inconsistent/apostrophe/year-only dates → misparsed tenure. 8. Keyword stuffing → ~30% penalty.

> **Take:** ATS-safe and accessible are the **same** single-column, tagged, text-first artifact — they reinforce, not conflict. (Vendors don't publish parser specs, so "simple tables parse OK" is the weakest claim — hedge it.)
