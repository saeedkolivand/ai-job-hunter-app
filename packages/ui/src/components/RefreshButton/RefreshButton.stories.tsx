import type { Meta, StoryObj } from '@storybook/react-vite';

import { RefreshButton } from './RefreshButton';

const meta = {
  component: RefreshButton,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
  args: { onRefresh: () => {} },
} satisfies Meta<typeof RefreshButton>;
export default meta;
type Story = StoryObj<typeof RefreshButton>;

export const Default: Story = {};

export const WithLabel: Story = { args: { variant: 'glass', children: 'Refresh' } };

// Resolves after a delay so the spinner animation is visible on click.
export const AsyncSpin: Story = {
  args: {
    variant: 'glass',
    children: 'Reload',
    onRefresh: () => new Promise((resolve) => setTimeout(resolve, 1500)),
  },
};

export const Disabled: Story = { args: { variant: 'glass', children: 'Refresh', disabled: true } };
