import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';

import { LocationInput } from '../components/LocationInput';

const meta: Meta<typeof LocationInput> = {
  title: 'Primitives/LocationInput',
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
};
export default meta;
type Story = StoryObj<typeof LocationInput>;

export const Default: Story = {
  render: () => {
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
  },
};

export const Prefilled: Story = {
  render: () => {
    const [value, setValue] = useState('Berlin, Berlin, Germany');
    return (
      <div className="w-72">
        <LocationInput value={value} onChange={setValue} />
      </div>
    );
  },
};

export const Disabled: Story = {
  render: () => (
    <div className="w-72">
      <LocationInput value="Munich, Bavaria, Germany" onChange={() => {}} disabled />
    </div>
  ),
};
