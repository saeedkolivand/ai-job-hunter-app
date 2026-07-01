import { invoke } from '@tauri-apps/api/core';

import type {
  BaseExportRequest,
  ContactFieldConflict,
  ContactProfile,
  ExportReport,
  StructuredResume,
  TemplateRecommendation,
  TemplateRecommendSignals,
} from '@ajh/shared/ipc';
import type { DocumentImportRequest } from '@ajh/shared/schemas';

import type { RawDoc } from '@/lib/doc-record';

export const documents = {
  list: () => invoke<RawDoc[]>('documents_list'),
  getText: (id: string) => invoke<string>('documents_get_text', { id }),
  import: (req: DocumentImportRequest) =>
    invoke<{
      id: string;
      success: boolean;
      review?: StructuredResume;
      contactConflicts?: ContactFieldConflict[];
      suggestedContact?: ContactProfile;
    }>('documents_import', { req }),
  recommendTemplate: (req: TemplateRecommendSignals) =>
    invoke<TemplateRecommendation>('documents_recommend_template', { req }),
  remove: (id: string) => invoke<void>('documents_remove', { id }),
  setDefault: (id: string) => invoke<void>('documents_set_default', { id }),
  exportDocument: (request: BaseExportRequest) =>
    invoke<{
      data: number[];
      mimeType: string;
      filename: string;
      report?: ExportReport;
    }>('documents_export_document', { request }),
  exportAndSave: (request: BaseExportRequest) =>
    invoke<string>('documents_export_and_save', { request }),
  renderPreviewImages: (request: BaseExportRequest) =>
    invoke<{ pages: string[]; mimeType: string }>('documents_render_preview_images', { request }),
};
