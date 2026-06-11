import { create } from 'zustand';

import type { InterviewAnswers } from '@ajh/prompts/builder';
import type { AutopilotFoundJob } from '@ajh/shared';

import type { TailorWizardState } from '@/features/autopilot/components/ApplyPage/lib/tailor-state';
import type { WizardState } from '@/features/autopilot/types';
import type { EmphasisId, GenerationMeta, GenerationMode, TemplateId } from '@/lib/generate';
import type { AnalysisMode, AnalysisResult } from '@/lib/resume-ai';

// ─── Per-route state shapes ───────────────────────────────────────────────────

type AIGenerateStage = 'idle' | 'extracting' | 'configuring' | 'generating' | 'done';
type AIGenerateTarget = 'resume' | 'cover' | 'both';

interface AIGenerateSlice {
  resume: string;
  jobAd: string;
  stage: AIGenerateStage;
  meta: GenerationMeta | null;
  mode: GenerationMode;
  /** User-selected emphasis directives (#15) — fact-safe rewrite biases. */
  emphasis: EmphasisId[];
  target: AIGenerateTarget;
  templateId: TemplateId;
  atsMode: boolean;
  /** Target market id (`us`, `de`, …); drives export page size. */
  locale: string;
  resumeOut: string;
  coverOut: string;
  activeOut: 'resume' | 'cover';
  /** Current step index in the GenerateWizard (0-based). Reset to 0 on resetAIGenerate. */
  wizardStep: number;
}

type AnalyzeStage = 'idle' | 'running' | 'done';

interface AnalyzeSlice {
  resume: string;
  jobAd: string;
  stage: AnalyzeStage;
  result: AnalysisResult | null;
  /** Evaluate as a corporate résumé (default) or an academic CV (#54). */
  analysisMode: AnalysisMode;
}

/** Resume Builder (#1 / B9) — interview answers + wizard/synthesis state. */
type ResumeBuilderStage = 'interview' | 'generating' | 'done';

export interface ResumeBuilderSlice {
  /** Structured interview answers — the grounding source for synthesis. */
  answers: InterviewAnswers;
  /** Current wizard step index (0-based). */
  wizardStep: number;
  stage: ResumeBuilderStage;
  /** Synthesized résumé markdown (populated when stage === 'done'). */
  output: string;
  /** Output language for synthesis (e.g. `en`, `de`) — drives section headers. */
  language: string;
  /** Export market id (`us`, `de`, …) — drives page size; defaults from language. */
  locale: string;
  templateId: TemplateId;
  atsMode: boolean;
}

interface JobsSlice {
  filter: string;
  sortBy: 'newest' | 'oldest' | 'company';
}

type ResumesTab = 'resumes' | 'coverLetters' | 'activity';

interface ResumesSlice {
  tab: ResumesTab;
  filter: string;
}

export type SettingsSection =
  | 'general'
  | 'contact'
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

/** The found job + its autopilot context, opened on the dedicated apply page (#51). */
export interface AutopilotApplyTarget {
  job: AutopilotFoundJob;
  /** The autopilot's base resume, pre-filled on the apply page. */
  resumeText?: string;
  /** The board the job came from — stored on the saved application record. */
  board: string;
}

interface AutopilotSlice {
  creating: boolean;
  // Set when the wizard is editing an existing autopilot; null when creating.
  editingId: string | null;
  wizardStep: number;
  wizardForm: WizardState | null;
  // Set by a tray "New jobs" click / deep link to auto-expand & scroll to a
  // card's found-jobs; the card clears it once handled.
  focusedId: string | null;
  // Set when the user opens the dedicated apply page for a found job (#51);
  // cleared on Back. Null shows the autopilot list.
  apply: AutopilotApplyTarget | null;
  // Tailoring wizard state on the apply page. Persisted (like wizardStep/
  // wizardForm) so the configuring stage survives remounts; cleared alongside
  // `apply` on Back so a different job starts fresh.
  applyWizardStep: number;
  applyWizardForm: TailorWizardState | null;
  // Sticky render-time template preference for the apply results screen. Unlike
  // applyWizardStep/applyWizardForm these survive Back (and switching jobs) so the
  // chosen template/ATS mode persists across the whole autopilot session. Template/
  // ATS are render-time only — switching them never regenerates.
  applyTemplateId: TemplateId;
  applyAtsMode: boolean;
}

