// Document export. DOCX/PDF render in the Rust backend via `documents.exportAndSave`;
// TXT is produced client-side. Filename + light text cleanup helpers live here too.

import type { GenerationMeta } from '@ajh/prompts/generate';

import { getClient } from '../../app-client';
import { type LetterLayoutId, type TemplateId, TEMPLATES } from '../templates';

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

// ─── Text helpers ─────────────────────────────────────────────────────────────

/** Strip **markers** from text for contexts that don't support bold (TXT). */
function stripMd(text: string): string {
  return text.replace(/\*\*([^*]+)\*\*/g, '$1');
}

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

// ─── SVG sanitisation ─────────────────────────────────────────────────────────

/** Typst SVG export leaves raw `&` in link hrefs (e.g. URL query separators),
 *  which is invalid XML and breaks SVG rendered via <img>. Escape stray `&`
 *  (those not already a valid XML entity) to `&amp;` so the SVG parses. */
function escapeSvgAmpersands(svg: string): string {
  return svg.replace(/&(?!(?:amp|lt|gt|quot|apos|#\d+|#x[0-9a-fA-F]+);)/g, '&amp;');
}

// ─── Public export API ────────────────────────────────────────────────────────

export async function exportDOCX(
  text: string,
  filename: string,
  type: 'resume' | 'cover-letter' = 'resume',
  meta?: GenerationMeta,
  templateId: TemplateId = 'classic',
  atsMode = false,
  locale?: string,
  accent?: string,
  letterLayoutId?: LetterLayoutId
): Promise<void> {
  try {
    if (!text || text.trim().length === 0) {
      throw new Error('Cannot export empty document. Please generate content first.');
    }
    if (!filename || filename.trim().length === 0) {
      throw new Error('Invalid filename provided.');
    }
    if (!TEMPLATES[templateId]) {
      // Surface the failure instead of silently swapping in a default template — a
      // wrong-template export is indistinguishable from a correct one and hides
      // the real bug. The Rust deserializer keeps a graceful, logged Classic
      // fallback as the proper degradation layer if an unknown id ever arrives.
      throw new Error(`Unknown export template: "${templateId}".`);
    }

    const api = getClient();
    const exportText = type === 'cover-letter' ? extractCoverLetterText(text) : text;
    // The stored contact profile is the source of truth for the header contact
    // line — passing it lets the backend build the header from named fields
    // (never the résumé's company-link pool). Missing profile → undefined (the
    // backend falls back to the text-derived header).
    const contact = await api.contactProfile.get().catch(() => undefined);
    const _filePath = await api.documents.exportAndSave({
      text: exportText,
      format: 'docx',
      documentType: type,
      templateId,
      atsMode,
      locale,
      // Per-export document accent (6-hex) — undefined leaves the template palette
      // untouched; the backend re-validates and ignores a malformed value.
      accent,
      // Cover-letter layout — omitted (undefined) → the backend renders classic;
      // ignored for résumé exports.
      letterLayoutId,
      contact,
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
  templateId: TemplateId = 'classic',
  atsMode = false,
  locale?: string,
  accent?: string,
  letterLayoutId?: LetterLayoutId
): Promise<void> {
  try {
    if (!text || text.trim().length === 0) {
      throw new Error('Cannot export empty document. Please generate content first.');
    }
    if (!filename || filename.trim().length === 0) {
      throw new Error('Invalid filename provided.');
    }
    if (!TEMPLATES[templateId]) {
      // Surface the failure instead of silently swapping in a default template — a
      // wrong-template export is indistinguishable from a correct one and hides
      // the real bug. The Rust deserializer keeps a graceful, logged Classic
      // fallback as the proper degradation layer if an unknown id ever arrives.
      throw new Error(`Unknown export template: "${templateId}".`);
    }

    const api = getClient();
    const exportText = type === 'cover-letter' ? extractCoverLetterText(text) : text;
    // See exportDOCX: the contact profile is the header source of truth.
    const contact = await api.contactProfile.get().catch(() => undefined);
    const _filePath = await api.documents.exportAndSave({
      text: exportText,
      format: 'pdf',
      documentType: type,
      templateId,
      atsMode,
      locale,
      // Per-export document accent (6-hex) — see exportDOCX.
      accent,
      // Cover-letter layout — see exportDOCX.
      letterLayoutId,
      contact,
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

/**
 * Render the document to SVG page images WITHOUT saving — the same Rust Typst
 * renderer the export uses (`documents.renderPreviewImages`), so the in-app
 * preview is the authoritative output, not an approximation (see ADR-012).
 * Returns one SVG string per page for the caller to display via `<img>` data
 * URLs. Throws on empty text, unknown template, or a backend validation block,
 * exactly like {@link exportPDF}.
 */
export async function renderDocumentPreview(
  text: string,
  type: 'resume' | 'cover-letter' = 'resume',
  meta?: GenerationMeta,
  templateId: TemplateId = 'classic',
  atsMode = false,
  locale?: string,
  accent?: string,
  letterLayoutId?: LetterLayoutId
): Promise<string[]> {
  if (!text || text.trim().length === 0) {
    throw new Error('Cannot preview an empty document.');
  }
  if (!TEMPLATES[templateId]) {
    throw new Error(`Unknown export template: "${templateId}".`);
  }

  const api = getClient();
  const exportText = type === 'cover-letter' ? extractCoverLetterText(text) : text;
  // Same header source of truth as the real export (see exportPDF/exportDOCX).
  const contact = await api.contactProfile.get().catch(() => undefined);
  const result = await api.documents.renderPreviewImages({
    text: exportText,
    // `format` is required by BaseExportRequest but ignored by the backend for
    // SVG preview output — the preview always emits SVG regardless of format.
    format: 'pdf',
    documentType: type,
    templateId,
    atsMode,
    locale,
    // Per-export document accent (6-hex) — must match the export so the preview
    // is faithful (undefined leaves the template palette untouched).
    accent,
    // Cover-letter layout — must match the export so the preview is faithful
    // (undefined → the backend renders classic).
    letterLayoutId,
    contact,
    meta: meta
      ? {
          candidateName: meta.candidateName,
          jobTitle: meta.jobTitle,
          companyName: meta.companyName,
          targetLanguage: meta.targetLanguage,
        }
      : undefined,
  });

  return result.pages.map(escapeSvgAmpersands);
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
