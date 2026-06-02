import { AISelectionStep } from './steps/AISelectionStep';
import { BrowserStep } from './steps/BrowserStep';
import { ResearchStep } from './steps/ResearchStep';
import { ResumeStep } from './steps/ResumeStep';
import { WelcomeStep } from './steps/WelcomeStep';

export const ONBOARDING_STEPS = [
  { id: 'welcome', component: WelcomeStep },
  { id: 'resume', component: ResumeStep },
  { id: 'ai', component: AISelectionStep },
  { id: 'research', component: ResearchStep },
  { id: 'browser', component: BrowserStep },
] as const;

export type StepId = (typeof ONBOARDING_STEPS)[number]['id'];
export const TOTAL_STEPS = ONBOARDING_STEPS.length;
