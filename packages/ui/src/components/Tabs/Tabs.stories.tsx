import { Briefcase, FileText, StickyNote } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { type TabItem, Tabs } from './Tabs';

const meta = {
  component: Tabs,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof Tabs>;
export default meta;
type Story = StoryObj<typeof Tabs>;

type Section = 'overview' | 'details' | 'notes';

const ITEMS: readonly TabItem<Section>[] = [
  { value: 'overview', label: 'Overview' },
  { value: 'details', label: 'Details' },
  { value: 'notes', label: 'Notes' },
];

const ITEMS_WITH_ICONS: readonly TabItem<Section>[] = [
  { value: 'overview', label: 'Overview', icon: Briefcase },
  { value: 'details', label: 'Details', icon: FileText },
  { value: 'notes', label: 'Notes', icon: StickyNote },
];

function Demo({ items, size }: { items: readonly TabItem<Section>[]; size?: 'sm' | 'md' }) {
  const [value, setValue] = useState<Section>('overview');
  return (
    <div className="w-80">
      <Tabs
        ariaLabel="Application sections"
        items={items}
        value={value}
        onChange={setValue}
        size={size}
      />
    </div>
  );
}

export const Default: Story = { render: () => <Demo items={ITEMS} /> };

export const WithIcons: Story = { render: () => <Demo items={ITEMS_WITH_ICONS} /> };

export const SizeMd: Story = { render: () => <Demo items={ITEMS_WITH_ICONS} size="md" /> };
