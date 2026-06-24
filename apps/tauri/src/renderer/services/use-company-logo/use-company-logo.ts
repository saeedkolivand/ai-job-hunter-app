import { useQuery } from '@tanstack/react-query';

import { QUERY_TIMES } from '../query-client';

// Clearbit Autocomplete API — returns the first logo URL for a company name.
// Defensive contract: ANY failure (network, CORS, non-OK, empty results,
// Clearbit deprecation) → returns null, never throws.
// Makes ZERO requests when enabled=false or company is blank.
//
// ARCHITECTURE EXCEPTION (ports & adapters Rule 1):
// This hook performs a direct `fetch()` to a third-party host (Clearbit)
// from the renderer, bypassing the normal IPC path through the Rust shell.
// This is a deliberate, security-reviewed exception because:
//   - The feature is opt-in (fetchCompanyLogos preference, default OFF).
//   - It is purely decorative — logo failure is silent and non-blocking.
//   - The call site is fully defensive (try/catch, retry:0, null on any error).
//   - The CSP is scoped to only the two required Clearbit hosts.
// The future-correct upgrade path is to route this through a Rust `fetch`
// command (net::http shared client), which would also eliminate the
// `connect-src https://autocomplete.clearbit.com` CSP grant entirely.
// Do NOT add further direct renderer fetches without the same review process.

interface ClearbitSuggestion {
  name: string;
  domain: string;
  logo: string;
}

async function fetchClearbitLogo(company: string): Promise<string | null> {
  try {
    const url = `https://autocomplete.clearbit.com/v1/companies/suggest?query=${encodeURIComponent(company)}`;
    const res = await fetch(url);
    if (!res.ok) return null;
    const data: unknown = await res.json();
    if (!Array.isArray(data) || data.length === 0) return null;
    const first = data[0] as Partial<ClearbitSuggestion>;
    return typeof first.logo === 'string' && first.logo ? first.logo : null;
  } catch {
    // Network error, CORS, JSON parse failure, Clearbit deprecation — all → null.
    return null;
  }
}

/**
 * Resolve a company logo URL via Clearbit Autocomplete.
 *
 * - Returns `null` when disabled, when company is blank, or on any fetch failure.
 * - Cached for the session lifetime (INFINITE stale + gc time) — logos don't
 *   change mid-session and we never want to re-fetch on remount.
 * - retry: 0 — Clearbit errors are deterministic; one failure is the answer.
 */
export function useCompanyLogo(company: string, enabled: boolean): string | null {
  const trimmed = company.trim();
  const result = useQuery({
    queryKey: ['company-logo', trimmed] as const,
    queryFn: () => fetchClearbitLogo(trimmed),
    enabled: enabled && !!trimmed,
    staleTime: QUERY_TIMES.INFINITE,
    gcTime: QUERY_TIMES.INFINITE,
    retry: 0,
  });
  return result.data ?? null;
}
