// Document export. DOCX/PDF render in the Rust backend via `documents.exportAndSave`;
// TXT is produced client-side. Filename + light text cleanup helpers live here too.

import type { GenerationMeta } from '@ajh/prompts/generate';

import { getClient } from '../app-client';
import { type TemplateId, TEMPLATES } from './templates';

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
