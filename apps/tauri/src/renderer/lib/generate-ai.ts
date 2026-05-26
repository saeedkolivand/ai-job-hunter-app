/**
 * AI generation pipeline for Resume + Cover Letter.
 *
 * Pipeline:
 * 1. Extract metadata (JSON — name, role, company, languages, keywords)
 * 2. Generate resume  (streamed text with **keyword** bold markers)
 * 3. Generate cover letter (streamed text with **keyword** bold markers)
 * 4. Export — three professional templates, real bold in DOCX + PDF
 *
 * Bold emphasis:
 *   AI outputs **keyword** notation → parseInlineMd() splits into segments
 *   → mdRunsDocx() creates real TextRun({ bold:true }) in DOCX
 *   → drawMixedText() renders font-weight changes mid-line in PDF
 *   The user never sees literal asterisks.
 */

import {
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildLeakageValidatorPrompt,
  buildMetadataPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  type GenerationMode,
  parseLeakageResult,
  validateMetadata,
} from '@ajh/prompts/generate';
import { detectLanguages } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from './app-client';

export type { GenerationMeta, GenerationMode };
export { MODES } from '@ajh/prompts/generate';

// ─── LLM helpers ─────────────────────────────────────────────────────────────

const VALID_LOCALES = ['en', 'de', 'fr', 'es', 'it', 'tr', 'pt', 'ru', 'zh', 'ja', 'ko'] as const;
type SupportedLocale = (typeof VALID_LOCALES)[number];

function safeLocale(lng: string): SupportedLocale {
  return VALID_LOCALES.includes(lng as SupportedLocale) ? (lng as SupportedLocale) : 'en';
}

async function streamGenerate(
  model: string,
  system: string,
  user: string,
  onToken: (tok: string) => void,
  temperature = 0.3,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const api = getClient();
  const storeState = usePreferencesStore.getState();
  const providerConfig = storeState.aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const providerSettings = providerConfig?.providers?.[activeProvider];
  const activeModel = providerSettings?.model || model;
  const res = (await api.ai.generate({
    model: activeModel,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    locale: safeLocale(locale),
    temperature,
    ...(activeProvider !== 'ollama'
      ? { provider: activeProvider, baseUrl: providerSettings?.baseUrl }
      : {}),
  } as Parameters<typeof api.ai.generate>[0])) as { jobId: string };

  const jobId = res.jobId;
  let buffer = '';

  // Tracks whether we're inside an inline <think>...</think> block emitted
  // token-by-token by local reasoning models (DeepSeek, Qwen, etc.)
  let inThinkBlock = false;
  let thinkAccum = '';

  return new Promise((resolve, reject) => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let poll: ReturnType<typeof setInterval> | null = null;
    let abortListener: (() => void) | null = null;

    const cleanup = () => {
      if (timeoutId !== null) clearTimeout(timeoutId);
      if (poll !== null) clearInterval(poll);
      if (abortListener && signal) signal.removeEventListener('abort', abortListener);
    };

    const off = api.ai.onStream((chunk: unknown) => {
      const c = chunk as { jobId: string; delta: string; done: boolean; thinking?: boolean };
      if (c.jobId !== jobId) return;

      if (c.delta) {
        if (c.thinking) {
          // Anthropic-style separate thinking flag
          onThinking?.(c.delta);
        } else {
          // Accumulate to detect inline <think> tags from local models
          thinkAccum += c.delta;

          // Flush any complete non-thinking content from the accumulator
          let out = '';
          let remaining = thinkAccum;

          while (remaining.length > 0) {
            if (inThinkBlock) {
              const closeIdx = remaining.indexOf('</think>');
              if (closeIdx !== -1) {
                onThinking?.(remaining.slice(0, closeIdx));
                inThinkBlock = false;
                remaining = remaining.slice(closeIdx + 8);
              } else {
                onThinking?.(remaining);
                remaining = '';
              }
            } else {
              const openIdx = remaining.indexOf('<think>');
              if (openIdx !== -1) {
                out += remaining.slice(0, openIdx);
                inThinkBlock = true;
                remaining = remaining.slice(openIdx + 7);
              } else {
                const holdBack = 7;
                if (remaining.length > holdBack) {
                  out += remaining.slice(0, remaining.length - holdBack);
                  remaining = remaining.slice(remaining.length - holdBack);
                }
                break;
              }
            }
          }

          thinkAccum = remaining;

          if (out) {
            buffer += out;
            onToken(out);
          }
        }
      }
      if (c.done) {
        console.debug(
          `[stream:${jobId}] DONE — chunks=${chunkCount} bufferLen=${buffer.length} inThinkBlock=${inThinkBlock} thinkAccumLen=${thinkAccum.length}`
        );
        if (thinkAccum) {
          if (!inThinkBlock) {
            // Trailing non-think content that wasn't flushed
            buffer += thinkAccum;
            onToken(thinkAccum);
          } else if (buffer.length === 0) {
            // Model wrapped everything in <think> without closing the tag.
            // Strip think markers and use as actual output.
            const rescued = thinkAccum.replace(/<\/?think>/g, '').trim();
            if (rescued) {
              console.debug(
                `[stream:${jobId}] rescued ${rescued.length} chars from unclosed think block`
              );
              buffer = rescued;
              onToken(rescued);
            }
          }
        }
        off();
        cleanup();
        resolve(buffer);
      }
    });

    // Handle abort signal
    if (signal) {
      abortListener = () => {
        off();
        void api.jobs.cancel(jobId);
        cleanup();
        reject(new Error('Generation cancelled'));
      };
      signal.addEventListener('abort', abortListener);
    }

    timeoutId = setTimeout(
      () => {
        off();
        cleanup();
        resolve(buffer);
      },
      5 * 60 * 1000
    );

    poll = setInterval(() => {
      void (async () => {
        const job = (await api.jobs.get(jobId).catch(() => null)) as {
          status: string;
        } | null;
        if (job?.status === 'failed' || job?.status === 'cancelled') {
          off();
          cleanup();
          reject(new Error(`Generation ${job.status}. Please try again.`));
        }
        if (job?.status === 'completed') {
          off();
          cleanup();
          resolve(buffer);
        }
      })();
    }, 3_000);
  });
}

