import { render } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { StepTemplate } from './index';

export const letterOption = (id: string) => `${TEST_IDS.generation.letterLayoutOption}-${id}`;

/** Render the step with the given handler mocks; `props` override the defaults. */
export const renderStepWith = (
  onTemplateChange: () => void,
  onAtsModeChange: () => void,
  props: Partial<Parameters<typeof StepTemplate>[0]> = {}
) =>
  render(
    <StepTemplate
      templateId="classic"
      atsMode={false}
      onTemplateChange={onTemplateChange}
      onAtsModeChange={onAtsModeChange}
      {...props}
    />
  );
