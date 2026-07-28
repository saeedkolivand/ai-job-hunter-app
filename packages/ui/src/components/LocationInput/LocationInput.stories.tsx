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
          'Location autocomplete. Debounces at 300 ms and delegates the lookup to the required `onFetchSuggestions` prop — the desktop app passes the Tauri `geocode_suggest` command (a bundled offline GeoNames index, with Photon/OpenStreetMap as the fallback). These stories use a static stub: the published Storybook must never issue live geocoding traffic.',
      },
    },
  },
} satisfies Meta<typeof LocationInput>;
export default meta;
type Story = StoryObj<typeof LocationInput>;

/** Static stand-in for the app's `geocode_suggest`. Deliberately offline. */
const stubSuggestions = async (query: string) =>
  [
    { display: 'Berlin, Germany', lat: 52.5244, lon: 13.4105, countryCode: 'DE' },
    { display: 'Bergen, Norway', lat: 60.393, lon: 5.3242, countryCode: 'NO' },
    { display: 'Munich, Germany', lat: 48.1374, lon: 11.5755, countryCode: 'DE' },
  ].filter((s) => s.display.toLowerCase().startsWith(query.trim().toLowerCase()));

function DefaultDemo() {
  const [value, setValue] = useState('');
  return (
    <div className="w-72">
      <LocationInput
        value={value}
        onChange={setValue}
        onFetchSuggestions={stubSuggestions}
        placeholder="e.g. Berlin, Germany"
      />
    </div>
  );
}

function PrefilledDemo() {
  const [value, setValue] = useState('Berlin, Germany');
  return (
    <div className="w-72">
      <LocationInput value={value} onChange={setValue} onFetchSuggestions={stubSuggestions} />
    </div>
  );
}

export const Default: Story = { render: () => <DefaultDemo /> };
export const Prefilled: Story = { render: () => <PrefilledDemo /> };
export const Disabled: Story = {
  render: () => (
    <div className="w-72">
      <LocationInput
        value="Munich, Germany"
        onChange={() => {}}
        onFetchSuggestions={stubSuggestions}
        disabled
      />
    </div>
  ),
};
