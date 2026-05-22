import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { LocationInput } from '../components/LocationInput';

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
          'Location autocomplete backed by Nominatim (OpenStreetMap). Supports city names and postcodes. Debounces at 300 ms.',
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
