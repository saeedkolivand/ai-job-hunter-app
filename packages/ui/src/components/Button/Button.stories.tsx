import { Download, Sparkles, Trash2 } from 'lucide-react';
import { expect } from 'storybook/test';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';

const meta = {
  component: Button,
  tags: ['autodocs'],
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'glass', 'ghost', 'danger', 'warning', 'info', 'success'],
    },
    size: { control: 'select', options: ['sm', 'md', 'lg'] },
    loading: { control: 'boolean' },
    disabled: { control: 'boolean' },
  },
} satisfies Meta<typeof Button>;
export default meta;
type Story = StoryObj<typeof Button>;

export const Default: Story = {
  args: { children: 'Button', variant: 'default', size: 'md' },
};

export const Glass: Story = {
  args: { children: 'Glass', variant: 'glass', size: 'md' },
};

export const Ghost: Story = {
  args: { children: 'Ghost', variant: 'ghost', size: 'md' },
};

export const AllVariants: Story = {
  render: () => (
    <div className="flex flex-wrap gap-3">
      {(['default', 'glass', 'ghost', 'danger', 'warning', 'info', 'success'] as const).map((v) => (
        <Button key={v} variant={v}>
          {v}
        </Button>
      ))}
    </div>
  ),
};

export const AllSizes: Story = {
  render: () => (
    <div className="flex items-center gap-3">
      <Button size="sm">Small</Button>
      <Button size="md">Medium</Button>
      <Button size="lg">Large</Button>
    </div>
  ),
};

export const WithIcon: Story = {
  render: () => (
    <div className="flex gap-3">
      <Button variant="glass" size="md">
        <Sparkles size={14} /> Generate
      </Button>
      <Button variant="danger" size="md">
        <Trash2 size={14} /> Delete
      </Button>
      <Button variant="success" size="md">
        <Download size={14} /> Export
      </Button>
    </div>
  ),
};

export const Loading: Story = {
  args: { children: 'Saving…', variant: 'glass', loading: true },
};

export const Disabled: Story = {
  args: { children: 'Disabled', variant: 'glass', disabled: true },
};

// Proof that the shared preview actually loaded Tailwind + the @ajh/ui design
// system. `toBeVisible` passes even on an unstyled button, so we assert concrete
// computed values from base classes the Button always applies: `inline-flex`
// (display) and `font-medium` (font-weight 500). If the CSS layer fails to load
// — the exact failure mode this preview guards against — the button falls back
// to the UA defaults `inline-block` / 400 and this story fails. Runs live in the
// Interactions panel (and any future Storybook/vitest browser runner).
export const CssCheck: Story = {
  tags: ['ai-generated'],
  args: { children: 'Submit', variant: 'default', size: 'md' },
  play: async ({ canvas }) => {
    const button = canvas.getByRole('button', { name: /submit/i });
    const styles = getComputedStyle(button);
    await expect(styles.display).toBe('inline-flex');
    await expect(styles.fontWeight).toBe('500');
  },
};
