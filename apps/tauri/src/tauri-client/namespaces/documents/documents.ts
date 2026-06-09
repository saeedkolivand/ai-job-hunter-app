import { invoke } from '@tauri-apps/api/core';

import type { DocumentImportRequest } from '@ajh/shared/schemas';

export const documents = {
  list: () => invoke('documents_list'),
  import: (req: DocumentImportRequest) => invoke('documents_import', { req }),
  recommendTemplate: (req: unknown) => invoke('documents_recommend_template', { req }),
  remove: (id: string) => invoke('documents_remove', { id }),
  setDefault: (id: string) => invoke('documents_set_default', { id }),
  exportDocument: (request: unknown) => invoke('documents_export_document', { request }),
  exportAndSave: (request: unknown) => invoke('documents_export_and_save', { request }),
  renderPreviewImages: (request: unknown) => invoke('documents_render_preview_images', { request }),
};
