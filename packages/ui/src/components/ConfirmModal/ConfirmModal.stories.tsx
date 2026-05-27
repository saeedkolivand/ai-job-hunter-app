import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';
import { ConfirmModal } from '../ConfirmModal';

const meta = {
  component: ConfirmModal,
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ConfirmModal>;
export default meta;
type Story = StoryObj<typeof ConfirmModal>;

function ModalDemo({ variant }: { variant: 'danger' | 'warning' | 'info' | 'success' }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="flex h-screen items-center justify-center">
      <Button variant="glass" onClick={() => setOpen(true)}>
        Open {variant}
      </Button>
      <ConfirmModal
        open={open}
        onClose={() => setOpen(false)}
        onConfirm={() => setOpen(false)}
        variant={variant}
        title={`${variant.charAt(0).toUpperCase() + variant.slice(1)} confirmation`}
        description="This action cannot be undone. Are you sure you want to continue?"
        confirmText="Confirm"
        cancelText="Cancel"
      />
    </div>
  );
}

export const Danger: Story = { render: () => <ModalDemo variant="danger" /> };
export const Warning: Story = { render: () => <ModalDemo variant="warning" /> };
export const Info: Story = { render: () => <ModalDemo variant="info" /> };
export const Success: Story = { render: () => <ModalDemo variant="success" /> };
