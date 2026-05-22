import { Briefcase, FileText, Search, Settings, Sparkles, Zap } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { ActionTile } from '../components/ActionTile';

const meta = {
  component: ActionTile,
  tags: ['autodocs'],
  argTypes: {
    active: { control: 'boolean' },
  },
} satisfies Meta<typeof ActionTile>;
export default meta;
type Story = StoryObj<typeof ActionTile>;

export const Default: Story = {
  args: { icon: Sparkles, label: 'AI Generate', description: 'Create tailored resumes' },
};

export const Active: Story = {
  args: {
    icon: Sparkles,
    label: 'AI Generate',
    description: 'Create tailored resumes',
    active: true,
  },
};

export const Grid: Story = {
  render: () => (
    <div className="grid grid-cols-3 gap-4 w-[500px]">
      {[
        { icon: Sparkles, label: 'AI', description: 'Generate content' },
        { icon: Briefcase, label: 'Jobs', description: 'Browse listings' },
        { icon: FileText, label: 'Documents', description: 'Manage resumes' },
        { icon: Search, label: 'Search', description: 'Find jobs' },
        { icon: Zap, label: 'Autopilot', description: 'Auto apply' },
        { icon: Settings, label: 'Settings', description: 'Configure app' },
      ].map((t) => (
        <ActionTile key={t.label} icon={t.icon} label={t.label} description={t.description} />
      ))}
    </div>
  ),
};
