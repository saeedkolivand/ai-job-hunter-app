import { useState } from 'react';

import { useContactPromptSeen, usePreferencesStore } from '@/store/preferences-store';

/**
 * First-run nudge: before the very first generation, surface the contact profile
 * so the document header is complete. Shown once (persisted flag), then never
 * again — afterwards `requestGenerate` runs `generate` straight through.
 */
export function useContactPromptGate(generate: () => Promise<unknown> | void) {
  const contactPromptSeen = useContactPromptSeen();
  const setContactPromptSeen = usePreferencesStore((s) => s.setContactPromptSeen);
  const [contactModalOpen, setContactModalOpen] = useState(false);

  const requestGenerate = () => {
    if (!contactPromptSeen) {
      setContactPromptSeen();
      setContactModalOpen(true);
      return;
    }
    void generate();
  };

  const continueFromContactPrompt = () => {
    setContactModalOpen(false);
    void generate();
  };

  return {
    contactModalOpen,
    closeContactModal: () => setContactModalOpen(false),
    requestGenerate,
    continueFromContactPrompt,
  };
}
