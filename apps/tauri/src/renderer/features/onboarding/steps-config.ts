import { AISelectionStep } from './steps/AISelectionStep';
import { AppearanceStep } from './steps/AppearanceStep';
import { BrowserStep } from './steps/BrowserStep';
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
  { id: 'extension', component: ExtensionStep },
  { id: 'appearance', component: AppearanceStep },
] as const;
