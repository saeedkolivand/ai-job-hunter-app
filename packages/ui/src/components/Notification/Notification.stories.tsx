import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';
import { type NotificationPlacement, NotificationProvider, useNotification } from './Notification';

const meta = {
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
} satisfies Meta<typeof NotificationProvider>;
export default meta;
// Render-based stories (the provider only needs `children`), so keep the story
// type loose rather than requiring the provider's args.
type Story = StoryObj;

function VariantDemo() {
  const api = useNotification();
  return (
    <div className="flex flex-wrap gap-2 p-8">
      <Button
        variant="glass"
        onClick={() => api.success({ message: 'Saved', description: 'Your changes were saved.' })}
      >
        Success
      </Button>
      <Button
        variant="glass"
        onClick={() =>
          api.error({ message: 'Upload failed', description: 'Check your connection and retry.' })
        }
      >
        Error
      </Button>
      <Button
        variant="glass"
        onClick={() =>
          api.info({ message: 'Heads up', description: 'A new version is available.' })
        }
      >
        Info
      </Button>
      <Button
        variant="glass"
        onClick={() =>
          api.warning({ message: 'Careful', description: 'This action cannot be undone.' })
        }
      >
        Warning
      </Button>
    </div>
  );
}
export const Variants: Story = { render: () => <VariantDemo /> };

const PLACEMENTS: NotificationPlacement[] = [
  'topLeft',
  'top',
  'topRight',
  'bottomLeft',
  'bottom',
  'bottomRight',
];

function PlacementDemo() {
  const api = useNotification();
  return (
    <div className="flex flex-wrap gap-2 p-8">
      {PLACEMENTS.map((placement) => (
        <Button
          key={placement}
          variant="glass"
          onClick={() =>
            api.info({ message: placement, description: `Placed at ${placement}.`, placement })
          }
        >
          {placement}
        </Button>
      ))}
    </div>
  );
}
export const Placements: Story = { render: () => <PlacementDemo /> };

function ActionDemo() {
  const api = useNotification();
  return (
    <div className="p-8">
      <Button
        variant="glass"
        onClick={() =>
          api.open({
            message: 'Update ready',
            description: 'Restart to install v1.2.0.',
            variant: 'info',
            duration: 0,
            btn: (
              <Button variant="glass" size="sm">
                Restart
              </Button>
            ),
          })
        }
      >
        With action button
      </Button>
    </div>
  );
}
export const WithAction: Story = { render: () => <ActionDemo /> };

function UpdateByKeyDemo() {
  const api = useNotification();
  return (
    <div className="p-8">
      <Button
        variant="glass"
        onClick={() => {
          api.open({
            key: 'check',
            message: 'Checking for updates…',
            variant: 'info',
            duration: 0,
          });
          setTimeout(
            () =>
              api.open({
                key: 'check',
                message: "You're on the latest version.",
                variant: 'success',
              }),
            1200
          );
        }}
      >
        Update one notification by key
      </Button>
    </div>
  );
}
export const UpdateByKey: Story = { render: () => <UpdateByKeyDemo /> };