// ─── Leakage validation ───────────────────────────────────────────────────────

async function runLeakageCheck(
  resume: string,
  jobAd: string,
  generated: string,
  model: string,
  locale: string
) {
  const { system, user } = buildLeakageValidatorPrompt(resume, jobAd, generated);
  try {
    const raw = await streamGenerate(model, system, user, () => {}, 0.0, locale);
    return parseLeakageResult(raw);
  } catch {
    return null;
  }
}

// ─── Generation steps ─────────────────────────────────────────────────────────

export async function extractMetadata(
  resume: string,
  jobAd: string,
  model: string,
  locale = 'en'
): Promise<GenerationMeta> {
  // Detect languages client-side
  const clientSideDetection = detectLanguages(resume, jobAd);

  const { system, user } = buildMetadataPrompt(resume, jobAd);
  try {
    const raw = await streamGenerate(model, system, user, () => {}, 0.1, locale);
    const meta = validateMetadata(raw);
    if (meta) {
      // Override with client-side detection
      return {
        ...meta,
        resumeLanguage: clientSideDetection.resumeName,
        jobAdLanguage: clientSideDetection.jobAdName,
        mismatch: clientSideDetection.mismatch,
      };
    }
  } catch {
    /* fall through */
  }

  const nameMatch = resume.match(/^([A-Z][a-z]+ [A-Z][a-z]+(?:\s[A-Z][a-z]+)?)/m);
  const titleMatch = jobAd.match(/(?:position|role|title|job)[:\s]+([^\n]+)/i);
  const companyMatch = jobAd.match(/(?:at|@|company|employer|firm)[:\s]+([^\n,]+)/i);
  return {
    candidateName: nameMatch?.[1] ?? '',
    jobTitle: titleMatch?.[1]?.trim() ?? '',
    companyName: companyMatch?.[1]?.trim() ?? '',
    resumeLanguage: clientSideDetection.resumeName,
    jobAdLanguage: clientSideDetection.jobAdName,
    mismatch: clientSideDetection.mismatch,
    targetLanguage: clientSideDetection.resumeName,
    topRequirements: [],
  };
}

export async function generateResume(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const system = buildResumeSystemPrompt(mode);
  const user = buildResumePrompt(resume, jobAd, meta, mode);
  let raw = await streamGenerate(model, system, user, onToken, 0.25, locale, signal, onThinking);
  let result = extractPlainText(raw);

  const { enableLeakageCheck } = usePreferencesStore.getState();
  if (enableLeakageCheck ?? true) {
    for (let attempt = 0; attempt < 2; attempt++) {
      const check = await runLeakageCheck(resume, jobAd, result, model, locale);
      if (!check || check.verdict === 'PASS') break;
      raw = await streamGenerate(model, system, user, onToken, 0.25, locale, signal, onThinking);
      result = extractPlainText(raw);
    }
  }
  return result;
}

export async function generateCoverLetter(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const system = buildCoverLetterSystemPrompt(mode);
  const user = buildCoverLetterPrompt(resume, jobAd, meta, mode);
  let raw = await streamGenerate(model, system, user, onToken, 0.4, locale, signal, onThinking);
  let result = extractPlainText(raw);

  const { enableLeakageCheck } = usePreferencesStore.getState();
  if (enableLeakageCheck ?? true) {
    for (let attempt = 0; attempt < 2; attempt++) {
      const check = await runLeakageCheck(resume, jobAd, result, model, locale);
      if (!check || check.verdict === 'PASS') break;
      raw = await streamGenerate(model, system, user, onToken, 0.4, locale, signal, onThinking);
      result = extractPlainText(raw);
    }
  }
  return result;
}

// ─── Filename ─────────────────────────────────────────────────────────────────

function sanitize(str: string): string {
  return str
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-zA-Z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
    .replace(/-{2,}/g, '-')
    .slice(0, 40);
}

