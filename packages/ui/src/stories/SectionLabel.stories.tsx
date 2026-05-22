import { Filter, Star } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { SectionLabel } from '../components/SectionLabel';

const meta = {
  component: SectionLabel,
  tags: ['autodocs'],
} satisfies Meta<typeof SectionLabel>;
export default meta;
type Story = StoryObj<typeof SectionLabel>;

export const Default: Story = {
  args: { children: 'Filters' },
};

export const WithIcon: Story = {
  args: { icon: Filter, children: 'Filters' },
};

export const InContext: Story = {
  render: () => (
    <div className="flex flex-col gap-4 rounded-xl border border-white/10 bg-white/5 p-4">
      <SectionLabel icon={Star}>Highlights</SectionLabel>
      <p className="text-sm text-foreground/60">Section content goes here.</p>
    </div>
  ),
};
