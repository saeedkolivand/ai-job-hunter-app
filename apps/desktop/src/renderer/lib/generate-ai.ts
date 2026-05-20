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
  buildMetadataPrompt,
  buildResumeSystemPrompt,
  buildResumePrompt,
  buildCoverLetterSystemPrompt,
  buildCoverLetterPrompt,
  validateMetadata,
  extractPlainText,
  type GenerationMode,
  type GenerationMeta,
} from '@ajh/prompts/generate';

export type { GenerationMode, GenerationMeta };
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
  locale = 'en'
): Promise<string> {
  const res = (await window.api.ai.generate({
    model,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    locale: safeLocale(locale),
    temperature,
    maxTokens: 3000,
  })) as { jobId: string };

  const jobId = res.jobId;
  let buffer = '';

  return new Promise((resolve, reject) => {
    const off = window.api.ai.onStream((chunk: unknown) => {
      const c = chunk as { jobId: string; delta: string; done: boolean };
      if (c.jobId !== jobId) return;
      if (c.delta) {
        buffer += c.delta;
        onToken(c.delta);
      }
      if (c.done) {
        off();
        resolve(buffer);
      }
    });

    setTimeout(
      () => {
        off();
        resolve(buffer);
      },
      5 * 60 * 1000
    );

    const poll = setInterval(async () => {
      const job = (await window.api.jobs.get(jobId).catch(() => null)) as { status: string } | null;
      if (job?.status === 'failed' || job?.status === 'cancelled') {
        clearInterval(poll);
        off();
        reject(new Error(`Generation ${job.status}. Please try again.`));
      }
      if (job?.status === 'completed') clearInterval(poll);
    }, 3_000);
  });
}

// ─── Generation steps ─────────────────────────────────────────────────────────

