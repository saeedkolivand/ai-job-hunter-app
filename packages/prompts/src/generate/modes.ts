/** Generation modes + metadata shape. */

import type { EmphasisId } from './emphasis.js';

export type GenerationMode =
  | 'ats' // Conservative ATS Optimization
  | 'recruiter' // Recruiter-Friendly Rewrite
  | 'technical' // Technical Role Optimization
  | 'executive' // Executive / Senior Rewrite
  | 'startup' // Startup Tone
  | 'corporate' // Corporate / Enterprise
  | 'localize'; // International Localization

export interface GenerationMeta {
  resumeLanguage: string;
  jobAdLanguage: string;
  mismatch: boolean;
  candidateName: string;
  jobTitle: string;
  companyName: string;
  targetLanguage: string;
  /** Top keywords/technologies extracted from the job ad. Used for bold emphasis. */
  topRequirements: string[];
  /**
   * Free-text job location exactly as written in the ad (e.g.
   * "Munich, Germany / remote in DE"). Empty when the ad doesn't state one.
   * Optional so existing `GenerationMeta` literals stay valid.
   */
  jobLocation?: string;
  /**
   * Normalized ISO-3166 alpha-2 country of the job (e.g. "DE"). Drives the
   * cover-letter market (decision: job country, not ad language). Empty/omitted
   * when unknown — resolution then falls back to the research brief / language.
   */
  jobCountry?: string;
  /**
   * User-selected emphasis directives (#15) — fact-safe rewrite biases applied on
   * top of the mode (e.g. quantify impact, more concise). Optional so existing
   * `GenerationMeta` literals stay valid; the wizard merges the chosen set into
   * meta just before generation.
   */
  emphasis?: EmphasisId[];
}

export const MODES: Record<
  GenerationMode,
  { label: string; description: string; toneInstruction: string }
> = {
  ats: {
    label: 'ATS Optimized',
    description: 'Maximize keyword coverage for applicant tracking systems',
    toneInstruction:
      'Optimize for ATS parsing above all else. Use exact keyword phrases from the job ad verbatim in context. Ensure standard section headers: Professional Summary, Work Experience, Education, Skills. Consistent date format throughout. Start every bullet with a strong action verb. Quantify every achievement that can be quantified.',
  },
  recruiter: {
    label: 'Recruiter-Friendly',
    description: 'Optimized for human recruiter 7-second screening',
    toneInstruction:
      'Optimize for the 7-second recruiter scan. Lead with most relevant experience. Every bullet starts with a strong action verb and ends with a measurable result. No walls of text. Professional Summary: 2-3 sentences stating seniority, domain, and value for THIS role.',
  },
  technical: {
    label: 'Technical Role',
    description: 'Highlights technical depth and engineering specifics',
    toneInstruction:
      'Lead with technical depth. Every bullet names specific technologies, architecture decisions, and scale metrics. Show system design thinking. Quantify performance improvements (latency, throughput, uptime, scale). Use precise technical vocabulary from the job ad.',
  },
  executive: {
    label: 'Executive / Senior',
    description: 'Leadership-focused, strategic and high-level',
    toneInstruction:
      'Lead with organizational impact. Every bullet answers: what changed, and what was the business outcome? Emphasize team size, budgets, revenue/cost impact, strategic initiatives. Remove tactical details. Use executive vocabulary: drove, built, transformed, scaled, led.',
  },
  startup: {
    label: 'Startup Tone',
    description: 'Modern, dynamic, growth-oriented language',
    toneInstruction:
      'Write for a startup reader who values velocity, ownership, and raw impact. Modern active language. Highlight things built from scratch, delivery speed, cross-functional ownership, growth metrics. Avoid corporate language.',
  },
  corporate: {
    label: 'Corporate / Enterprise',
    description: 'Formal, structured, compliance-ready',
    toneInstruction:
      'Formal enterprise tone. Precise and structured. Emphasize process adherence, stakeholder management, cross-functional collaboration, risk management, governance. Remove casual phrasing.',
  },
  localize: {
    label: 'Localized Output',
    description: 'Culturally adapted for the target market',
    toneInstruction:
      'Write natively in the target language. Do NOT translate literally — fully adapt for the target market. Use local resume conventions and market-expected terminology.',
  },
};
