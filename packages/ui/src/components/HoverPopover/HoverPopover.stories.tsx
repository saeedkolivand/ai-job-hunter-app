import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';
import { HoverPopover } from './HoverPopover';

const meta = {
  component: HoverPopover,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof HoverPopover>;
export default meta;
type Story = StoryObj<typeof HoverPopover>;

const PANEL = 'dropdown-surface max-w-xs rounded-xl p-3 text-xs text-foreground/80';

export const Top: Story = {
  render: () => (
    <HoverPopover
      ariaLabel="Details"
      placement="top"
      contentClassName={PANEL}
      trigger={
        <Button variant="glass" size="sm">
          Hover me (top)
        </Button>
      }
    >
      Opens upward on hover or keyboard focus. Move onto the panel to keep it open.
    </HoverPopover>
  ),
};

export const Bottom: Story = {
  render: () => (
    <HoverPopover
      ariaLabel="Details"
      placement="bottom"
      contentClassName={PANEL}
      trigger={
        <Button variant="glass" size="sm">
          Hover me (bottom)
        </Button>
      }
    >
      Opens downward.
    </HoverPopover>
  ),
};
