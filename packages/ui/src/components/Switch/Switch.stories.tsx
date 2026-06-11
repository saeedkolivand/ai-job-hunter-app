import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Switch } from '../Switch';

const meta = {
  component: Switch,
  tags: ['autodocs'],
  argTypes: {
    checked: { control: 'boolean' },
    size: { control: 'select', options: ['sm', 'md'] },
    label: { control: 'text' },
    description: { control: 'text' },
    disabled: { control: 'boolean' },
  },
} satisfies Meta<typeof Switch>;
export default meta;
type Story = StoryObj<typeof Switch>;

/** Controlled wrapper so the knob actually moves in the Storybook canvas. */
function Controlled({
  initial = false,
  ...rest
}: { initial?: boolean } & Omit<
  React.ComponentProps<typeof Switch>,
  'checked' | 'onCheckedChange'
>) {
  const [checked, setChecked] = useState(initial);
  return <Switch checked={checked} onCheckedChange={setChecked} {...rest} />;
}

export const Off: Story = {
  render: () => <Controlled initial={false} aria-label="Toggle setting" />,
};

export const On: Story = {
  render: () => <Controlled initial aria-label="Toggle setting" />,
};

export const Sizes: Story = {
  render: () => (
    <div className="flex items-center gap-6">
      <Controlled initial size="sm" aria-label="Small switch" />
      <Controlled initial size="md" aria-label="Medium switch" />
    </div>
  ),
};

export const WithLabel: Story = {
  render: () => (
    <div className="w-80">
      <Controlled
        label="Reduce transparency"
        description="Use opaque surfaces instead of frosted glass."
      />
    </div>
  ),
};

export const Disabled: Story = {
  render: () => (
    <div className="flex flex-col gap-3">
      <Controlled initial={false} disabled aria-label="Disabled off" />
      <Controlled initial disabled aria-label="Disabled on" />
    </div>
  ),
};
