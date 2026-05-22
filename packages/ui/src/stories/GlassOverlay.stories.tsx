import type { Meta, StoryObj } from '@storybook/react';

import { GlassOverlay } from '../components/GlassOverlay';

const meta: Meta<typeof GlassOverlay> = {
  title: 'Overlays/GlassOverlay',
  component: GlassOverlay,
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
};
export default meta;
type Story = StoryObj<typeof GlassOverlay>;

export const Default: Story = {
  render: () => (
    <div className="relative h-64 overflow-hidden rounded-xl bg-gradient-to-br from-purple-900 to-slate-900 p-6">
      <p className="text-sm text-white/60">Content behind the overlay</p>
      <GlassOverlay zIndex={10} />
    </div>
  ),
};

export const Clickable: Story = {
  render: () => (
    <div className="relative h-64 overflow-hidden rounded-xl bg-gradient-to-br from-purple-900 to-slate-900 p-6">
      <p className="text-sm text-white/60">Click the overlay</p>
      <GlassOverlay zIndex={10} onClick={() => alert('overlay clicked')} />
    </div>
  ),
};
