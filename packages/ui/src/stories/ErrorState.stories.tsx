import type { Meta, StoryObj } from '@storybook/react-vite';

import { ErrorState } from '../components/ErrorState';

const meta = {
  component: ErrorState,
  tags: ['autodocs'],
  argTypes: {
    onRetry: { action: 'retry clicked' },
  },
} satisfies Meta<typeof ErrorState>;
export default meta;
type Story = StoryObj<typeof ErrorState>;

export const Default: Story = {};

export const WithMessage: Story = {
  args: {
    title: 'Failed to load jobs',
    description: 'There was a problem connecting to the scraper. Check your network and try again.',
  },
};

export const WithRetry: Story = {
  args: {
    title: 'Something went wrong',
    description: 'The request timed out.',
    onRetry: () => {},
  },
};
