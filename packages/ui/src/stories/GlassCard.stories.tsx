import type { Meta, StoryObj } from '@storybook/react';
import { GlassCard } from '../components/GlassCard';

const meta: Meta<typeof GlassCard> = {
  title: 'Surfaces/GlassCard',
  component: GlassCard,
  tags: ['autodocs'],
  argTypes: {
    tone: { control: 'select', options: ['neutral', 'violet', 'indigo', 'graphite'] },
    highlight: { control: 'boolean' },
    glow: { control: 'boolean' },
  },
};
export default meta;
type Story = StoryObj<typeof GlassCard>;

const Content = () => (
  <>
    <div className="mb-2 text-sm font-semibold text-foreground/80">Card Title</div>
    <div className="text-xs text-foreground/50">
      This is a glass card with some content. It uses backdrop-filter blur and a subtle border.
    </div>
  </>
);

export const Neutral: Story = {
  args: { tone: 'neutral', highlight: true, glow: false, children: <Content /> },
};

export const Violet: Story = {
  args: { tone: 'violet', highlight: true, glow: true, children: <Content /> },
};

export const Indigo: Story = {
  args: { tone: 'indigo', highlight: true, glow: false, children: <Content /> },
};

export const Graphite: Story = {
  args: { tone: 'graphite', highlight: false, glow: false, children: <Content /> },
};

export const AllTones: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-4 w-[600px]">
      {(['neutral', 'violet', 'indigo', 'graphite'] as const).map((tone) => (
        <GlassCard key={tone} tone={tone} highlight>
          <div className="text-xs font-medium text-foreground/60 capitalize mb-1">{tone}</div>
          <div className="text-xs text-foreground/40">Glass card content</div>
        </GlassCard>
      ))}
    </div>
  ),
};
