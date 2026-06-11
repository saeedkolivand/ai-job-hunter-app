import type { Meta, StoryObj } from '@storybook/react-vite';

import { ProgressBar } from './ProgressBar';

const meta = {
  component: ProgressBar,
  tags: ['autodocs'],
  args: { value: 60 },
  decorators: [
    (Story) => (
      <div className="w-80">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ProgressBar>;
export default meta;
type Story = StoryObj<typeof ProgressBar>;

export const Default: Story = {};

export const Empty: Story = { args: { value: 0 } };

export const Full: Story = { args: { value: 100 } };

export const NoLabel: Story = { args: { value: 45, showLabel: false } };

export const LabelAtStart: Story = { args: { value: 72, labelPosition: 'start' } };
