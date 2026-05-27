import { Lock, Settings, Shield, Sparkles, User } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { IconBadge } from '../IconBadge';

const meta = {
  component: IconBadge,
  tags: ['autodocs'],
  argTypes: {
    size: { control: 'select', options: ['xs', 'sm', 'md', 'lg'] },
    shape: { control: 'select', options: ['rounded', 'circle', 'square'] },
  },
} satisfies Meta<typeof IconBadge>;
export default meta;
type Story = StoryObj<typeof IconBadge>;

export const Default: Story = {
  args: { icon: Sparkles, size: 'md', shape: 'rounded' },
};

export const AllSizes: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      {(['xs', 'sm', 'md', 'lg'] as const).map((s) => (
        <IconBadge key={s} icon={Sparkles} size={s} />
      ))}
    </div>
  ),
};

export const AllShapes: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <IconBadge icon={Settings} shape="rounded" />
      <IconBadge icon={User} shape="circle" />
      <IconBadge icon={Lock} shape="square" />
    </div>
  ),
};

export const InContext: Story = {
  render: () => (
    <div className="space-y-3 w-64">
      {[
        { icon: User, label: 'Profile' },
        { icon: Lock, label: 'Accounts' },
        { icon: Shield, label: 'Privacy' },
        { icon: Settings, label: 'General' },
      ].map(({ icon, label }) => (
        <div key={label} className="flex items-center gap-3 glass-surface rounded-xl p-3">
          <IconBadge icon={icon} size="sm" />
          <span className="text-sm font-medium text-foreground/80">{label}</span>
        </div>
      ))}
    </div>
  ),
};
