import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';
import { Toast } from '../components/Toast';
import { Button } from '../components/Button';

const meta: Meta<typeof Toast> = {
  title: 'Overlays/Toast',
  component: Toast,
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
};
export default meta;
type Story = StoryObj<typeof Toast>;

function ToastDemo({ variant }: { variant: 'success' | 'error' | 'info' | 'warning' }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="p-8">
      <Button variant="glass" onClick={() => setOpen(true)}>
        Show {variant} toast
      </Button>
      <Toast
        open={open}
        onClose={() => setOpen(false)}
        variant={variant}
        message={`This is a ${variant} message. It will auto-dismiss in 5 seconds.`}
      />
    </div>
  );
}

export const Success: Story = { render: () => <ToastDemo variant="success" /> };
export const Error: Story = { render: () => <ToastDemo variant="error" /> };
export const Info: Story = { render: () => <ToastDemo variant="info" /> };
export const Warning: Story = { render: () => <ToastDemo variant="warning" /> };