export function buildFilename(
  meta: GenerationMeta,
  type: 'resume' | 'cover-letter',
  ext: 'pdf' | 'docx' | 'txt'
): string {
  const name = sanitize(meta.candidateName) || 'Candidate';
  const role = sanitize(meta.jobTitle) || 'Role';
  const company = sanitize(meta.companyName) || 'Company';
  return `${name}-${role}-${company}-${type}.${ext}`;
}

// ─── Inline markdown parser ───────────────────────────────────────────────────
// Converts "Use **React** and **TypeScript** here" into typed segments.

interface MdSegment {
  text: string;
  bold: boolean;
}

function parseInlineMd(line: string): MdSegment[] {
  // Handle edge cases: malformed markers, nested markers, escaped asterisks
  const segments: MdSegment[] = [];
  let current = '';
  let inBold = false;
  let i = 0;

  while (i < line.length) {
    // Check for ** (bold marker)
    if (i < line.length - 1 && line[i] === '*' && line[i + 1] === '*') {
      // Save current segment if any
      if (current) {
        segments.push({ text: current, bold: inBold });
        current = '';
      }
      // Toggle bold state
      inBold = !inBold;
      i += 2;
    } else {
      current += line[i];
      i++;
    }
  }

  // Save final segment
  if (current) {
    segments.push({ text: current, bold: inBold });
  }

  return segments.length ? segments : [{ text: line, bold: false }];
}

/** Strip **markers** from text for contexts that don't support bold (TXT). */
function stripMd(text: string): string {
  return text.replace(/\*\*([^*]+)\*\*/g, '$1');
}

// ─── Text structure parser ────────────────────────────────────────────────────

type LineKind =
  | 'name'
  | 'contact'
  | 'sectionHeader'
  | 'jobEntry'
  | 'jobTitle'
  | 'bullet'
  | 'text'
  | 'blank';

interface ParsedLine {
  kind: LineKind;
  raw: string; // original (may contain **bold**)
  text: string; // stripped text
  rightText?: string; // for jobEntry: date portion (never bold)
  segments: MdSegment[]; // parsed inline for left text
}

const SECTION_NAMES = new Set([
  'professional summary',
  'summary',
  'profile',
  'objective',
  'about',
  'work experience',
  'experience',
  'employment',
  'employment history',
  'career history',
  'education',
  'academic background',
  'academic history',
  'skills',
  'technical skills',
  'core competencies',
  'key skills',
  'competencies',
  'certifications',
  'licenses',
  'credentials',
  'certifications & training',
  'languages',
  'additional languages',
  'projects',
  'key projects',
  'notable projects',
  'side projects',
  'achievements',
  'awards',
  'honors',
  'accomplishments',
  'publications',
  'volunteer',
  'volunteering',
  'community',
  // German
  'berufserfahrung',
  'arbeitserfahrung',
  'ausbildung',
  'bildung',
  'fähigkeiten',
  'kenntnisse',
  'kompetenzen',
  'sprachen',
  'zusammenfassung',
  'profil',
  // French
  'expérience professionnelle',
  'formation',
  'compétences',
]);

const DATE_RE =
  /\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d\d|20\d\d)[\s\S]{0,30}?(?:Present|Current|Now|Heute|Ongoing|Actuel|20\d\d|19\d\d)\b/i;

// Common company/role keywords that should NOT be treated as section headers even if all caps
const COMPANY_KEYWORDS = new Set([
  'NASA',
  'IBM',
  'AWS',
  'GCP',
  'USA',
  'UK',
  'EU',
  'CEO',
  'CTO',
  'VP',
  'SVP',
  'ENGINEER',
  'DEVELOPER',
  'MANAGER',
  'DIRECTOR',
  'LEAD',
  'SENIOR',
  'SR',
  'JUNIOR',
  'JR',
  'STAFF',
  'PRINCIPAL',
  'ARCHITECT',
  'ANALYST',
  'CONSULTANT',
  'IT',
  'AI',
  'ML',
  'UI',
  'UX',
  'API',
  'REST',
  'SaaS',
  'B2B',
  'B2C',
  'HR',
]);

// Check if all-caps text is likely a company/role, not a section header
function isLikelyCompanyOrRole(text: string): boolean {
  const words = text.split(/\s+/);
  return words.some((word) => COMPANY_KEYWORDS.has(word));
}

