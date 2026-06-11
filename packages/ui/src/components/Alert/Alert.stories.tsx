import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';
import { Alert } from './Alert';

const meta = {
  component: Alert,
  tags: ['autodocs'],
  args: { message: 'This is an alert.' },
} satisfies Meta<typeof Alert>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Info: Story = { args: { type: 'info', message: 'Heads up — something to note.' } };

export const Success: Story = {
  args: { type: 'success', message: 'Saved successfully.', showIcon: true },
};

export const Warning: Story = {
  args: { type: 'warning', message: 'Careful with this action.', showIcon: true },
};

export const Danger: Story = {
  args: { type: 'error', message: 'Something went wrong.', showIcon: true },
};

export const WithDescription: Story = {
  args: {
    type: 'info',
    message: 'Update available',
    description: 'Version 1.2.0 is ready to install. Restart the app to apply it.',
  },
};

export const Closable: Story = {
  args: { type: 'success', message: 'Dismiss me with the × button.', closable: true },
};

export const WithAction: Story = {
  args: {
    type: 'warning',
    message: 'You have unsaved changes.',
    action: (
      <Button variant="glass" size="sm">
        Save
      </Button>
    ),
  },
};

export const Banner: Story = {
  args: { type: 'warning', message: 'Scheduled maintenance tonight at 22:00 UTC.', banner: true },
  parameters: { layout: 'fullscreen' },
};
