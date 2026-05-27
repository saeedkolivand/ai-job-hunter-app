import { Bot, Globe, User } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { FloatingIcon } from '../FloatingIcon';

const meta = {
  component: FloatingIcon,
  tags: ['autodocs'],
  argTypes: {
    size: { control: 'number', min: 12, max: 48 },
  },
} satisfies Meta<typeof FloatingIcon>;
export default meta;
type Story = StoryObj<typeof FloatingIcon>;

export const Default: Story = {
  args: { icon: Bot, size: 24 },
};

export const Small: Story = {
  args: { icon: Globe, size: 16 },
};

export const Large: Story = {
  args: { icon: User, size: 32 },
};

export const WithCustomContent: Story = {
  args: {
    icon: Bot,
    children: <span className="text-2xl">🎯</span>,
  },
};

export const AllIcons: Story = {
  render: () => (
    <div className="flex gap-8">
      <FloatingIcon icon={Bot} size={24} />
      <FloatingIcon icon={Globe} size={24} />
      <FloatingIcon icon={User} size={24} />
    </div>
  ),
};
