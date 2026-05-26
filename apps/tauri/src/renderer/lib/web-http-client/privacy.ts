import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function privacy(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    signOutAll: () => cmd('privacy', 'signOutAll'),
    clearInteractions: () => cmd('privacy', 'clearInteractions'),
    resetApp: () => cmd('privacy', 'resetApp'),
  };
}
