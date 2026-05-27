import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';
import { NotificationProvider, type NotificationVariant, useNotification } from '../Notification';

const meta: Meta = {
  component: NotificationProvider,
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <NotificationProvider>
        <Story />
      </NotificationProvider>
    ),
  ],
} satisfies Meta;
export default meta;
type Story = StoryObj;

function NotificationDemo({ variant }: { variant: NotificationVariant }) {
  const notify = useNotification();
  return (
    <div className="p-8">
      <Button variant="glass" onClick={() => notify(`This is a ${variant} message.`, variant)}>
        Show {variant} notification
      </Button>
    </div>
  );
}

export const Success: Story = { render: () => <NotificationDemo variant="success" /> };
export const Error: Story = { render: () => <NotificationDemo variant="error" /> };
export const Info: Story = { render: () => <NotificationDemo variant="info" /> };
export const Warning: Story = { render: () => <NotificationDemo variant="warning" /> };
export const Stacked: Story = {
  render: () => {
    function StackDemo() {
      const notify = useNotification();
      return (
        <div className="flex gap-2 p-8">
          <Button onClick={() => notify('Operation succeeded!', 'success')}>Success</Button>
          <Button onClick={() => notify('Something went wrong.', 'error')}>Error</Button>
          <Button onClick={() => notify('Update available.', 'info')}>Info</Button>
          <Button onClick={() => notify('Check your settings.', 'warning')}>Warning</Button>
        </div>
      );
    }
    return <StackDemo />;
  },
};
