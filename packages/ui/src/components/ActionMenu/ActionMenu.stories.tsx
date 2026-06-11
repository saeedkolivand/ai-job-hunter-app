import { Copy, Pencil, Trash2 } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { ActionMenu, type ActionMenuItem } from './ActionMenu';

const meta = {
  component: ActionMenu,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof ActionMenu>;
export default meta;
type Story = StoryObj<typeof ActionMenu>;

const items: ActionMenuItem[] = [
  { label: 'Edit', icon: <Pencil size={14} />, onSelect: () => {} },
  { label: 'Duplicate', icon: <Copy size={14} />, onSelect: () => {} },
  { label: 'Delete', icon: <Trash2 size={14} />, destructive: true, onSelect: () => {} },
];

export const Default: Story = { args: { items } };

export const AlignStart: Story = { args: { align: 'start', items } };

export const WithDisabledItem: Story = {
  args: {
    items: [
      ...items.slice(0, 2),
      { label: 'Archive', onSelect: () => {}, disabled: true },
      items[2] as ActionMenuItem,
    ],
  },
};
