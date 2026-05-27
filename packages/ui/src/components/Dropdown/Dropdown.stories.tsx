import { Cpu, Globe, Search } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Dropdown } from '../Dropdown';

const meta = {
  component: Dropdown,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof Dropdown>;
export default meta;
type Story = StoryObj<typeof Dropdown>;

const AI_MODELS = [
  { value: 'llama3.2', label: 'llama3.2', meta: '2.0 GB' },
  { value: 'mistral', label: 'mistral', meta: '4.1 GB' },
  { value: 'gemma2', label: 'gemma2', meta: '5.4 GB' },
  { value: 'phi3', label: 'phi3', meta: '2.2 GB' },
];

const BOARDS = [
  { value: 'linkedin', label: 'LinkedIn' },
  { value: 'indeed', label: 'Indeed' },
  { value: 'stepstone', label: 'StepStone' },
];

function ModelDemo() {
  const [value, setValue] = useState('llama3.2');
  return (
    <div className="w-72">
      <Dropdown options={AI_MODELS} value={value} onChange={setValue} icon={<Cpu size={13} />} />
    </div>
  );
}

function BoardDemo() {
  const [value, setValue] = useState('');
  return (
    <div className="w-72">
      <Dropdown
        options={BOARDS}
        value={value}
        onChange={setValue}
        placeholder="Select a board…"
        icon={<Globe size={13} />}
      />
    </div>
  );
}

function SearchableDemo() {
  const [value, setValue] = useState('');
  const many = Array.from({ length: 10 }, (_, i) => ({
    value: `model-${i}`,
    label: `model-option-${i}`,
    meta: `${((i + 1) * 1.2) | 0}.0 GB`,
  }));
  return (
    <div className="w-72">
      <Dropdown
        options={many}
        value={value}
        onChange={setValue}
        icon={<Search size={13} />}
        placeholder="Choose model…"
      />
    </div>
  );
}

export const WithModels: Story = { render: () => <ModelDemo /> };
export const WithBoards: Story = { render: () => <BoardDemo /> };
export const Searchable: Story = { render: () => <SearchableDemo /> };
export const Disabled: Story = {
  render: () => (
    <div className="w-72">
      <Dropdown
        options={AI_MODELS}
        value="llama3.2"
        onChange={() => {}}
        icon={<Cpu size={13} />}
        disabled
      />
    </div>
  ),
};
