import { Bell, Shield } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../components/Button';
import { SettingsSection } from '../components/SettingsSection';

const meta = {
  component: SettingsSection,
  tags: ['autodocs'],
} satisfies Meta<typeof SettingsSection>;
export default meta;
type Story = StoryObj<typeof SettingsSection>;

export const Default: Story = {
  args: {
    icon: Bell,
    label: 'Notifications',
    children: <p className="text-sm text-foreground/60">Notification settings go here.</p>,
  },
};

export const WithActions: Story = {
  render: () => (
    <SettingsSection icon={Shield} label="Security">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-foreground/80">Two-factor authentication</p>
          <p className="text-xs text-foreground/40">Add an extra layer of security.</p>
        </div>
        <Button size="sm" variant="glass">
          Enable
        </Button>
      </div>
    </SettingsSection>
  ),
};
