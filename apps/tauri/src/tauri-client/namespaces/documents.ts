import { invoke } from '@tauri-apps/api/core';

export const documents = {
  list: () => invoke('documents_list'),
  import: (req: unknown) => invoke('documents_import', { req }),
  remove: (id: string) => invoke('documents_remove', { id }),
  setDefault: (id: string) => invoke('documents_set_default', { id }),
  exportDocument: (request: unknown) => invoke('documents_export_document', { request }),
  exportAndSave: (request: unknown) => invoke('documents_export_and_save', { request }),
};
