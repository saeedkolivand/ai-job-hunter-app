import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { NumberField } from './NumberField';

const meta = {
  component: NumberField,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof NumberField>;
export default meta;
type Story = StoryObj<typeof NumberField>;

function Demo({ initial = 8, min, max }: { initial?: number; min?: number; max?: number }) {
  const [value, setValue] = useState(initial);
  return (
    <div className="w-40">
      <NumberField
        value={value}
        onChange={setValue}
        fallback={initial}
        min={min}
        max={max}
        step={1}
      />
    </div>
  );
}

export const Default: Story = { render: () => <Demo /> };

export const Bounded: Story = { render: () => <Demo initial={10} min={1} max={20} /> };