function parseLine(raw: string, idx: number, all: string[]): ParsedLine {
  const trimmed = raw.trim();
  const clean = stripMd(trimmed);
  const segs = parseInlineMd(trimmed);

  const blank = (): ParsedLine => ({ kind: 'blank', raw: '', text: '', segments: [] });
  const make = (kind: LineKind, rightText?: string): ParsedLine => ({
    kind,
    raw: trimmed,
    text: clean,
    segments: segs,
    rightText,
  });

  if (!clean) return blank();

  const lower = clean.toLowerCase();

  // First line is name ONLY if it doesn't look like a section header or contact
  if (idx === 0) {
    // Skip if it's a known section header
    if (SECTION_NAMES.has(lower)) {
      return make('sectionHeader');
    }
    // Skip if it looks like contact info
    if (clean.includes('@') || /\+?\d[\d\s\-().]{7,}/.test(clean)) {
      return make('contact');
    }
    return make('name');
  }

  // Bullet - improved detection for various bullet styles and numbered lists
  const bulletMatch = clean.match(/^([•\-–*·▪▸►✓✔○●◆◇■□▹▸]|\d+\.|[a-z]\))\s+(.+)$/i);
  if (bulletMatch && bulletMatch[2]) {
    const bulletText = trimmed.replace(/^([•\-–*·▪▸►✓✔○●◆◇■□▹▸]|\d+\.|[a-z]\))\s*/, '');
    return {
      kind: 'bullet',
      raw: bulletText,
      text: stripMd(bulletText),
      segments: parseInlineMd(bulletText),
    };
  }

  // Tab-indented bullet (common in copy-paste)
  if (/^\t+/.test(raw) && clean.length > 5 && !SECTION_NAMES.has(lower)) {
    const bulletText = trimmed;
    return {
      kind: 'bullet',
      raw: bulletText,
      text: stripMd(bulletText),
      segments: parseInlineMd(bulletText),
    };
  }

  // Section header: known name OR all-caps short line (but NOT company/role names)
  if (SECTION_NAMES.has(lower)) {
    return make('sectionHeader');
  }

  // All-caps detection - but exclude company names and roles
  if (
    clean === clean.toUpperCase() &&
    clean.length >= 4 &&
    clean.length <= 60 &&
    /[A-ZÄÖÜ]{2,}/.test(clean) &&
    !/\d{4}/.test(clean) && // No years
    !isLikelyCompanyOrRole(clean) && // Not a company/role
    !DATE_RE.test(clean) && // Not a date
    !clean.includes('@') // Not email
  ) {
    return make('sectionHeader');
  }

  // Job entry: 2+ spaces gap before a date range (more lenient)
  const gapMatch = clean.match(/^(.+?)\s{2,}(.+)$/);
  if (gapMatch && gapMatch[1] && gapMatch[2] && DATE_RE.test(gapMatch[2])) {
    // Make sure left side is substantial (not just a word)
    if (gapMatch[1].trim().split(/\s+/).length >= 2 || gapMatch[1].length > 10) {
      return {
        kind: 'jobEntry',
        raw: trimmed,
        text: gapMatch[1].trim(),
        segments: parseInlineMd(trimmed.split(/\s{2,}/)[0] ?? trimmed),
        rightText: gapMatch[2].trim(),
      };
    }
  }
  // Line that ends with a date range
  const endDate = clean.match(
    /^(.*?)\s+((?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec|19\d\d|20\d\d).*(?:Present|Current|20\d\d|19\d\d))$/i
  );
  if (endDate && endDate[1] && endDate[2] && endDate[1].length > 3) {
    return {
      kind: 'jobEntry',
      raw: trimmed,
      text: endDate[1].trim(),
      segments: parseInlineMd(endDate[1].trim()),
      rightText: endDate[2].trim(),
    };
  }

  // Contact: has @ or phone or pipe separators or URLs
  if (
    clean.includes('@') ||
    /\+?\d[\d\s\-().]{7,}/.test(clean) ||
    clean.split(/[|·•]/).length >= 3 || // At least 3 parts separated by pipes
    /linkedin\.com|github\.com|portfolio|website/i.test(clean) ||
    /^https?:\/\//i.test(clean)
  ) {
    return make('contact');
  }

  // Job title: short non-bullet line immediately after a jobEntry
  const prevClean = stripMd(all[idx - 1]?.trim() ?? '');
  const prevIsEntry = (() => {
    const g = prevClean.match(/^(.+?)\s{3,}(.+)$/);
    return (g && g[2] && DATE_RE.test(g[2])) || DATE_RE.test(prevClean);
  })();
  if (prevIsEntry && clean.length < 100) return make('jobTitle');

  return make('text');
}

export function parseDocument(text: string): ParsedLine[] {
  const lines = text.split('\n');
  return lines.map((l, i) => parseLine(l, i, lines));
}

// ─── Template system ──────────────────────────────────────────────────────────

export type TemplateId =
  | 'classic'
  | 'modern'
  | 'executive'
  | 'editorial-serif'
  | 'swiss-minimal'
  | 'two-column'
  | 'mono-technical'
  | 'refined-executive'
  | 'academic';

interface DocTemplate {
  id: TemplateId;
  name: string;
  // Colors (hex, no #)
  nameColor: string;
  sectionColor: string;
  accentColor: string;
  bodyColor: string;
  dateColor: string;
  emphasisColor: string;
  ruleColor: string;
  // Sizes (pt)
  namePt: number;
  sectionPt: number;
  bodyPt: number;
  // DOCX layout
  marginIn: number;
  lineSpacingDocx: number;
  sectionSpacingBefore: number;
  // Style flags
  nameCentered: boolean;
  sectionAllCaps: boolean;
  sectionStyle: 'ruled-bottom' | 'underline' | 'bold-only';
}

