import { Brain, Settings, Shield } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { SectionHeader } from '../components/SectionHeader';

const meta = {
  component: SectionHeader,
  tags: ['autodocs'],
  argTypes: {
    size: { control: 'select', options: ['sm', 'md'] },
  },
} satisfies Meta<typeof SectionHeader>;
export default meta;
type Story = StoryObj<typeof SectionHeader>;

export const Default: Story = {
  args: { icon: Settings, title: 'Settings', size: 'md' },
};

export const WithDescription: Story = {
  args: {
    icon: Brain,
    title: 'AI Configuration',
    description: 'Configure your local model and provider settings.',
    size: 'md',
  },
};

export const Small: Story = {
  args: { icon: Shield, title: 'Security', description: 'Manage credentials.', size: 'sm' },
};

export const AllSizes: Story = {
  render: () => (
    <div className="flex flex-col gap-6">
      <SectionHeader icon={Settings} title="Medium Header" description="Default size" size="md" />
      <SectionHeader icon={Settings} title="Small Header" description="Compact size" size="sm" />
    </div>
  ),
};
