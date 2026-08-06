import { AdzunaKeyStep } from './steps/AdzunaKeyStep';
import { AISelectionStep } from './steps/AISelectionStep';
import { AppearanceStep } from './steps/AppearanceStep';
import { AutoIndexStep } from './steps/AutoIndexStep';
import { BrowserStep } from './steps/BrowserStep';
import { CrashReportingStep } from './steps/CrashReportingStep';
import { ExtensionStep } from './steps/ExtensionStep';
import { ResearchStep } from './steps/ResearchStep';
import { ResumeStep } from './steps/ResumeStep';
import { WelcomeStep } from './steps/WelcomeStep';

export const ONBOARDING_STEPS = [
  { id: 'welcome', component: WelcomeStep },
  { id: 'resume', component: ResumeStep },
  { id: 'ai', component: AISelectionStep },
  { id: 'research', component: ResearchStep },
  { id: 'browser', component: BrowserStep },
  { id: 'adzunaKey', component: AdzunaKeyStep },
  { id: 'extension', component: ExtensionStep },
  // Sits immediately before the crash-reporting consent: both ask "may this run
  // on your behalf", and auto-indexing calls a provider that may bill per token,
  // so the two spend/privacy decisions stay together rather than scattered.
  { id: 'autoIndex', component: AutoIndexStep },
  // Consent before the finish line: advancing past this step is what unlocks
  // transmission (see CrashReportingStep), so it must be reachable in the normal
  // flow rather than tucked behind an optional branch.
  { id: 'crashReporting', component: CrashReportingStep },
  { id: 'appearance', component: AppearanceStep },
] as const;
