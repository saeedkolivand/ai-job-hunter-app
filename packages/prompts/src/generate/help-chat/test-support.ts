import type { ProviderProfile } from '../../provider/index.js';
import type { HelpChatAppSection, HelpChatEntry, HelpDataGlanceInput } from './help-chat.js';

export const SMALL: ProviderProfile = { kind: 'ollama', sizeHint: 'small' };
export const LARGE: ProviderProfile = { kind: 'ollama', sizeHint: 'large' };

export const ENTRIES: HelpChatEntry[] = [
  { title: 'How do I export a PDF?', body: 'Open the document and click Export.' },
  { title: 'Why is my job list empty?', body: 'The live job list is cleared on restart.' },
  { title: 'How do I pair the extension?', body: 'Open Settings and copy the pairing code.' },
  { title: 'What leaves my computer?', body: 'Only what your chosen AI provider receives.' },
];

export const GLANCE: HelpDataGlanceInput = {
  documentCount: 3,
  interactionCounts: { viewed: 12, applied: 2, bookmarked: 0 },
  applicationsByStatus: { applied: 4, interview: 1 },
  recentApplications: [{ title: 'Senior Engineer', company: 'Acme', status: 'applied' }],
  autopilotCount: 2,
  autopilots: [
    { name: 'Berlin React roles', status: 'active', runStatus: 'completed', totalFound: 7 },
    { name: 'Remote Rust', status: 'paused', totalFound: 0 },
  ],
};

export const APP_PAGES: HelpChatAppSection[] = [
  { section: 'Job Search', pages: ['Jobs', 'Autopilot', 'Best Matches'] },
  { section: 'Documents', pages: ['Documents', 'AI Generate'] },
];
