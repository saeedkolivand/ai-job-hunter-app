import { createMachine } from '@/lib/machine';

/**
 * Autopilot creation wizard state machine.
 *
 * The wizard has 4 steps. Each NEXT transition advances by one step.
 * PREV goes back. SUBMIT on the final step saves and closes the wizard.
 *
 * States:
 *   step_0  → Basic info (name, description)
 *   step_1  → Job target (boards, search queries)
 *   step_2  → Filters (salary, location, remote, tech stack)
 *   step_3  → Schedule (frequency, time window)
 *   saving  → Calling window.api.autopilot.create
 *   done    → Successfully created
 *   error   → Save failed
 *
 * Valid transitions:
 *   step_0 ⇄ step_1 ⇄ step_2 ⇄ step_3 → saving → done
 *                                            ↘ error → step_3 (retry)
 */

export type AutopilotWizardState =
  | 'step_0'
  | 'step_1'
  | 'step_2'
  | 'step_3'
  | 'saving'
  | 'done'
  | 'error';

export type AutopilotWizardEvent =
  | 'NEXT' // advance one step
  | 'PREV' // go back one step
  | 'SUBMIT' // save on final step
  | 'SAVED' // save succeeded
  | 'ERROR' // save failed
  | 'RESET'; // start over

export const autopilotWizardMachine = createMachine<AutopilotWizardState, AutopilotWizardEvent>({
  transitions: {
    step_0: { NEXT: 'step_1', RESET: 'step_0' },
    step_1: { NEXT: 'step_2', PREV: 'step_0' },
    step_2: { NEXT: 'step_3', PREV: 'step_1' },
    step_3: { SUBMIT: 'saving', PREV: 'step_2' },
    saving: { SAVED: 'done', ERROR: 'error' },
    done: { RESET: 'step_0' },
    error: { RESET: 'step_3' }, // go back to the last step to retry
  },
  busyStates: ['saving'],
  errorStates: ['error'],
});

/** Derive the numeric step index (0-3) from the wizard state. */
export function wizardStepIndex(state: AutopilotWizardState): number {
  const map: Partial<Record<AutopilotWizardState, number>> = {
    step_0: 0,
    step_1: 1,
    step_2: 2,
    step_3: 3,
    saving: 3,
    done: 3,
    error: 3,
  };
  return map[state] ?? 0;
}
