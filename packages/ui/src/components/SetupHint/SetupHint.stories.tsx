import { WifiOff } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { SetupHint } from './SetupHint';

const meta = {
  component: SetupHint,
  tags: ['autodocs'],
  parameters: { layout: 'padded' },
  args: { message: 'Connect an AI provider to generate résumés.' },
  decorators: [
    (Story) => (
      <div className="w-[28rem]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof SetupHint>;
export default meta;
type Story = StoryObj<typeof SetupHint>;

export const MessageOnly: Story = {};

export const WithAction: Story = {
  args: {
    message: 'No AI provider is connected yet.',
    actionLabel: 'Open settings',
    onAction: () => {},
  },
};

export const Amber: Story = {
  args: {
    tone: 'amber',
    icon: WifiOff,
    message: 'Your LinkedIn session expired.',
    actionLabel: 'Reconnect',
    onAction: () => {},
  },
};

export const Pending: Story = {
  args: {
    message: 'Installing the CLI agent…',
    actionLabel: 'Install',
    onAction: () => {},
    pending: true,
  },
};
