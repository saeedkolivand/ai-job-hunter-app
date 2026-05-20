import type { Meta, StoryObj } from '@storybook/react';
import { Sparkles, Trash2, Download } from 'lucide-react';
import { Button } from '../components/Button';

const meta: Meta<typeof Button> = {
  title: 'Primitives/Button',
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
};
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
