import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function documents(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    list: () => cmd('documents', 'list'),
    import: (req: unknown) => cmd('documents', 'import', req),
    remove: (id: string) => cmd('documents', 'remove', { id }),
    setDefault: (id: string) => cmd('documents', 'setDefault', { id }),
    exportDocument: (req: unknown) => cmd('documents', 'exportDocument', req),
    exportAndSave: (req: unknown) => cmd('documents', 'exportAndSave', req),
  };
}
