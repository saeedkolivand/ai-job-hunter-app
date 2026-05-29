import { describe, expect, it } from 'vitest';

import { isBusy, transition } from '@/lib/machine';

import { autopilotWizardMachine, wizardStepIndex } from './autopilot-wizard.machine';

describe('autopilotWizardMachine', () => {
  it('advances and retreats through the steps', () => {
    expect(transition(autopilotWizardMachine, 'step_0', 'NEXT')).toBe('step_1');
    expect(transition(autopilotWizardMachine, 'step_2', 'PREV')).toBe('step_1');
    expect(transition(autopilotWizardMachine, 'step_3', 'SUBMIT')).toBe('saving');
    expect(transition(autopilotWizardMachine, 'saving', 'SAVED')).toBe('done');
  });

  it('returns to the last step on error for retry', () => {
    expect(transition(autopilotWizardMachine, 'saving', 'ERROR')).toBe('error');
    expect(transition(autopilotWizardMachine, 'error', 'RESET')).toBe('step_3');
    expect(isBusy(autopilotWizardMachine, 'saving')).toBe(true);
  });

  it('maps states to a numeric step index', () => {
    expect(wizardStepIndex('step_0')).toBe(0);
    expect(wizardStepIndex('step_2')).toBe(2);
    expect(wizardStepIndex('saving')).toBe(3);
    expect(wizardStepIndex('done')).toBe(3);
  });
});
