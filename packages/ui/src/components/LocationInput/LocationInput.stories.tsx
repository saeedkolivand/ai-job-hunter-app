import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { LocationInput } from '../LocationInput';

const meta = {
  component: LocationInput,
  tags: ['autodocs'],
  argTypes: {
    disabled: { control: 'boolean' },
  },
  parameters: {
    docs: {
      description: {
        component:
          'Location autocomplete. Debounces at 300 ms and delegates the lookup to `onFetchSuggestions` — the desktop app passes the Tauri `geocode_suggest` command (a bundled offline GeoNames index, with Photon/OpenStreetMap as the fallback).',
      },
    },
  },
} satisfies Meta<typeof LocationInput>;
export default meta;
type Story = StoryObj<typeof LocationInput>;

function DefaultDemo() {
  const [value, setValue] = useState('');
  return (
    <div className="w-72">
      <LocationInput
        value={value}
        onChange={setValue}
        placeholder="e.g. Berlin, Germany or 10115"
      />
    </div>
  );
}

function PrefilledDemo() {
  const [value, setValue] = useState('Berlin, Berlin, Germany');
  return (
    <div className="w-72">
      <LocationInput value={value} onChange={setValue} />
    </div>
  );
}

export const Default: Story = { render: () => <DefaultDemo /> };
export const Prefilled: Story = { render: () => <PrefilledDemo /> };
export const Disabled: Story = {
  render: () => (
    <div className="w-72">
      <LocationInput value="Munich, Bavaria, Germany" onChange={() => {}} disabled />
    </div>
  ),
};
