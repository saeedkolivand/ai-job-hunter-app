import { Cpu, Globe, Zap } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { SegmentedControl, type SegmentedOption } from './SegmentedControl';

const meta = {
  component: SegmentedControl,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof SegmentedControl>;
export default meta;
type Story = StoryObj<typeof SegmentedControl>;

type Quality = 'full' | 'auto' | 'fast';
const OPTIONS: readonly SegmentedOption<Quality>[] = [
  { value: 'full', label: 'Full' },
  { value: 'auto', label: 'Auto' },
  { value: 'fast', label: 'Fast' },
];

function TrackDemo({ tone }: { tone?: 'brand' | 'neutral' }) {
  const [value, setValue] = useState<Quality>('auto');
  return (
    <SegmentedControl
      options={OPTIONS}
      value={value}
      onChange={setValue}
      tone={tone}
      ariaLabel="Quality"
    />
  );
}

function GridDemo() {
  const [value, setValue] = useState<Quality>('auto');
  const options: readonly SegmentedOption<Quality>[] = [
    { value: 'full', label: 'Full', icon: Cpu },
    { value: 'auto', label: 'Auto', icon: Globe },
    { value: 'fast', label: 'Fast', icon: Zap },
  ];
  return (
    <div className="w-80">
      <SegmentedControl
        variant="grid"
        options={options}
        value={value}
        onChange={setValue}
        ariaLabel="Quality"
      />
    </div>
  );
}

export const Track: Story = { render: () => <TrackDemo /> };

export const TrackBrand: Story = { render: () => <TrackDemo tone="brand" /> };

export const Grid: Story = { render: () => <GridDemo /> };
