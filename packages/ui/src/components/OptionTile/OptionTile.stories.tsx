import { Brain, Globe, Zap } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { OptionTile } from '../OptionTile';

const meta = {
  component: OptionTile,
  tags: ['autodocs'],
} satisfies Meta<typeof OptionTile>;
export default meta;
type Story = StoryObj<typeof OptionTile>;

export const Selected: Story = {
  args: {
    icon: Brain,
    label: 'AI Analysis',
    description: 'Powered by local model',
    selected: true,
    onClick: () => {},
  },
};

export const Unselected: Story = {
  args: {
    icon: Globe,
    label: 'Web Scraping',
    description: 'Fetch live job data',
    selected: false,
    onClick: () => {},
  },
};

function GroupDemo() {
  const [selected, setSelected] = useState('ai');
  return (
    <div className="grid grid-cols-3 gap-3">
      {[
        { id: 'ai', icon: Brain, label: 'AI Mode', description: 'Local inference' },
        { id: 'web', icon: Globe, label: 'Web Mode', description: 'Live scraping' },
        { id: 'fast', icon: Zap, label: 'Fast Mode', description: 'Cached results' },
      ].map(({ id, icon, label, description }) => (
        <OptionTile
          key={id}
          icon={icon}
          label={label}
          description={description}
          selected={selected === id}
          onClick={() => setSelected(id)}
          layoutId="option-group"
        />
      ))}
    </div>
  );
}

export const Group: Story = { render: () => <GroupDemo /> };
