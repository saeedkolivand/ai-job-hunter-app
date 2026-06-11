import type { Meta, StoryObj } from '@storybook/react-vite';

import { OptionalHint } from './OptionalHint';

const meta = {
  component: OptionalHint,
  tags: ['autodocs'],
} satisfies Meta<typeof OptionalHint>;
export default meta;
type Story = StoryObj<typeof OptionalHint>;

export const Default: Story = {};

export const Custom: Story = {
  args: { children: 'optional — appears on your résumé header' },
};

export const InlineWithLabel: Story = {
  render: () => (
    <label className="flex items-center gap-2 text-sm text-foreground/80">
      Cover letter <OptionalHint />
    </label>
  ),
};
