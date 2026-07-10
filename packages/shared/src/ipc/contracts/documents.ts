import type { DocumentImportRequest } from '../../schemas/index.js';
import type { DocumentRecord } from '../../types/index.js';
import type { ContactProfile } from './contactProfile.js';

export type TemplateId =
  | 'classic'
  | 'swiss-minimal'
  | 'academic'
  | 'atelier'
  | 'meridian'
  | 'throughline'
  | 'portrait'
  | 'lebenslauf';

interface ExportMeta {
  candidateName?: string;
  jobTitle?: string;
  companyName?: string;
  targetLanguage?: string;
}

export interface BaseExportRequest {
  text: string;
  format: 'docx' | 'pdf' | 'txt';
  documentType: 'resume' | 'cover-letter';
  templateId: TemplateId;
  meta?: ExportMeta;
  /** Linearize two-column layouts for ATS parsers. Only affects two-column template. */
  atsMode?: boolean;
  /** Target market id (`us`, `de`, …); drives the page size (US → Letter, else A4). */
  locale?: string;
  /**
   * Header contact source of truth — named fields rendered as clickable links,
   * localized per language. When present it overrides whatever links the
   * generated text carried, so a company link can't displace a personal profile.
   */
  contact?: ContactProfile;
  /**
   * Per-export **document accent** (ADR 0004): an optional 6-digit hex
   * (`#RRGGBB` or bare `RRGGBB`) recoloring the chosen template's accent.
   * Distinct from the app-UI accent — the backend never reads theme prefs.
   * Omitted (the default) leaves the template palette untouched; a malformed
   * value is ignored by the backend.
   */
  accent?: string;
}

export type ExportIssueSeverity = 'critical' | 'warning';

/** A single problem found while re-reading an exported document. */
export interface ExportIssue {
  severity: ExportIssueSeverity;
  /** Stable machine code (e.g. `section_order`, `missing_section`). */
  code: string;
  /** Plain-language explanation for the user. */
  message: string;
}

/**
 * Pre-export validation report. Present for PDF/DOCX, absent for TXT. The
 * backend auto-fixes a two-column layout that doesn't survive extraction and
 * blocks the export only when a critical issue survives, so `ok` is `false`
 * only on a hard failure the user must address.
 */
export interface ExportReport {
  ok: boolean;
  /** Whether the returned bytes were rendered in ATS (single-column) mode. */
  atsMode: boolean;
  issues: ExportIssue[];
  /** Human-readable description of each auto-fix that was applied. */
  fixed: string[];
}

export interface CoverLetterExportRequest {
  templateId: TemplateId;
  /** Recipient first/last name — used for salutation. */
  recipientName?: string;
  /** Honorific: "Dr.", "Prof.", "Ms.", etc. — prepended to salutation. */
  recipientTitle?: string;
  recipientCompany?: string;
  /** Multi-line OK — rendered as recipient block. */
  recipientAddress?: string;
  /** Overrides the template's default closing phrase. */
  closingPhrase?: string;
  /** User's professional title — shown in NameAndTitle / ScriptStyle signatures. */
  signatureTitle?: string;
  /** Overrides the app locale for salutation and closing phrase resolution. */
  locale?: 'en' | 'de';
}

export type ConfidenceLevel = 'high' | 'medium' | 'low';

/** Byte span `[start, end)` into the extracted source text. */
export interface SourceSpan {
  start: number;
  end: number;
}

/** One structured-extraction field: value, confidence, and where it was found. */
export interface ResumeField<T> {
  value: T;
  confidence: ConfidenceLevel;
  sourceSpan?: SourceSpan;
}

/** A detected section in the review inventory. */
export interface SectionSummary {
  heading: string;
  /** Canonical kind: `experience`, `skills`, `custom`, … */
  kind: string;
  confidence: ConfidenceLevel;
}

/**
 * Typed view of an imported resume with per-field confidence. Returned by
 * `import` so the renderer can surface low-confidence / missing fields for
 * review before generation. `reviewRequired` flags (never blocks).
 */
export interface StructuredResume {
  name: ResumeField<string>;
  email?: ResumeField<string>;
  phone?: ResumeField<string>;
  location?: ResumeField<string>;
  links: ResumeField<string>[];
  sections: SectionSummary[];
  /** Whole-document confidence (the fast gate). */
  overall: ConfidenceLevel;
  reviewRequired: boolean;
  warnings: string[];
}

/**
 * A single identity field where the imported résumé's contact value conflicts
 * with the saved contact profile (both non-empty, normalized values differ).
 * The import never blocks on these — it still silently fills empty fields — but
 * they are returned so the renderer can let the user resolve each one per-field.
 * `field` is a stable key: `email`, `phone`, `fullName`, `linkedin`, `github`,
 * `website`, or `location`. `current`/`suggested` are the original (un-normalized)
 * values for faithful display.
 */
export interface ContactFieldConflict {
  field: string;
  current: string;
  suggested: string;
}

/** Signals the recommender reads — a subset of the generation metadata. */
export interface TemplateRecommendSignals {
  jobTitle?: string;
  /** `junior | mid | senior | lead | executive` */
  candidateSeniority?: string;
  topRequirements?: string[];
  resumeLanguage?: string;
  jobAdLanguage?: string;
  /** Job ad's target country/market (`us`, `de`, `gb`, …); wins over language. */
  targetCountry?: string;
}

/** A template + locale suggestion with a printed reason. Always overridable. */
export interface TemplateRecommendation {
  templateId: TemplateId;
  /** Market id (`us`, `dach`, `en`, …). */
  locale: string;
  atsSuggested: boolean;
  rationale: string;
}

export interface DocumentsContract {
  list(): Promise<DocumentRecord[]>;

  /**
   * Fetch the stored extracted text for one document by id. Returns the empty
   * string when the document is missing or has no text (never rejects), so a
   * caller can safely seed a generator without a missing-doc guard.
   */
  getText(id: string): Promise<string>;

  import(req: DocumentImportRequest): Promise<{
    id: string;
    success: boolean;
    review?: StructuredResume;
    contactConflicts?: ContactFieldConflict[];
    suggestedContact?: ContactProfile;
  }>;

  /** Suggest a template + locale from the generation metadata signals. */
  recommendTemplate(req: TemplateRecommendSignals): Promise<TemplateRecommendation>;

  remove(id: string): Promise<void>;

  setDefault(id: string): Promise<void>;

  exportDocument(
    req: BaseExportRequest
  ): Promise<{ data: number[]; mimeType: string; filename: string; report?: ExportReport }>;

  exportAndSave(req: BaseExportRequest): Promise<string>;

  /**
   * Render the same document to per-page images for the live preview, shown via
   * `<img>` (CSP `img-src 'self' data: blob:`) instead of the PDF→iframe path.
   * Takes the same request fields as {@link exportDocument} (`format` is ignored
   * — the preview always emits SVG) and renders the identical model + Typst
   * world, so preview fidelity matches export. `pages` is one SVG document string
   * per page; `mimeType` is always `image/svg+xml`. Called imperatively (no React
   * Query key) — the preview is requested on demand, like an export.
   */
  renderPreviewImages(req: BaseExportRequest): Promise<{ pages: string[]; mimeType: string }>;
}

export const DOCUMENTS_CHANNELS = {
  list: 'documents:list',
  getText: 'documents:get_text',
  import: 'documents:import',
  recommendTemplate: 'documents:recommend_template',
  remove: 'documents:remove',
  exportDocument: 'documents:export_document',
  exportAndSave: 'documents:export_and_save',
  renderPreviewImages: 'documents:render_preview_images',
} as const;
