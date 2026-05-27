import type { Meta, StoryObj } from '@storybook/react-vite';

import { StepDots } from '../StepDots';

const meta = {
  component: StepDots,
  tags: ['autodocs'],
  argTypes: {
    currentStep: { control: 'number', min: 0 },
    totalSteps: { control: 'number', min: 1 },
  },
} satisfies Meta<typeof StepDots>;
export default meta;
type Story = StoryObj<typeof StepDots>;

export const Default: Story = {
  args: { currentStep: 0, totalSteps: 4 },
};

export const Step1: Story = {
  args: { currentStep: 0, totalSteps: 4 },
};

export const Step2: Story = {
  args: { currentStep: 1, totalSteps: 4 },
};

export const Step3: Story = {
  args: { currentStep: 2, totalSteps: 4 },
};

export const Step4: Story = {
  args: { currentStep: 3, totalSteps: 4 },
};

export const ThreeSteps: Story = {
  args: { currentStep: 1, totalSteps: 3 },
};

export const FiveSteps: Story = {
  args: { currentStep: 2, totalSteps: 5 },
};