export const TEMPLATES: Record<TemplateId, DocTemplate> = {
  /** ATS Classic — maximum compatibility, no color, safe for all ATS parsers */
  classic: {
    id: 'classic',
    name: 'ATS Classic',
    nameColor: '111111',
    sectionColor: '111111',
    accentColor: '222222',
    bodyColor: '222222',
    dateColor: '555555',
    emphasisColor: '000000',
    ruleColor: 'AAAAAA',
    namePt: 20,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 1.0,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'underline',
  },

  /** Modern Technical — clean navy, professional, best for tech roles */
  modern: {
    id: 'modern',
    name: 'Modern Technical',
    nameColor: '0D1F3C',
    sectionColor: '0D1F3C',
    accentColor: '1A3A6B',
    bodyColor: '1A1A2E',
    dateColor: '6B6B8A',
    emphasisColor: '0D3D6B',
    ruleColor: 'B8C4DC',
    namePt: 22,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 1.0,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Executive — minimalist, charcoal, premium whitespace for senior roles */
  executive: {
    id: 'executive',
    name: 'Executive',
    nameColor: '1C1C1C',
    sectionColor: '2C2C2C',
    accentColor: '444444',
    bodyColor: '2C2C2C',
    dateColor: '808080',
    emphasisColor: '1C1C1C',
    ruleColor: 'CCCCCC',
    namePt: 24,
    sectionPt: 10.5,
    bodyPt: 10.5,
    marginIn: 1.1,
    lineSpacingDocx: 288,
    sectionSpacingBefore: 300,
    nameCentered: true,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Editorial Serif — Source Serif 4 + Inter, deep indigo accent, NYT op-ed character */
  'editorial-serif': {
    id: 'editorial-serif',
    name: 'Editorial Serif',
    nameColor: '1A1A1A',
    sectionColor: '2D2B55',
    accentColor: '2D2B55',
    bodyColor: '1A1A1A',
    dateColor: '5A5A5A',
    emphasisColor: '2D2B55',
    ruleColor: '2D2B55',
    namePt: 22,
    sectionPt: 11,
    bodyPt: 11,
    marginIn: 1.0,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Swiss Minimal — Manrope, red accent, clean whitespace */
  'swiss-minimal': {
    id: 'swiss-minimal',
    name: 'Swiss Minimal',
    nameColor: '141414',
    sectionColor: '141414',
    accentColor: 'E63946',
    bodyColor: '282828',
    dateColor: '787878',
    emphasisColor: '141414',
    ruleColor: 'E63946',
    namePt: 22,
    sectionPt: 10.5,
    bodyPt: 10.5,
    marginIn: 1.15,
    lineSpacingDocx: 299,
    sectionSpacingBefore: 320,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'bold-only',
  },

  /** Two Column — Inter, light sidebar tint */
  'two-column': {
    id: 'two-column',
    name: 'Two Column',
    nameColor: '141414',
    sectionColor: '1E40AF',
    accentColor: '1E40AF',
    bodyColor: '1E1E1E',
    dateColor: '646478',
    emphasisColor: '1E40AF',
    ruleColor: 'B4C8F0',
    namePt: 22,
    sectionPt: 10.5,
    bodyPt: 10,
    marginIn: 0.5,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 200,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'bold-only',
  },

  /** Mono Technical — JetBrains Mono headings, Inter body, cyan accent */
  'mono-technical': {
    id: 'mono-technical',
    name: 'Mono Technical',
    nameColor: '0A0A0A',
    sectionColor: '0096B4',
    accentColor: '00B4D8',
    bodyColor: '1E1E1E',
    dateColor: '647882',
    emphasisColor: '0096B4',
    ruleColor: '00B4D8',
    namePt: 20,
    sectionPt: 10.5,
    bodyPt: 10.5,
    marginIn: 1.0,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Refined Executive — Playfair Display name, warm gold accent */
  'refined-executive': {
    id: 'refined-executive',
    name: 'Refined Executive',
    nameColor: '141414',
    sectionColor: '645032',
    accentColor: '8B7355',
    bodyColor: '282623',
    dateColor: '78695F',
    emphasisColor: '645032',
    ruleColor: 'C8B9A0',
    namePt: 26,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 1.1,
    lineSpacingDocx: 288,
    sectionSpacingBefore: 300,
    nameCentered: true,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Academic — Source Serif 4 throughout, forest green accent */
  academic: {
    id: 'academic',
    name: 'Academic',
    nameColor: '141E1E',
    sectionColor: '1B4332',
    accentColor: '1B4332',
    bodyColor: '1E1E1E',
    dateColor: '5A6E64',
    emphasisColor: '1B4332',
    ruleColor: '649678',
    namePt: 20,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.85,
    lineSpacingDocx: 252,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'underline',
  },
};

// ─── PDF helpers ──────────────────────────────────────────────────────────────

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace('#', ''), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

interface JsPDFLike {
  setFont(name: string, style: string): void;
  setFontSize(size: number): void;
  setTextColor(r: number, g: number, b: number): void;
  setDrawColor(r: number, g: number, b: number): void;
  setLineWidth(w: number): void;
  setFillColor(r: number, g: number, b: number): void;
  text(text: string, x: number, y: number, opts?: { align?: string }): void;
  line(x1: number, y1: number, x2: number, y2: number): void;
  circle(x: number, y: number, r: number, style: string): void;
  splitTextToSize(text: string, maxWidth: number): string[];
  getStringUnitWidth(text: string): number;
  addPage(): void;
  save(filename: string): void;
  internal: { pageSize: { getWidth(): number; getHeight(): number }; scaleFactor: number };
}

/**
 * Draw a line of mixed bold/normal text on a jsPDF canvas, with word-wrapping.
 * Returns the Y position AFTER the last rendered line.
 */
function drawMixedText(
  doc: JsPDFLike,
  segments: MdSegment[],
  x0: number,
  y0: number,
  maxW: number,
  lineH: number,
  font: string,
  size: number,
  normalColor: string,
  boldColor: string
): number {
  let x = x0;
  let y = y0;
  const scale = size / (doc.internal.scaleFactor * 2.8346); // pt → mm

  for (const seg of segments) {
    doc.setFont(font, seg.bold ? 'bold' : 'normal');
    doc.setFontSize(size);
    const [r, g, b] = hexToRgb(seg.bold ? boldColor : normalColor);
    doc.setTextColor(r, g, b);

    const words = seg.text.split(' ');
    for (let wi = 0; wi < words.length; wi++) {
      const word = words[wi] + (wi < words.length - 1 ? ' ' : '');
      const w = doc.getStringUnitWidth(word) * scale;
      if (x > x0 && x + w > x0 + maxW) {
        y += lineH;
        x = x0;
      }
      doc.text(word, x, y);
      x += w;
    }
  }
  return y + lineH;
}

// ─── PDF Resume builder ───────────────────────────────────────────────────────

export async function exportResumePDF(
  text: string,
  filename: string,
  meta?: GenerationMeta,
  templateId: TemplateId = 'modern'
): Promise<void> {
  const { jsPDF } = await import('jspdf');
  const doc = new jsPDF({ unit: 'mm', format: 'a4' }) as unknown as JsPDFLike & {
    save(f: string): void;
  };
  const tpl = TEMPLATES[templateId];

  const ML = tpl.marginIn * 25.4,
    MR = tpl.marginIn * 25.4,
    MT = 16,
    MB = 16;
  const PW = doc.internal.pageSize.getWidth();
  const PH = doc.internal.pageSize.getHeight();
  const UW = PW - ML - MR;
  let y = MT;
  const F = 'helvetica';
  const LH = tpl.bodyPt * 0.42; // line height in mm (approx)

  const newPage = () => {
    doc.addPage();
    y = MT;
  };
  const checkY = (n: number) => {
    if (y + n > PH - MB) newPage();
  };
  const setC = (hex: string) => {
    const [r, g, b] = hexToRgb(hex);
    doc.setTextColor(r, g, b);
  };
  const setDC = (hex: string) => {
    const [r, g, b] = hexToRgb(hex);
    doc.setDrawColor(r, g, b);
  };

  const parsed = parseDocument(text);

  for (const line of parsed) {
    switch (line.kind) {
      case 'blank':
        y += LH * 0.6;
        break;

      case 'name': {
        checkY(tpl.namePt * 0.5);
        doc.setFont(F, 'bold');
        doc.setFontSize(tpl.namePt);
        setC(tpl.nameColor);
        const nameText = meta?.candidateName || line.text;
        if (tpl.nameCentered) doc.text(nameText, PW / 2, y, { align: 'center' });
        else doc.text(nameText, ML, y);
        y += tpl.namePt * 0.42 + 1;
        break;
      }

      case 'contact': {
        checkY(8);
        doc.setFont(F, 'normal');
        doc.setFontSize(8.5);
        setC(tpl.dateColor);
        if (tpl.nameCentered) doc.text(line.text, PW / 2, y, { align: 'center' });
        else doc.text(line.text, ML, y);
        y += 3.5;
        setDC(tpl.ruleColor);
        doc.setLineWidth(0.25);
        doc.line(ML, y, PW - MR, y);
        y += 5;
        break;
      }

      case 'sectionHeader': {
        checkY(10);
        y += 4;
        const headerText = tpl.sectionAllCaps ? line.text.toUpperCase() : line.text;
        doc.setFont(F, 'bold');
        doc.setFontSize(tpl.sectionPt);
        setC(tpl.sectionColor);
        doc.text(headerText, ML, y);
        y += 2.5;
        if (tpl.sectionStyle !== 'bold-only') {
          setDC(tpl.accentColor);
          doc.setLineWidth(tpl.sectionStyle === 'ruled-bottom' ? 0.5 : 0.25);
          doc.line(ML, y, PW - MR, y);
        }
        y += 4;
        break;
      }

      case 'jobEntry': {
        checkY(7);
        // Company name (may contain bold keywords)
        const compSegs = line.segments;
        y = drawMixedText(
          doc,
          compSegs.map((s) => ({ ...s, bold: true })),
          ML,
          y,
          UW * 0.7,
          LH,
          F,
          tpl.bodyPt,
          tpl.bodyColor,
          tpl.emphasisColor
        );
        y -= LH; // same line — date on right
        if (line.rightText) {
          doc.setFont(F, 'normal');
          doc.setFontSize(9);
          setC(tpl.dateColor);
          doc.text(line.rightText, PW - MR, y - LH * 0.3, { align: 'right' });
        }
        y += LH * 0.5;
        break;
      }

      case 'jobTitle': {
        checkY(5);
        doc.setFont(F, 'italic');
        doc.setFontSize(tpl.bodyPt - 0.5);
        setC(tpl.dateColor);
        doc.text(line.text, ML, y);
        y += LH + 1;
        break;
      }

      case 'bullet': {
        checkY(5);
        // Bullet dot
        const [br, bg, bb] = hexToRgb(tpl.bodyColor);
        doc.setFillColor(br, bg, bb);
        doc.circle(ML + 1.3, y - 1.3, 0.65, 'F');
        const bulletX = ML + 4;
        const nextY = drawMixedText(
          doc,
          line.segments,
          bulletX,
          y,
          UW - 4,
          LH,
          F,
          tpl.bodyPt,
          tpl.bodyColor,
          tpl.emphasisColor
        );
        y = nextY;
        break;
      }

      default: {
        checkY(5);
        const nextY = drawMixedText(
          doc,
          line.segments,
          ML,
          y,
          UW,
          LH + 0.5,
          F,
          tpl.bodyPt,
          tpl.bodyColor,
          tpl.emphasisColor
        );
        y = nextY;
        break;
      }
    }
  }

  doc.save(filename);
}

// ─── PDF Cover Letter builder ─────────────────────────────────────────────────

export async function exportCoverLetterPDF(
  text: string,
  filename: string,
  meta?: GenerationMeta,
  templateId: TemplateId = 'modern'
): Promise<void> {
  const { jsPDF } = await import('jspdf');
  const doc = new jsPDF({ unit: 'mm', format: 'a4' }) as unknown as JsPDFLike & {
    save(f: string): void;
  };
  const tpl = TEMPLATES[templateId];

  const ML = (tpl.marginIn + 0.15) * 25.4,
    MR = ML;
  const PW = doc.internal.pageSize.getWidth();
  const PH = doc.internal.pageSize.getHeight();
  const UW = PW - ML - MR;
  let y = 20;
  const F = 'helvetica';
  const LH = tpl.bodyPt * 0.45;

  const newPage = () => {
    doc.addPage();
    y = 20;
  };
  const checkY = (n: number) => {
    if (y + n > PH - 20) newPage();
  };
  const setC = (hex: string) => {
    const [r, g, b] = hexToRgb(hex);
    doc.setTextColor(r, g, b);
  };
  const setDC = (hex: string) => {
    const [r, g, b] = hexToRgb(hex);
    doc.setDrawColor(r, g, b);
  };

  const lines = text.split('\n').map((l) => l.trim());
  let headerDone = false;
  let inBody = false;

  for (const rawLine of lines) {
    const clean = stripMd(rawLine);
    if (!clean) {
      y += LH * 0.7;
      continue;
    }
    const segs = parseInlineMd(rawLine);

    const isSalutation = /^(?:Dear|Sehr geehrte|À l'attention|Estimado|Geachte)/i.test(clean);
    const isSignoff =
      /^(?:Kind regards|Sincerely|Best regards|Yours|Mit freundlichen|Cordialement)/i.test(clean);

    if (!headerDone && y === 20) {
      // Name
      doc.setFont(F, 'bold');
      doc.setFontSize(tpl.namePt - 2);
      setC(tpl.nameColor);
      doc.text(meta?.candidateName || clean, ML, y);
      y += (tpl.namePt - 2) * 0.42 + 1.5;
      continue;
    }

    if (
      !headerDone &&
      (clean.includes('@') || /\|/.test(clean) || /\+?\d[\d\s\-()]{6,}/.test(clean))
    ) {
      doc.setFont(F, 'normal');
      doc.setFontSize(8.5);
      setC(tpl.dateColor);
      doc.text(clean, ML, y);
      y += 3.5;
      setDC(tpl.ruleColor);
      doc.setLineWidth(0.25);
      doc.line(ML, y, PW - MR, y);
      y += 7;
      headerDone = true;
      continue;
    }

    if (
      !inBody &&
      /^(?:\d{1,2}\s+\w+|\w+\s+\d{4}|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec))/i.test(
        clean
      )
    ) {
      doc.setFont(F, 'normal');
      doc.setFontSize(9.5);
      setC(tpl.dateColor);
      doc.text(clean, ML, y);
      y += LH + 1;
      continue;
    }

    if (isSalutation) {
      inBody = true;
      y += 3;
      checkY(8);
      doc.setFont(F, 'bold');
      doc.setFontSize(tpl.bodyPt + 0.5);
      setC(tpl.bodyColor);
      doc.text(clean, ML, y);
      y += LH + 3;
      continue;
    }

    if (isSignoff) {
      y += 5;
      checkY(8);
      doc.setFont(F, 'normal');
      doc.setFontSize(tpl.bodyPt + 0.5);
      setC(tpl.bodyColor);
      doc.text(clean, ML, y);
      y += LH + 12;
      continue;
    }

    if (!inBody) {
      doc.setFont(F, 'normal');
      doc.setFontSize(tpl.bodyPt);
      setC(tpl.dateColor);
      doc.text(clean, ML, y);
      y += LH + 1;
      continue;
    }

    // Body with mixed bold
    checkY(6);
    const nextY = drawMixedText(
      doc,
      segs,
      ML,
      y,
      UW,
      LH + 1,
      F,
      tpl.bodyPt + 0.5,
      tpl.bodyColor,
      tpl.emphasisColor
    );
    y = nextY + 2;
  }

  doc.save(filename);
}

// ─── Cover letter text extraction ────────────────────────────────────────────

/**
 * Strips prompt scaffolding from AI cover letter output.
 * If the AI echoed the resume/job-ad context, extract only the letter section.
 */
function extractCoverLetterText(raw: string): string {
  const marker = '### COMPLETE COVER LETTER ###';
  const idx = raw.indexOf(marker);
  if (idx !== -1) {
    return raw.slice(idx + marker.length).trim();
  }
  // Fallback: if the AI output contains the resume section marker, strip everything before the letter.
  // Heuristic: find first "Dear " or "Sehr geehrte" that comes after any ### markers.
  const lastHash = raw.lastIndexOf('###');
  if (lastHash !== -1) {
    const afterHash = raw.slice(lastHash);
    const salutationMatch = afterHash.search(/\n(Dear |Sehr geehrte)/);
    if (salutationMatch !== -1) {
      return afterHash.slice(salutationMatch).trim();
    }
    // If no salutation found after last ###, just return everything after it.
    return afterHash.replace(/^###[^\n]*\n/, '').trim();
  }
  return raw.trim();
}

// ─── Public export API ────────────────────────────────────────────────────────

export async function exportDOCX(
  text: string,
  filename: string,
  type: 'resume' | 'cover-letter' = 'resume',
  meta?: GenerationMeta,
  templateId: TemplateId = 'modern',
  atsMode = false
): Promise<void> {
  try {
    if (!text || text.trim().length === 0) {
      throw new Error('Cannot export empty document. Please generate content first.');
    }
    if (!filename || filename.trim().length === 0) {
      throw new Error('Invalid filename provided.');
    }
    if (!TEMPLATES[templateId]) {
      console.warn(`Template "${templateId}" not found, using "modern" instead.`);
      templateId = 'modern';
    }

    const { getClient } = await import('@/lib/app-client');
    const api = getClient();
    const exportText = type === 'cover-letter' ? extractCoverLetterText(text) : text;
    const _filePath = await api.documents.exportAndSave({
      text: exportText,
      format: 'docx',
      documentType: type,
      templateId,
      atsMode,
      meta: meta
        ? {
            candidateName: meta.candidateName,
            jobTitle: meta.jobTitle,
            companyName: meta.companyName,
            targetLanguage: meta.targetLanguage,
          }
        : undefined,
    });
  } catch (error) {
    console.error('DOCX export failed:', error);
    throw new Error(
      `Failed to export DOCX: ${error instanceof Error ? error.message : 'Unknown error'}`,
      { cause: error }
    );
  }
}

export async function exportPDF(
  text: string,
  filename: string,
  type: 'resume' | 'cover-letter' = 'resume',
  meta?: GenerationMeta,
  templateId: TemplateId = 'modern',
  atsMode = false
): Promise<void> {
  try {
    if (!text || text.trim().length === 0) {
      throw new Error('Cannot export empty document. Please generate content first.');
    }
    if (!filename || filename.trim().length === 0) {
      throw new Error('Invalid filename provided.');
    }
    if (!TEMPLATES[templateId]) {
      console.warn(`Template "${templateId}" not found, using "modern" instead.`);
      templateId = 'modern';
    }

    const { getClient } = await import('@/lib/app-client');
    const api = getClient();
    const exportText = type === 'cover-letter' ? extractCoverLetterText(text) : text;
    const _filePath = await api.documents.exportAndSave({
      text: exportText,
      format: 'pdf',
      documentType: type,
      templateId,
      atsMode,
      meta: meta
        ? {
            candidateName: meta.candidateName,
            jobTitle: meta.jobTitle,
            companyName: meta.companyName,
            targetLanguage: meta.targetLanguage,
          }
        : undefined,
    });
  } catch (error) {
    console.error('PDF export failed:', error);
    throw new Error(
      `Failed to export PDF: ${error instanceof Error ? error.message : 'Unknown error'}`,
      { cause: error }
    );
  }
}

export function exportTXT(text: string, filename: string): void {
  try {
    // Validation
    if (!text || text.trim().length === 0) {
      throw new Error('Cannot export empty document. Please generate content first.');
    }
    if (!filename || filename.trim().length === 0) {
      throw new Error('Invalid filename provided.');
    }

    const clean = stripMd(text); // no **asterisks** in plain text
    const blob = new Blob([clean], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = Object.assign(document.createElement('a'), { href: url, download: filename });
    a.click();
    URL.revokeObjectURL(url);
  } catch (error) {
    console.error('TXT export failed:', error);
    throw new Error(
      `Failed to export TXT: ${error instanceof Error ? error.message : 'Unknown error'}`,
      { cause: error }
    );
  }
}