// ─── Defaults ─────────────────────────────────────────────────────────────────

const AI_GENERATE_DEFAULTS: AIGenerateSlice = {
  resume: '',
  jobAd: '',
  stage: 'idle',
  meta: null,
  mode: 'ats',
  emphasis: [],
  target: 'both',
  templateId: 'modern',
  atsMode: false,
  locale: 'en',
  resumeOut: '',
  coverOut: '',
  activeOut: 'resume',
  wizardStep: 0,
};

const ANALYZE_DEFAULTS: AnalyzeSlice = {
  resume: '',
  jobAd: '',
  stage: 'idle',
  result: null,
  analysisMode: 'work',
};

const RESUME_BUILDER_DEFAULTS: ResumeBuilderSlice = {
  answers: {
    fullName: '',
    headline: '',
    summary: '',
    experience: [],
    education: [],
    skills: [],
    projects: [],
    publications: [],
    awards: [],
    volunteer: [],
    languages: [],
    certifications: [],
  },
  wizardStep: 0,
  stage: 'interview',
  output: '',
  language: 'en',
  locale: 'en',
  templateId: 'modern',
  atsMode: false,
};

// ─── Store ────────────────────────────────────────────────────────────────────

interface SessionState {
  aiGenerate: AIGenerateSlice;
  analyze: AnalyzeSlice;
  resumeBuilder: ResumeBuilderSlice;
  jobs: JobsSlice;
  resumes: ResumesSlice;
  settings: SettingsSlice;
  autopilot: AutopilotSlice;

  setAIGenerate: (patch: Partial<AIGenerateSlice>) => void;
  resetAIGenerate: () => void;

  setAnalyze: (patch: Partial<AnalyzeSlice>) => void;
  resetAnalyze: () => void;

  setResumeBuilder: (patch: Partial<ResumeBuilderSlice>) => void;
  resetResumeBuilder: () => void;

  setJobs: (patch: Partial<JobsSlice>) => void;

  setResumes: (patch: Partial<ResumesSlice>) => void;

  setSettings: (patch: Partial<SettingsSlice>) => void;

  setAutopilot: (patch: Partial<AutopilotSlice>) => void;
  resetAutopilotWizard: () => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  aiGenerate: { ...AI_GENERATE_DEFAULTS },
  analyze: { ...ANALYZE_DEFAULTS },
  resumeBuilder: { ...RESUME_BUILDER_DEFAULTS },
  jobs: { filter: '', sortBy: 'newest' },
  resumes: { tab: 'resumes', filter: '' },
  settings: { activeSection: 'general' },
  autopilot: {
    creating: false,
    editingId: null,
    wizardStep: 0,
    wizardForm: null,
    focusedId: null,
    apply: null,
    applyWizardStep: 0,
    applyWizardForm: null,
    applyTemplateId: 'modern',
    applyAtsMode: false,
  },

  setAIGenerate: (patch) => set((s) => ({ aiGenerate: { ...s.aiGenerate, ...patch } })),
  resetAIGenerate: () => set({ aiGenerate: { ...AI_GENERATE_DEFAULTS } }),

  setAnalyze: (patch) => set((s) => ({ analyze: { ...s.analyze, ...patch } })),
  resetAnalyze: () => set({ analyze: { ...ANALYZE_DEFAULTS } }),

  setResumeBuilder: (patch) => set((s) => ({ resumeBuilder: { ...s.resumeBuilder, ...patch } })),
  resetResumeBuilder: () => set({ resumeBuilder: { ...RESUME_BUILDER_DEFAULTS } }),

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
