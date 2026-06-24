---
name: job-match-standards
description: How real ATS (Workday/Greenhouse/Lever/Taleo/iCIMS/Ashby) parse, score and rank in 2026; evidence-based matching; and the legal limits on automated screening (EU AI Act high-risk, NYC LL144, EEOC Title VII, GDPR Art. 22). Load for changes under commands/match_resume.rs, cover_letter.rs, validate/, documents/embed.
---

# ATS scoring & job-match standards (reality, not myth)

External best-practices for ATS scoring, JD analysis, and resume↔job matching. Load with `author-contract` (job-match-author) / `token-efficiency` (job-match-expert). Pairs with `docs/knowledge/matching-algorithm.md` (the scoring kernel).

## How real ATS work (verified 2026-06)

- **No universal "ATS score."** Each platform scores differently; a single portable percentage is marketing fiction. Present our number as a _guidance estimate with evidence_, never as the employer's verdict. https://www.hireflow.net/blog/workday-vs-greenhouse-vs-lever-which-parses-best
- **Greenhouse** — structured scorecards + Boolean over parsed fields; **AI Talent Matching added Feb 2026**. **Lever** — full-text relevance + Gem _semantic_ JD understanding (not exact-keyword). **Workday** — weights **job-title/seniority match heavily** (mismatched title tanks the score). **Taleo** — strict literal keyword match. **iCIMS** — ML semantic match. **Ashby** — Boolean search; 0–100 Match Score + reason bullets only via AI add-ons.
- **Recruiter Boolean/keyword search is still the dominant filter** — candidates surface via search, not just auto-rank.
- **AI/LLM screening** — ~65% of US enterprise employers use AI-assisted screening (2025); LLM layers now score career-narrative fit + achievement quality. https://incruiter.com/blog/ai-in-recruitment-2026-trends-stats-what-works/

## Matching best-practices (what our scorer should do)

- Extract JD requirements and **classify hard (must-have/knockout) vs nice-to-have**; treat knockout/screening questions as **gating**, not weighted.
- **Normalize keywords + synonyms** (title/skill aliases, seniority mapping) — helps both literal (Taleo) and semantic (iCIMS/Lever) parsers.
- **Evidence-based scoring** — credit skills backed by experience/context, not raw frequency; **never reward keyword stuffing** (semantic + AI-content detection penalize it). https://www.jobscan.co/blog/can-ats-detect-ai-resume/
- **Explainable output** — per-requirement match + reason bullets; be honest the number is _our_ estimate.
- **Invalidate derived caches on input change** — when a posting's text changes (e.g. the full description is resolved on open), drop its cached **embedding** + any text-hash-keyed score, **and** invalidate the renderer query that reads that posting. Otherwise the next score reuses the stale snippet embedding _and_ the UI keeps showing the truncated text (#486).

## ⚠️ 2026 legal / AI constraints on automated screening — flag prominently

- **EU AI Act:** recruitment AI that sources/scores/ranks/shortlists CVs→JDs is **high-risk (Annex III)**. The legally binding high-risk deadline under **Art. 113 is still 2 Aug 2026**; a provisional May-2026 "Digital Omnibus" political agreement _would_ defer it to 2 Dec 2027 but is **not yet adopted in the Official Journal** — until formally enacted, treat **2 Aug 2026** as the binding date and advise preparing for it. Obligations: risk mgmt, human oversight, transparency, logging, conformity assessment. (Prohibited-practices + AI-literacy duties already in force since 2 Feb 2025.) https://www.gibsondunn.com/eu-ai-act-omnibus-agreement-postponed-high-risk-deadlines-and-other-key-changes/
- **NYC Local Law 144:** automated employment-decision tools need an **independent bias audit within the prior 12 months**, published, with **10-business-day candidate notice**. https://rules.cityofnewyork.us/rule/automated-employment-decision-tools-2/
- **EEOC (US):** withdrew its 2023 AI guidance (2025-01-27), but **Title VII disparate-impact liability still applies** (unintentional bias counts); four-fifths/adverse-impact validation + human oversight expected.
- **GDPR Art. 22:** no decision based **solely** on automated processing with significant effect — a glance at an AI shortlist is not "meaningful" human involvement; candidates get human review + contest rights + a right to meaningful information. https://gdprinfo.eu/gdpr-article-22-explained-automated-decision-making-profiling-and-your-rights

## Myths & mistakes — do NOT encode these

- ❌ "75% of resumes are auto-rejected by ATS" — **debunked**; traces to a 2012 sales pitch, no primary source. https://jobcannon.io/blog/ai-resume-statistics-2026
- ❌ "One ATS score works everywhere" — vendor logic differs (Workday title-weighted, Lever/iCIMS semantic, Taleo literal).
- ❌ "Keyword stuffing beats the bot" — semantic + AI-detection layers penalize it.
- ❌ "All ATS keyword-match like Taleo" — over-tuning for literal match misleads users.
- ❌ "ATS read everything" — scanned/image PDFs + graphics-heavy layouts break legacy parsers.
- ❌ "Our match % = the employer's decision" — present as a guidance estimate with caveats.
