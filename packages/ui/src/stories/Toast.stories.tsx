import type { Meta, StoryObj } from '@storybook/react';

import { Button } from '../components/Button';
import { ToastProvider, type ToastVariant, useToast } from '../components/Toast';

const meta: Meta = {
  title: 'Overlays/Toast',
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <ToastProvider>
        <Story />
      </ToastProvider>
    ),
  ],
};
export default meta;
type Story = StoryObj;

function ToastDemo({ variant }: { variant: ToastVariant }) {
  const toast = useToast();
  return (
    <div className="p-8">
      <Button variant="glass" onClick={() => toast(`This is a ${variant} message.`, variant)}>
        Show {variant} toast
      </Button>
    </div>
  );
}

export const Success: Story = { render: () => <ToastDemo variant="success" /> };
export const Error: Story = { render: () => <ToastDemo variant="error" /> };
export const Info: Story = { render: () => <ToastDemo variant="info" /> };
export const Warning: Story = { render: () => <ToastDemo variant="warning" /> };
export const Stacked: Story = {
  render: () => {
    function StackDemo() {
      const toast = useToast();
      return (
        <div className="flex gap-2 p-8">
          <Button onClick={() => toast('Operation succeeded!', 'success')}>Success</Button>
          <Button onClick={() => toast('Something went wrong.', 'error')}>Error</Button>
          <Button onClick={() => toast('Update available.', 'info')}>Info</Button>
          <Button onClick={() => toast('Check your settings.', 'warning')}>Warning</Button>
        </div>
      );
    }
    return <StackDemo />;
  },
};
