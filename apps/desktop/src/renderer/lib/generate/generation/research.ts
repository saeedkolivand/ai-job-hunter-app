import type { SalaryRange } from '@ajh/prompts/generate';

import { getClient } from '../../app-client';
import { resolveActiveProvider } from '../provider-context';

// Routing (provider/model/base_url) is backend-owned (task #16) — the enrichers
// read the active provider from the store, so nothing is threaded here. Every
// function below is best-effort: any failure or a provider that can't search
// degrades to an empty result and must never block or fail generation.

/**
 * Best-effort company research for the cover-letter "fit" paragraph. Routes
 * through the backend enricher — the active provider's own web search +
 * synthesis, cached. The returned brief is untrusted reference text — the
 * prompt fences it.
 */
export async function researchCompany(
  jobAd: string,
  company?: string,
  role?: string
): Promise<string> {
  try {
    const res = await getClient().ai.researchCompany({
      jobAd,
      // The AI-extracted company name is far more reliable than the backend's
      // heuristic job-ad scan (which can grab a tagline), so send it when known.
      company: company?.trim() || undefined,
      // Same reasoning: the heuristic falls back to the ad's first short line,
      // which on a scraped page is an apply button.
      role: role?.trim() || undefined,
      // Same effort the generation request carries — the backend's deadline
      // around search + synthesis scales with it. Without this a reasoning model
      // never finishes research inside the flat bound, and the cover letter is
      // written with no company knowledge and no visible failure.
      effort: resolveActiveProvider().providerSettings?.effort,
    });
    return res?.brief ?? '';
  } catch {
    return '';
  }
}

/**
 * Per-question web-search reference notes for an application answer — opt-in
 * sibling of {@link researchCompany}, scoped to a single question's topic
 * (combined with the role + company for relevance).
 */
export async function researchAnswer(
  question: string,
  role: string,
  company: string
): Promise<string> {
  try {
    const res = await getClient().ai.researchAnswer({
      question,
      role: role.trim() || undefined,
      company: company.trim() || undefined,
    });
    return res ?? '';
  } catch {
    return '';
  }
}

/**
 * Web-grounded market salary-range lookup for the salary application question
 * (C2) — the active provider's own web search, validated and cached. Yields
 * `undefined` on any failure so the salary answer falls back to the C1
 * applicant-preference-only grounding.
 */
export async function lookupSalaryRange(
  role: string,
  company: string,
  location: string,
  /** ISO-3166 alpha-2 job country, when known — grounds the researched currency. */
  country?: string,
  /** Authoritative ISO-4217 currency for `country` (resolve via `countryToCurrency`
   *  from `@ajh/prompts/generate`); omitted falls back to today's unconstrained
   *  "local currency for that location" behavior. */
  currency?: string
): Promise<SalaryRange | undefined> {
  try {
    const res = await getClient().ai.lookupSalary({
      role,
      company: company.trim() || undefined,
      location: location.trim() || undefined,
      country: country?.trim() || undefined,
      currency: currency?.trim() || undefined,
      effort: resolveActiveProvider().providerSettings?.effort,
    });
    return res ?? undefined;
  } catch {
    return undefined;
  }
}
