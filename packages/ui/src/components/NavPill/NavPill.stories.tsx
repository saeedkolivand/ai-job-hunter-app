import { Briefcase, LayoutDashboard, Settings } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { NavPill } from '../NavPill';

const meta = {
  component: NavPill,
  tags: ['autodocs'],
} satisfies Meta<typeof NavPill>;
export default meta;
type Story = StoryObj<typeof NavPill>;

const ROWS = [
  { icon: LayoutDashboard, label: 'Dashboard' },
  { icon: Briefcase, label: 'Jobs' },
  { icon: Settings, label: 'Settings' },
];

/** Shown in context: the pill sits behind the active row of a vertical nav list. */
export const InNavList: Story = {
  render: () => (
    <nav className="flex w-56 flex-col gap-1 rounded-2xl bg-black/40 p-3">
      {ROWS.map(({ icon: Icon, label }, i) => {
        const active = i === 0;
        return (
          <div key={label} className="relative">
            {active && <NavPill layoutId="story-pill" />}
            <div
              className={`relative flex items-center gap-2.5 rounded-xl px-3 py-2 text-sm ${active ? 'text-foreground' : 'text-foreground/45'}`}
            >
              <Icon size={15} className={active ? 'text-brand-soft' : 'text-foreground/35'} />
              <span className="font-medium">{label}</span>
            </div>
          </div>
        );
      })}
    </nav>
  ),
};
