import { Globe } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';

import { SelectDropdown } from '../components/SelectDropdown';

const meta: Meta<typeof SelectDropdown> = {
  title: 'Primitives/SelectDropdown',
  component: SelectDropdown,
  tags: ['autodocs'],
};
export default meta;
type Story = StoryObj<typeof SelectDropdown>;

const OPTIONS = [
  { value: 'linkedin', label: 'LinkedIn' },
  { value: 'indeed', label: 'Indeed' },
  { value: 'stepstone', label: 'StepStone' },
  { value: 'xing', label: 'Xing' },
  { value: 'greenhouse', label: 'Greenhouse' },
];

function DefaultDemo() {
  const [value, setValue] = useState('');
  return (
    <SelectDropdown
      options={OPTIONS}
      value={value}
      onChange={setValue}
      placeholder="Select board…"
    />
  );
}

function WithIconDemo() {
  const [value, setValue] = useState('linkedin');
  return (
    <SelectDropdown
      options={OPTIONS}
      value={value}
      onChange={setValue}
      icon={<Globe size={12} />}
    />
  );
}

function SearchableDemo() {
  const [value, setValue] = useState('');
  const many = Array.from({ length: 12 }, (_, i) => ({
    value: String(i),
    label: `Option ${i + 1}`,
  }));
  return (
    <SelectDropdown
      options={many}
      value={value}
      onChange={setValue}
      placeholder="Search options…"
    />
  );
}

export const Default: Story = { render: () => <DefaultDemo /> };
export const WithIcon: Story = { render: () => <WithIconDemo /> };
export const Disabled: Story = {
  render: () => <SelectDropdown options={OPTIONS} value="linkedin" onChange={() => {}} disabled />,
};
export const Searchable: Story = { render: () => <SearchableDemo /> };
