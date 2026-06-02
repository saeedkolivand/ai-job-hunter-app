// Illustrative, generic samples for the AI-Generate option previews.
//
// These are FIXED examples for a fictional candidate ("Jordan Avery") — never
// built from the user's input. They exist so a user can see what each option
// does to the *end result* before spending tokens on a real generation:
//   • styles/tones, document target, and prompt-quality → sample wording (here)
//   • templates → a rendered page image (see ./template-previews) + a caption
//
// Bodies are Markdown, rendered through the same `MarkdownMessage` the finished
// output uses, so a sample reads like a real result. English-only for now
// (illustrative); per-locale sample text is a deliberate follow-up.

import type { GenerationMode, TemplateId } from '@/lib/generate';
import type { PromptQuality } from '@/store/preferences-schema';

/** Which option group a preview is for. */
export type PreviewGroup = 'template' | 'style' | 'target' | 'quality';

/** The option currently being previewed in the result panel. */
export interface PreviewFocus {
  group: PreviewGroup;
  id: string;
}

export type GenTarget = 'resume' | 'cover' | 'both';

// ── Style / tone (the 7 generation modes) ────────────────────────────────────
// Same fictional candidate + role, rewritten so each mode's voice is visibly
// different. Keyed by GenerationMode so the set stays in lock-step with MODES.

export const STYLE_SAMPLES: Record<GenerationMode, string> = {
  ats: `**Senior Backend Engineer**

Backend engineer with 8+ years building **Node.js**, **PostgreSQL**, and **AWS** services. Proven record of **CI/CD** automation, **REST API** design, and query optimization.

- Reduced deployment time 60% by automating CI/CD pipelines with GitHub Actions and Docker.
- Optimized PostgreSQL queries, cutting p95 API latency from 800 ms to 120 ms.`,

  recruiter: `**Senior Backend Engineer**

Backend engineer who ships reliable services at scale and mentors the people around them — ready to own a payments platform end to end.

- Led a 5-engineer team that delivered a new billing service 3 weeks ahead of schedule.
- Cut infrastructure costs 35% while traffic doubled — clear, measurable impact.`,

  technical: `**Senior Backend Engineer**

Distributed-systems engineer focused on throughput, consistency, and observability.

- Architected an event-driven order pipeline (Kafka, gRPC, Postgres) sustaining 12k req/s with zero downtime through a 10× traffic spike.
- Added connection pooling and read replicas, dropping p95 query latency from 800 ms to 120 ms.`,

  executive: `**Engineering Leader**

Engineering leader who turns infrastructure investment into business outcomes.

- Led a cloud migration that reduced deployment time 75%, enabling 3× faster feature velocity across 40+ engineers.
- Owned a $2M infrastructure budget; cut annual cloud spend 35% while supporting Series B growth.`,

  startup: `**Founding Backend Engineer**

Builder who ships fast and owns the whole stack — from first commit to production.

- Built the payments backend from scratch and shipped it to production in 6 weeks.
- Held reliability solo through a 10× launch spike: 99.99% uptime for 3 months straight.`,

  corporate: `**Senior Backend Engineer**

Backend engineer experienced in regulated, enterprise-scale delivery and cross-functional governance.

- Partnered with risk and compliance stakeholders to deliver an audited billing platform under SOC 2 controls.
- Standardized release governance across 6 teams, improving change-failure rate by 40%.`,

  localize: `**Ingénieur Backend Senior**  *(example — localized to French)*

Ingénieur backend avec 8 ans d'expérience sur des systèmes Node.js et PostgreSQL à grande échelle.

- Réduction du temps de déploiement de 60 % grâce à l'automatisation CI/CD.
- Optimisation des requêtes PostgreSQL : latence p95 ramenée de 800 ms à 120 ms.`,
};

// ── Document target (what to generate) ───────────────────────────────────────

export const TARGET_SAMPLES: Record<GenTarget, string> = {
  resume: `**Jordan Avery** — Senior Backend Engineer

**Summary**
Backend engineer with 8+ years building reliable, high-throughput services.

**Experience — Apex Technologies**
- Reduced deployment time 60% by automating CI/CD pipelines.
- Cut p95 API latency from 800 ms to 120 ms via query optimization.`,

  cover: `Dear Hiring Manager,

I'm excited to apply for the Senior Backend Engineer role at Northwind. Over the past 8 years I've built payment and billing systems that stayed up through 10× traffic spikes — exactly the reliability your team is scaling toward.

In my current role I led a migration that cut deployment time 75% and freed engineers to ship 3× faster…`,

  both: `**You'll get both, tailored to this job:**

- A one-page **résumé** rewritten around the job's keywords and your real experience.
- A matching **cover letter** in the same voice, ready to send.

Each is generated separately, so you can copy, edit, and export them independently.`,
};

// ── Prompt quality (depth / detail) ──────────────────────────────────────────
// The SAME bullet at three depths, so the trade-off is obvious.

export const QUALITY_SAMPLES: Record<PromptQuality, string> = {
  compact: `**Fast** — quick, lean output. Best for small or local models.

- Automated CI/CD; cut deployment time 60%.`,

  auto: `**Auto** — balanced depth, chosen for your model. Recommended.

- Automated CI/CD pipelines with GitHub Actions and Docker, reducing deployment time 60%.`,

  full: `**Full** — maximum detail and rewrites. Best on larger models.

- Re-architected the release pipeline with GitHub Actions and Docker — parallel test stages and blue-green deploys cut deployment time 60% (22 → 9 min) and eliminated release-night rollbacks.`,
};

// ── Template captions ────────────────────────────────────────────────────────
// One-line "best for" shown under each template image. Kept here (not in
// templates.ts) to stay additive — templates.ts is render metadata only.

export const TEMPLATE_CAPTIONS: Record<TemplateId, string> = {
  classic: 'Maximum ATS safety — single column, no color. Safe for every parser.',
  modern: 'Clean navy, single column. A strong default for software & engineering roles.',
  'swiss-minimal': 'Minimalist Manrope with a red accent. Design-adjacent and product roles.',
  academic: 'Serif throughout with ruled headings. Academia, research, and publications.',
  atelier: 'Premium two-column sidebar. Skills-forward; collapses to single column for ATS.',
  meridian: 'Header-forward tinted band, copper accent. Airy, modern professional.',
  throughline: 'Vertical timeline spine. Engineering & product careers with a clear arc.',
  portrait: 'Photo header, two columns. European market and personal-brand résumés.',
  lebenslauf: 'DIN-style tabular CV with photo. German-speaking (DACH) market standard.',
};
