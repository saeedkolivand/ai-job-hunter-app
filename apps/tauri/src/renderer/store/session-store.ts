import { create } from 'zustand';

import type { WizardState } from '@/features/autopilot/types';
import type { GenerationMeta, GenerationMode, TemplateId } from '@/lib/generate-ai';
import type { AnalysisResult } from '@/lib/resume-ai';

// ─── Per-route state shapes ───────────────────────────────────────────────────

type AIGenerateStage = 'idle' | 'extracting' | 'configuring' | 'generating' | 'done';
type AIGenerateTarget = 'resume' | 'cover' | 'both';

interface AIGenerateSlice {
  resume: string;
  jobAd: string;
  stage: AIGenerateStage;
  meta: GenerationMeta | null;
  mode: GenerationMode;
  target: AIGenerateTarget;
  templateId: TemplateId;
  atsMode: boolean;
  resumeOut: string;
  coverOut: string;
  activeOut: 'resume' | 'cover';
}

type AnalyzeStage = 'idle' | 'running' | 'done';

interface AnalyzeSlice {
  resume: string;
  jobAd: string;
  stage: AnalyzeStage;
  result: AnalysisResult | null;
}

interface JobsSlice {
  filter: string;
  sortBy: 'newest' | 'oldest' | 'company';
}

type ResumesTab = 'applied' | 'viewed' | 'bookmarked' | 'generated';

interface ResumesSlice {
  tab: ResumesTab;
  filter: string;
}

type SettingsSection =
  | 'general'
  | 'ai'
  | 'job'
  | 'resume'
  | 'accounts'
  | 'privacy'
  | 'performance'
  | 'developer';

interface SettingsSlice {
  activeSection: SettingsSection;
}

interface AutopilotSlice {
  creating: boolean;
  // Set when the wizard is editing an existing autopilot; null when creating.
  editingId: string | null;
  wizardStep: number;
  wizardForm: WizardState | null;
}

// ─── Defaults ─────────────────────────────────────────────────────────────────

const AI_GENERATE_DEFAULTS: AIGenerateSlice = {
  resume: '',
  jobAd: '',
  stage: 'idle',
  meta: null,
  mode: 'ats',
  target: 'both',
  templateId: 'modern',
  atsMode: false,
  resumeOut: '',
  coverOut: '',
  activeOut: 'resume',
};

const ANALYZE_DEFAULTS: AnalyzeSlice = {
  resume: '',
  jobAd: '',
  stage: 'idle',
  result: null,
};

// ─── Store ────────────────────────────────────────────────────────────────────

interface SessionState {
  aiGenerate: AIGenerateSlice;
  analyze: AnalyzeSlice;
  jobs: JobsSlice;
  resumes: ResumesSlice;
  settings: SettingsSlice;
  autopilot: AutopilotSlice;

  setAIGenerate: (patch: Partial<AIGenerateSlice>) => void;
  resetAIGenerate: () => void;

  setAnalyze: (patch: Partial<AnalyzeSlice>) => void;
  resetAnalyze: () => void;

  setJobs: (patch: Partial<JobsSlice>) => void;

  setResumes: (patch: Partial<ResumesSlice>) => void;

  setSettings: (patch: Partial<SettingsSlice>) => void;

  setAutopilot: (patch: Partial<AutopilotSlice>) => void;
  resetAutopilotWizard: () => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  aiGenerate: { ...AI_GENERATE_DEFAULTS },
  analyze: { ...ANALYZE_DEFAULTS },
  jobs: { filter: '', sortBy: 'newest' },
  resumes: { tab: 'applied', filter: '' },
  settings: { activeSection: 'general' },
  autopilot: { creating: false, editingId: null, wizardStep: 0, wizardForm: null },

  setAIGenerate: (patch) => set((s) => ({ aiGenerate: { ...s.aiGenerate, ...patch } })),
  resetAIGenerate: () => set({ aiGenerate: { ...AI_GENERATE_DEFAULTS } }),

  setAnalyze: (patch) => set((s) => ({ analyze: { ...s.analyze, ...patch } })),
  resetAnalyze: () => set({ analyze: { ...ANALYZE_DEFAULTS } }),

  setJobs: (patch) => set((s) => ({ jobs: { ...s.jobs, ...patch } })),

  setResumes: (patch) => set((s) => ({ resumes: { ...s.resumes, ...patch } })),

  setSettings: (patch) => set((s) => ({ settings: { ...s.settings, ...patch } })),

  setAutopilot: (patch) => set((s) => ({ autopilot: { ...s.autopilot, ...patch } })),
  resetAutopilotWizard: () =>
    set((s) => ({
      autopilot: {
        ...s.autopilot,
        creating: false,
        editingId: null,
        wizardStep: 0,
        wizardForm: null,
      },
    })),
}));
