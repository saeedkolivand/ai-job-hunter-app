import { Globe } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';

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

export const Default: Story = {
  render: () => {
    const [value, setValue] = useState('');
    return (
      <SelectDropdown
        options={OPTIONS}
        value={value}
        onChange={setValue}
        placeholder="Select board…"
      />
    );
  },
};

export const WithIcon: Story = {
  render: () => {
    const [value, setValue] = useState('linkedin');
    return (
      <SelectDropdown
        options={OPTIONS}
        value={value}
        onChange={setValue}
        icon={<Globe size={12} />}
      />
    );
  },
};

export const Disabled: Story = {
  render: () => <SelectDropdown options={OPTIONS} value="linkedin" onChange={() => {}} disabled />,
};

export const Searchable: Story = {
  render: () => {
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
  },
};