export async function extractMetadata(
  resume: string,
  jobAd: string,
  model: string,
  locale = 'en'
): Promise<GenerationMeta> {
  const { system, user } = buildMetadataPrompt(resume, jobAd);
  try {
    const raw = await streamGenerate(model, system, user, () => {}, 0.1, locale);
    const meta = validateMetadata(raw);
    if (meta) return meta;
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
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
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
  locale = 'en'
): Promise<string> {
  const system = buildResumeSystemPrompt(mode);
  const user = buildResumePrompt(resume, jobAd, meta, mode);
  const raw = await streamGenerate(model, system, user, onToken, 0.25, locale);
  return extractPlainText(raw);
}

export async function generateCoverLetter(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en'
): Promise<string> {
  const system = buildCoverLetterSystemPrompt(mode);
  const user = buildCoverLetterPrompt(resume, jobAd, meta, mode);
  const raw = await streamGenerate(model, system, user, onToken, 0.4, locale);
  return extractPlainText(raw);
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
  const parts = line.split(/\*\*([^*]+)\*\*/g);
  const segments: MdSegment[] = [];
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i];
    if (!part) continue;
    segments.push({ text: part, bold: i % 2 === 1 });
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
  /\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d\d|20\d\d)[\s\S]{0,30}?(?:Present|Current|Now|Heute|20\d\d|19\d\d)\b/i;

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
  if (idx === 0) return make('name');

  // Bullet
  if (/^[•\-–*·▪▸►]\s/.test(clean)) {
    const bulletText = trimmed.replace(/^[•\-–*·▪▸►]\s*/, '');
    return {
      kind: 'bullet',
      raw: bulletText,
      text: stripMd(bulletText),
      segments: parseInlineMd(bulletText),
    };
  }

  // Section header: known name OR all-caps short line
  const lower = clean.toLowerCase();
  if (
    SECTION_NAMES.has(lower) ||
    (clean === clean.toUpperCase() &&
      clean.length <= 60 &&
      /[A-ZÄÖÜ]{2,}/.test(clean) &&
      !/\d{4}/.test(clean))
  ) {
    return make('sectionHeader');
  }

  // Job entry: 3+ spaces gap before a date range
  const gapMatch = clean.match(/^(.+?)\s{3,}(.+)$/);
  if (gapMatch && gapMatch[1] && gapMatch[2] && DATE_RE.test(gapMatch[2])) {
    return {
      kind: 'jobEntry',
      raw: trimmed,
      text: gapMatch[1].trim(),
      segments: parseInlineMd(trimmed.split(/\s{3,}/)[0] ?? trimmed),
      rightText: gapMatch[2].trim(),
    };
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

  // Contact: has @ or phone or pipe separators
  if (
    clean.includes('@') ||
    /\+?\d[\d\s\-().]{7,}/.test(clean) ||
    clean.split(/[|·•]/).length >= 2 ||
    /linkedin\.com/i.test(clean)
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

export type TemplateId = 'classic' | 'modern' | 'executive';

interface DocTemplate {
  id: TemplateId;
  name: string;
  // Colors (hex, no #)
  nameColor: string;
  sectionColor: string; // section header text
  accentColor: string; // section header rule/border
  bodyColor: string;
  dateColor: string;
  emphasisColor: string; // bolded keyword color
  ruleColor: string; // divider line
  // Sizes (pt — converted to half-pt for docx: pt * 2)
  namePt: number;
  sectionPt: number;
  bodyPt: number;
  // DOCX layout
  marginIn: number;
  lineSpacingDocx: number; // 240=1.0, 276=1.15, 288=1.2
  sectionSpacingBefore: number; // twips before section header
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
};

// ─── DOCX helpers ─────────────────────────────────────────────────────────────

/** Convert **bold** markdown segments into TextRun[] for docx. */
function mdRunsDocx(
  text: string,
  base: {
    size: number;
    color: string;
    font: string;
    italics?: boolean;
    bold?: boolean;
  },
  emphasisColor?: string
) {
  // Dynamic import wrapper — called inside async functions only
  type TR = any;
  const segs = parseInlineMd(text);
  // We return a factory function so TextRun can be imported once at call site
  return (TextRun: new (opts: object) => TR) =>
    segs.map(
      (seg) =>
        new TextRun({
          text: seg.text,
          font: base.font,
          size: base.size,
          color: seg.bold ? (emphasisColor ?? base.color) : base.color,
          bold: seg.bold || !!base.bold,
          italics: base.italics,
        })
    );
}

// ─── DOCX Resume builder ──────────────────────────────────────────────────────

async function buildResumeDocx(text: string, meta: GenerationMeta | undefined, tpl: DocTemplate) {
  const { Document, Paragraph, TextRun, BorderStyle, TabStopType, convertInchesToTwip } =
    await import('docx');

  const F = 'Calibri';
  const PT = (pt: number) => Math.round(pt * 2); // pt → half-pt
  const PAGE_W = convertInchesToTwip(6.27);

  const parsed = parseDocument(text);
  const children: any[] = [];

  // Section header border config
  const sectionBorder =
    tpl.sectionStyle === 'ruled-bottom'
      ? { bottom: { color: tpl.accentColor, space: 3, style: BorderStyle.SINGLE, size: 8 } }
      : tpl.sectionStyle === 'underline'
        ? { bottom: { color: tpl.accentColor, space: 2, style: BorderStyle.SINGLE, size: 4 } }
        : undefined;

  let nameWritten = false;

  for (let i = 0; i < parsed.length; i++) {
    const line = parsed[i];
    if (!line) continue;

    switch (line.kind) {
      case 'blank':
        if (i > 0 && parsed[i - 1]?.kind !== 'sectionHeader') {
          children.push(new Paragraph({ children: [], spacing: { after: 60 } }));
        }
        break;

      case 'name': {
        nameWritten = true;
        const nameText = meta?.candidateName || line.text;
        children.push(
          new Paragraph({
            children: [
              new TextRun({
                text: nameText,
                bold: true,
                size: PT(tpl.namePt),
                color: tpl.nameColor,
                font: F,
              }),
            ],
            alignment: tpl.nameCentered ? 'center' : 'left',
            spacing: { after: 40 },
          })
        );
        break;
      }

      case 'contact':
        children.push(
          new Paragraph({
            children: [
              new TextRun({ text: line.text, size: PT(9), color: tpl.dateColor, font: F }),
            ],
            alignment: tpl.nameCentered ? 'center' : 'left',
            spacing: { after: 0 },
            border: {
              bottom: { color: tpl.ruleColor, space: 6, style: BorderStyle.SINGLE, size: 3 },
            },
          })
        );
        children.push(new Paragraph({ children: [], spacing: { after: 80 } }));
        break;

      case 'sectionHeader': {
        const headerText = tpl.sectionAllCaps ? line.text.toUpperCase() : line.text;
        children.push(
          new Paragraph({
            children: [
              new TextRun({
                text: headerText,
                bold: true,
                size: PT(tpl.sectionPt),
                color: tpl.sectionColor,
                font: F,
                characterSpacing: tpl.sectionAllCaps ? 30 : 0,
              }),
            ],
            spacing: { before: tpl.sectionSpacingBefore, after: 60 },
            ...(sectionBorder ? { border: sectionBorder } : {}),
          })
        );
        break;
      }

      case 'jobEntry': {
        const runsFactory = mdRunsDocx(
          line.raw.split(/\s{3,}/)[0] ?? line.raw,
          {
            size: PT(tpl.bodyPt),
            color: tpl.bodyColor,
            font: F,
            bold: true,
          },
          tpl.emphasisColor
        );
        children.push(
          new Paragraph({
            tabStops: [{ type: TabStopType.RIGHT, position: PAGE_W }],
            children: [
              ...runsFactory(TextRun),
              new TextRun({ text: '\t', font: F }),
              new TextRun({
                text: line.rightText ?? '',
                size: PT(9.5),
                color: tpl.dateColor,
                font: F,
              }),
            ],
            spacing: { before: 160, after: 20 },
          })
        );
        break;
      }

      case 'jobTitle': {
        const runsFactory = mdRunsDocx(
          line.raw,
          {
            size: PT(tpl.bodyPt - 0.5),
            color: tpl.dateColor,
            font: F,
            italics: true,
          },
          tpl.emphasisColor
        );
        children.push(
          new Paragraph({
            children: runsFactory(TextRun),
            spacing: { after: 60 },
          })
        );
        break;
      }

      case 'bullet': {
        const runsFactory = mdRunsDocx(
          line.raw,
          {
            size: PT(tpl.bodyPt),
            color: tpl.bodyColor,
            font: F,
          },
          tpl.emphasisColor
        );
        children.push(
          new Paragraph({
            children: runsFactory(TextRun),
            bullet: { level: 0 },
            spacing: { after: 40 },
            indent: { left: convertInchesToTwip(0.2) },
          })
        );
        break;
      }

      default: {
        const runsFactory = mdRunsDocx(
          line.raw,
          {
            size: PT(tpl.bodyPt),
            color: tpl.bodyColor,
            font: F,
          },
          tpl.emphasisColor
        );
        children.push(
          new Paragraph({
            children: runsFactory(TextRun),
            spacing: { after: 80 },
          })
        );
      }
    }
  }

  if (!nameWritten && meta?.candidateName) {
    children.unshift(
      new Paragraph({
        children: [
          new TextRun({
            text: meta.candidateName,
            bold: true,
            size: PT(tpl.namePt),
            color: tpl.nameColor,
            font: F,
          }),
        ],
        alignment: tpl.nameCentered ? 'center' : 'left',
        spacing: { after: 40 },
      })
    );
  }

  return new Document({
    styles: {
      default: {
        document: {
          run: { font: F, size: PT(tpl.bodyPt), color: tpl.bodyColor },
          paragraph: { spacing: { line: tpl.lineSpacingDocx } },
        },
      },
    },
    sections: [
      {
        properties: {
          page: {
            margin: {
              top: convertInchesToTwip(0.9),
              bottom: convertInchesToTwip(0.9),
              left: convertInchesToTwip(tpl.marginIn),
              right: convertInchesToTwip(tpl.marginIn),
            },
          },
        },
        children,
      },
    ],
  });
}

// ─── DOCX Cover Letter builder ────────────────────────────────────────────────

async function buildCoverLetterDocx(
  text: string,
  meta: GenerationMeta | undefined,
  tpl: DocTemplate
) {
  const { Document, Paragraph, TextRun, BorderStyle, convertInchesToTwip } = await import('docx');
  const F = 'Calibri';
  const PT = (pt: number) => Math.round(pt * 2);
  const children: any[] = [];

  const lines = text.split('\n').map((l) => l.trim());
  let headerDone = false;
  let inBody = false;

  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i];
    if (!raw) continue;
    const clean = stripMd(raw);
    if (!clean) {
      children.push(new Paragraph({ children: [], spacing: { after: 120 } }));
      continue;
    }

    const isSalutation = /^(?:Dear|Sehr geehrte|À l'attention|Estimado|Geachte)/i.test(clean);
    const isSignoff =
      /^(?:Kind regards|Sincerely|Best regards|Yours|Mit freundlichen|Cordialement|Atenciosamente)/i.test(
        clean
      );
    const isDate =
      i > 0 &&
      !headerDone &&
      /^(?:\d{1,2}\s+\w+|\w+\s+\d{4}|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec))/i.test(
        clean
      );

    // Name — first line
    if (i === 0) {
      children.push(
        new Paragraph({
          children: [
            new TextRun({
              text: meta?.candidateName || clean,
              bold: true,
              size: PT(tpl.namePt - 2),
              color: tpl.nameColor,
              font: F,
            }),
          ],
          spacing: { after: 40 },
        })
      );
      continue;
    }

    // Contact line
    if (
      !headerDone &&
      (clean.includes('@') || /\|/.test(clean) || /\+?\d[\d\s\-()]{6,}/.test(clean))
    ) {
      children.push(
        new Paragraph({
          children: [new TextRun({ text: clean, size: PT(9), color: tpl.dateColor, font: F })],
          spacing: { after: 0 },
          border: {
            bottom: { color: tpl.ruleColor, space: 5, style: BorderStyle.SINGLE, size: 3 },
          },
        })
      );
      children.push(new Paragraph({ children: [], spacing: { after: 180 } }));
      headerDone = true;
      continue;
    }

    if (isDate) {
      children.push(
        new Paragraph({
          children: [new TextRun({ text: clean, size: PT(9.5), color: tpl.dateColor, font: F })],
          spacing: { after: 140 },
        })
      );
      continue;
    }

    if (isSalutation) {
      inBody = true;
      children.push(
        new Paragraph({
          children: [
            new TextRun({
              text: clean,
              bold: true,
              size: PT(tpl.bodyPt),
              color: tpl.bodyColor,
              font: F,
            }),
          ],
          spacing: { before: 120, after: 180 },
        })
      );
      continue;
    }

    if (isSignoff) {
      children.push(
        new Paragraph({
          children: [
            new TextRun({ text: clean, size: PT(tpl.bodyPt), color: tpl.bodyColor, font: F }),
          ],
          spacing: { before: 180, after: 320 },
        })
      );
      continue;
    }

    if (!inBody) {
      // Addressee block
      children.push(
        new Paragraph({
          children: [
            new TextRun({ text: clean, size: PT(tpl.bodyPt - 1), color: tpl.dateColor, font: F }),
          ],
          spacing: { after: 60 },
        })
      );
      continue;
    }

    // Body paragraph with inline bold
    const runsFactory = mdRunsDocx(
      raw,
      { size: PT(tpl.bodyPt + 0.5), color: tpl.bodyColor, font: F },
      tpl.emphasisColor
    );
    children.push(new Paragraph({ children: runsFactory(TextRun), spacing: { after: 200 } }));
  }

  return new Document({
    styles: {
      default: {
        document: {
          run: { font: F, size: PT(tpl.bodyPt), color: tpl.bodyColor },
          paragraph: { spacing: { line: 300 } },
        },
      },
    },
    sections: [
      {
        properties: {
          page: {
            margin: {
              top: convertInchesToTwip(1.0),
              bottom: convertInchesToTwip(1.0),
              left: convertInchesToTwip(tpl.marginIn + 0.15),
              right: convertInchesToTwip(tpl.marginIn + 0.15),
            },
          },
        },
        children,
      },
    ],
  });
}

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
          doc as unknown as JsPDFLike,
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
          doc as unknown as JsPDFLike,
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
          doc as unknown as JsPDFLike,
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
      doc as unknown as JsPDFLike,
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

