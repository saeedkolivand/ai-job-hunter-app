import type { Meta, StoryObj } from '@storybook/react-vite';

import { Timeline, type TimelineItem } from './index';

const meta = {
  component: Timeline,
  tags: ['autodocs'],
} satisfies Meta<typeof Timeline>;

export default meta;
type Story = StoryObj<typeof Timeline>;

const items: TimelineItem[] = [
  { color: 'green', label: '2024-01-01', children: 'Application created' },
  { color: 'brand', label: '2024-01-03', children: 'Applied' },
  { color: 'blue', label: '2024-01-08', children: 'Phone screen scheduled' },
  { color: 'red', label: '2024-01-15', children: 'Rejected' },
];

export const Default: Story = {
  args: { items },
};

export const LeftMode: Story = {
  args: { items, mode: 'left' },
};

export const Alternate: Story = {
  args: { items, mode: 'alternate' },
};

export const WithPending: Story = {
  args: { items: items.slice(0, 2), pending: 'Waiting for the recruiter to respond…' },
};

export const Reverse: Story = {
  args: { items, reverse: true },
};

export const CustomColor: Story = {
  args: {
    items: [
      { color: '#a855f7', label: 'Step 1', children: 'Custom purple dot (any CSS colour)' },
      { color: 'gray', label: 'Step 2', children: 'Muted gray dot' },
    ],
  },
};
