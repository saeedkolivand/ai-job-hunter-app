import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';

import { Button } from '../components/Button';
import { ModalShell } from '../components/ModalShell';

const meta: Meta<typeof ModalShell> = {
  title: 'Overlays/ModalShell',
  component: ModalShell,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
};
export default meta;
type Story = StoryObj<typeof ModalShell>;

function DefaultDemo() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Modal</Button>
      <ModalShell open={open} onClose={() => setOpen(false)}>
        <div className="p-6">
          <h2 className="mb-2 text-base font-semibold text-foreground/90">Modal Title</h2>
          <p className="mb-4 text-sm text-foreground/60">Modal content goes here.</p>
          <Button size="sm" onClick={() => setOpen(false)}>
            Close
          </Button>
        </div>
      </ModalShell>
    </>
  );
}

function WideDemo() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Wide Modal</Button>
      <ModalShell open={open} onClose={() => setOpen(false)} maxWidth="max-w-2xl">
        <div className="p-6">
          <h2 className="mb-2 text-base font-semibold text-foreground/90">Wide Modal</h2>
          <p className="text-sm text-foreground/60">This modal uses max-w-2xl.</p>
        </div>
      </ModalShell>
    </>
  );
}

function DangerDemo() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="danger" onClick={() => setOpen(true)}>
        Danger Modal
      </Button>
      <ModalShell open={open} onClose={() => setOpen(false)} borderClass="border-red-500/30">
        <div className="p-6">
          <h2 className="mb-2 text-base font-semibold text-red-400">Danger Zone</h2>
          <p className="text-sm text-foreground/60">This action cannot be undone.</p>
        </div>
      </ModalShell>
    </>
  );
}

export const Default: Story = { render: () => <DefaultDemo /> };
export const Wide: Story = { render: () => <WideDemo /> };
export const CustomBorder: Story = { render: () => <DangerDemo /> };
