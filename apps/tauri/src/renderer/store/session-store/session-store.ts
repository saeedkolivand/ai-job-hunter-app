import { create } from 'zustand';

import type { InterviewAnswers } from '@ajh/prompts/builder';
import type { AutopilotFoundJob } from '@ajh/shared';

import type { TailorWizardState } from '@/features/autopilot/components/ApplyPage/lib/tailor-state';
import type { WizardState } from '@/features/autopilot/types';
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

type ResumeBuilderStage = 'interview' | 'generating' | 'done';

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
  | 'developer';

interface SettingsSlice {
  activeSection: SettingsSection;
}

export interface AutopilotApplyTarget {
  job: AutopilotFoundJob;
  resumeText?: string;
  board: string;
}

interface AutopilotSlice {
  creating: boolean;
  editingId: string | null;
  wizardStep: number;
  wizardForm: WizardState | null;
  focusedId: string | null;
  apply: AutopilotApplyTarget | null;
  applyWizardStep: number;
  applyWizardForm: TailorWizardState | null;
  applyTemplateId: TemplateId;
  applyAtsMode: boolean;
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
  applications: ApplicationsSlice;

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

  setApplications: (patch: Partial<ApplicationsSlice>) => void;
  toggleApplicationSection: (stageId: string) => void;
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
  applications: { collapsedSections: [], filter: '' },

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
