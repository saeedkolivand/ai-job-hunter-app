import { create } from 'zustand';

import type { InterviewAnswers } from '@ajh/prompts/builder';

import type { WizardState } from '@/features/autopilot/types';
import type { TailorWizardState } from '@/features/documents/components/TailorFlow/lib/tailor-state';
import type { EmphasisId, GenerationMeta, GenerationMode, TemplateId } from '@/lib/generate';
import type { AnalysisMode, AnalysisResult } from '@/lib/resume-ai';

// Per-route state shapes

type AIGenerateStage = 'idle' | 'extracting' | 'configuring' | 'generating' | 'done';
type AIGenerateTarget = 'resume' | 'cover' | 'both';

interface AIGenerateSlice {
  resume: string;
  jobAd: string;
  stage: AIGenerateStage;
  meta: GenerationMeta | null;
  mode: GenerationMode;
  emphasis: EmphasisId[];
  target: AIGenerateTarget;
  templateId: TemplateId;
  atsMode: boolean;
  locale: string;
  resumeOut: string;
  coverOut: string;
  activeOut: 'resume' | 'cover';
  wizardStep: number;
}

type AnalyzeStage = 'idle' | 'running' | 'done';

interface AnalyzeSlice {
  resume: string;
  jobAd: string;
  stage: AnalyzeStage;
  result: AnalysisResult | null;
  analysisMode: AnalysisMode;
}

export type ResumeBuilderStage = 'interview' | 'generating' | 'done';

export interface ResumeBuilderSlice {
  answers: InterviewAnswers;
  wizardStep: number;
  stage: ResumeBuilderStage;
  output: string;
  language: string;
  locale: string;
  templateId: TemplateId;
  atsMode: boolean;
}

interface JobsSlice {
  filter: string;
  sortBy: 'newest' | 'oldest' | 'company';
  viewMode: 'list' | 'split';
  selectedId: string | null;
  detailCollapsed: boolean;
}

type ResumesTab = 'resumes' | 'coverLetters' | 'activity';

interface ResumesSlice {
  tab: ResumesTab;
  filter: string;
}

export type SettingsSection =
  | 'general'
  | 'appearance'
  | 'contact'
  | 'ai'
  | 'job'
  | 'resume'
  | 'accounts'
  | 'privacy'
  | 'performance'
  | 'developer'
  | 'about';

interface SettingsSlice {
  activeSection: SettingsSection;
}

interface AutopilotSlice {
  creating: boolean;
  editingId: string | null;
  wizardStep: number;
  wizardForm: WizardState | null;
  focusedId: string | null;
  /**
   * The autopilot the user last applied from (deep-linked into the application
   * detail). Consumed once when the Autopilot page next mounts — promoted to
   * `focusedId` so pressing Back re-expands that card instead of collapsing it.
   */
  lastAppliedId: string | null;
}

/**
 * Application-detail tailoring state — a SEPARATE slice from `autopilot` so the
 * application-detail surface owns its own wizard/template/ATS persistence and
 * never shares configuring state with the autopilot apply flow.
 * applyForId — which application id this configuring state belongs to (used to
 * reset on switch).
 */
interface ApplicationApplySlice {
  applyWizardStep: number;
  applyWizardForm: TailorWizardState | null;
  applyTemplateId: TemplateId;
  applyAtsMode: boolean;
  applyForId: string | null;
  /** One-shot résumé seed for the Documents-tab wizard when arriving from the
   *  autopilot Apply deep-link; cleared when switching to another application. */
  applySeedResume: string | null;
  /** One-shot match-level id (e.g. `strong`) carried from autopilot Apply so the
   *  detail header can show the badge; cleared when switching applications. */
  applyMatchLevel: string | null;
}

/**
 * Applications-page UI state.
 * collapsedSections — stage ids currently collapsed; all sections expanded by default.
 * filter — text filter applied across company/title/candidate.
 * (The "flash a just-imported row once" highlight is now a `?highlight` search
 *  param consumed locally by `ApplicationsPage`, not session state.)
 */
interface ApplicationsSlice {
  collapsedSections: string[];
  filter: string;
}

// Defaults

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

const APPLICATION_APPLY_DEFAULTS: ApplicationApplySlice = {
  applyWizardStep: 0,
  applyWizardForm: null,
  applyTemplateId: 'modern',
  applyAtsMode: false,
  applyForId: null,
  applySeedResume: null,
  applyMatchLevel: null,
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

// Store

interface SessionState {
  aiGenerate: AIGenerateSlice;
  analyze: AnalyzeSlice;
  resumeBuilder: ResumeBuilderSlice;
  jobs: JobsSlice;
  resumes: ResumesSlice;
  settings: SettingsSlice;
  autopilot: AutopilotSlice;
  applicationApply: ApplicationApplySlice;
  applications: ApplicationsSlice;
  /** Session-scoped job-summary cache for flows without a persisted applicationId. */
  jobSummaryCache: Record<string, string>;
  setCachedJobSummary: (key: string, summary: string) => void;

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

  setApplicationApply: (patch: Partial<ApplicationApplySlice>) => void;

  setApplications: (patch: Partial<ApplicationsSlice>) => void;
  toggleApplicationSection: (stageId: string) => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  aiGenerate: { ...AI_GENERATE_DEFAULTS },
  analyze: { ...ANALYZE_DEFAULTS },
  resumeBuilder: { ...RESUME_BUILDER_DEFAULTS },
  jobs: {
    filter: '',
    sortBy: 'newest',
    viewMode: 'split',
    selectedId: null,
    detailCollapsed: false,
  },
  resumes: { tab: 'resumes', filter: '' },
  settings: { activeSection: 'general' },
  autopilot: {
    creating: false,
    editingId: null,
    wizardStep: 0,
    wizardForm: null,
    focusedId: null,
    lastAppliedId: null,
  },
  applicationApply: { ...APPLICATION_APPLY_DEFAULTS },
  applications: { collapsedSections: [], filter: '' },
  jobSummaryCache: {},
  setCachedJobSummary: (key, summary) =>
    set((s) => ({ jobSummaryCache: { ...s.jobSummaryCache, [key]: summary } })),

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

  setApplicationApply: (patch) =>
    set((s) => ({ applicationApply: { ...s.applicationApply, ...patch } })),

  setApplications: (patch) => set((s) => ({ applications: { ...s.applications, ...patch } })),
  toggleApplicationSection: (stageId) =>
    set((s) => {
      const collapsed = s.applications.collapsedSections;
      const next = collapsed.includes(stageId)
        ? collapsed.filter((id) => id !== stageId)
        : [...collapsed, stageId];
      return { applications: { ...s.applications, collapsedSections: next } };
    }),
}));
