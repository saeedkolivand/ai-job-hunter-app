import type { Meta, StoryObj } from '@storybook/react-vite';

import { TextArea } from '../TextArea';

const meta = {
  component: TextArea,
  tags: ['autodocs'],
  argTypes: {
    variant: { control: 'select', options: ['default', 'glass'] },
    disabled: { control: 'boolean' },
    rows: { control: 'number' },
  },
} satisfies Meta<typeof TextArea>;
export default meta;
type Story = StoryObj<typeof TextArea>;

export const Default: Story = {
  args: { placeholder: 'Write your cover letter…', rows: 4, variant: 'default' },
};

export const Glass: Story = {
  args: { placeholder: 'Glass variant…', rows: 4, variant: 'glass' },
};

export const Disabled: Story = {
  args: { value: 'This textarea is disabled.', rows: 3, disabled: true },
};

export const AllVariants: Story = {
  render: () => (
    <div className="flex flex-col gap-4">
      <TextArea placeholder="Default variant" rows={3} variant="default" />
      <TextArea placeholder="Glass variant" rows={3} variant="glass" />
    </div>
  ),
};