// ─── Public export API ────────────────────────────────────────────────────────

export async function exportDOCX(
  text: string,
  filename: string,
  type: 'resume' | 'cover-letter' = 'resume',
  meta?: GenerationMeta,
  templateId: TemplateId = 'modern'
): Promise<void> {
  const { Packer } = await import('docx');
  const tpl = TEMPLATES[templateId];
  const doc =
    type === 'resume'
      ? await buildResumeDocx(text, meta, tpl)
      : await buildCoverLetterDocx(text, meta, tpl);

  const blob = new Blob([new Uint8Array(await Packer.toBuffer(doc))], {
    type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  });
  const url = URL.createObjectURL(blob);
  const a = Object.assign(document.createElement('a'), { href: url, download: filename });
  a.click();
  URL.revokeObjectURL(url);
}

export async function exportPDF(
  text: string,
  filename: string,
  type: 'resume' | 'cover-letter' = 'resume',
  meta?: GenerationMeta,
  templateId: TemplateId = 'modern'
): Promise<void> {
  if (type === 'cover-letter') await exportCoverLetterPDF(text, filename, meta, templateId);
  else await exportResumePDF(text, filename, meta, templateId);
}

export function exportTXT(text: string, filename: string): void {
  const clean = stripMd(text); // no **asterisks** in plain text
  const blob = new Blob([clean], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = Object.assign(document.createElement('a'), { href: url, download: filename });
  a.click();
  URL.revokeObjectURL(url);
}
