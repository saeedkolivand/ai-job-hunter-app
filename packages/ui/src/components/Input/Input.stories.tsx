import type { Meta, StoryObj } from '@storybook/react-vite';

import { Input } from '../Input';

const meta = {
  component: Input,
  tags: ['autodocs'],
  argTypes: {
    variant: { control: 'select', options: ['default', 'glass'] },
    disabled: { control: 'boolean' },
  },
} satisfies Meta<typeof Input>;
export default meta;
type Story = StoryObj<typeof Input>;

export const Glass: Story = {
  args: { variant: 'glass', placeholder: 'Type something…' },
};

export const Default: Story = {
  args: { variant: 'default', placeholder: 'Default variant' },
};

export const Password: Story = {
  args: { type: 'password', placeholder: 'Enter password', variant: 'glass' },
};

export const Disabled: Story = {
  args: { variant: 'glass', placeholder: 'Disabled', disabled: true },
};

export const WithValue: Story = {
  args: { variant: 'glass', value: 'Prefilled value', readOnly: true },
};
