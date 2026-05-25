import { BrowserStep } from './steps/BrowserStep';
import { OllamaStep } from './steps/OllamaStep';
import { ResumeStep } from './steps/ResumeStep';
import { WelcomeStep } from './steps/WelcomeStep';

export const ONBOARDING_STEPS = [
  { id: 'welcome', component: WelcomeStep },
  { id: 'resume', component: ResumeStep },
  { id: 'ollama', component: OllamaStep },
  { id: 'browser', component: BrowserStep },
] as const;

export type StepId = (typeof ONBOARDING_STEPS)[number]['id'];
export const TOTAL_STEPS = ONBOARDING_STEPS.length;
