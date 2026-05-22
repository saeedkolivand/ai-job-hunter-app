import { Sparkles, Star, Zap } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { IconText } from '../components/IconText';

const meta = {
  component: IconText,
  tags: ['autodocs'],
  argTypes: {
    gap: { control: 'select', options: ['sm', 'md', 'lg'] },
  },
} satisfies Meta<typeof IconText>;
export default meta;
type Story = StoryObj<typeof IconText>;

export const Default: Story = {
  args: { icon: <Sparkles size={14} />, children: 'Generate', gap: 'md' },
};

export const AllGaps: Story = {
  render: () => (
    <div className="flex flex-col gap-3 text-sm text-foreground/80">
      <IconText icon={<Star size={14} />} gap="sm">
        Small gap
      </IconText>
      <IconText icon={<Star size={14} />} gap="md">
        Medium gap
      </IconText>
      <IconText icon={<Star size={14} />} gap="lg">
        Large gap
      </IconText>
    </div>
  ),
};

export const InText: Story = {
  render: () => (
    <p className="text-sm text-foreground/80">
      Click <IconText icon={<Zap size={12} />}>Run</IconText> to start.
    </p>
  ),
};
