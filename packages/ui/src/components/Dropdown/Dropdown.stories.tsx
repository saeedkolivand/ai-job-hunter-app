import { Cpu, Globe, Search, Tag } from 'lucide-react';
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

function DefaultDemo() {
  const [value, setValue] = useState('');
  return (
    <div className="w-72">
      <Dropdown options={BOARDS} value={value} onChange={setValue} placeholder="Select a board…" />
    </div>
  );
}

function WithIconDemo() {
  const [value, setValue] = useState('llama3.2');
  return (
    <div className="w-72">
      <Dropdown options={AI_MODELS} value={value} onChange={setValue} icon={<Cpu size={13} />} />
    </div>
  );
}

function SearchableDemo() {
  const [value, setValue] = useState('');
  const many = Array.from({ length: 12 }, (_, i) => ({
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

function WithSectionsAndMetaDemo() {
  const [value, setValue] = useState('');
  const options = [
    { value: 'llama3.2', label: 'llama3.2', meta: '2.0 GB', section: 'Meta' },
    { value: 'llama3.1', label: 'llama3.1', meta: '4.7 GB', section: 'Meta' },
    { value: 'mistral', label: 'mistral', meta: '4.1 GB', section: 'Mistral AI' },
    { value: 'mixtral', label: 'mixtral', meta: '26 GB', section: 'Mistral AI' },
    { value: 'gemma2', label: 'gemma2', meta: '5.4 GB', section: 'Google' },
    { value: 'phi3', label: 'phi3', meta: '2.2 GB', section: 'Microsoft' },
    { value: 'qwen2', label: 'qwen2', meta: '4.4 GB', section: 'Alibaba' },
    { value: 'deepseek', label: 'deepseek-coder-v2', meta: '8.9 GB', section: 'DeepSeek' },
  ];
  return (
    <div className="w-72">
      <Dropdown
        options={options}
        value={value}
        onChange={setValue}
        icon={<Tag size={13} />}
        placeholder="Choose model…"
      />
    </div>
  );
}

function LongOptionsDemo() {
  const [value, setValue] = useState('');
  const options = [
    { value: 'a', label: 'Applied — waiting for a response from the recruiter' },
    { value: 'b', label: 'Interviewing — final round with the hiring panel' },
    { value: 'c', label: 'Offer negotiation — reviewing the compensation package' },
    { value: 'd', label: 'Rejected after technical screen — feedback requested' },
  ];
  return (
    <div className="w-64">
      <Dropdown options={options} value={value} onChange={setValue} placeholder="Select stage…" />
    </div>
  );
}

function NearViewportBottomDemo() {
  const [value, setValue] = useState('');
  return (
    <div style={{ marginTop: '90vh' }}>
      <div className="w-72">
        <Dropdown
          options={BOARDS}
          value={value}
          onChange={setValue}
          icon={<Globe size={13} />}
          placeholder="Drop-up near bottom…"
        />
      </div>
    </div>
  );
}

export const Default: Story = { render: () => <DefaultDemo /> };
export const WithIcon: Story = { render: () => <WithIconDemo /> };
export const Searchable: Story = { render: () => <SearchableDemo /> };
export const WithSectionsAndMeta: Story = { render: () => <WithSectionsAndMetaDemo /> };
export const LongOptions: Story = { render: () => <LongOptionsDemo /> };
export const NearViewportBottom: Story = { render: () => <NearViewportBottomDemo /> };
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

// Legacy named exports kept for existing story references
export const WithModels: Story = { render: () => <WithIconDemo /> };
export const WithBoards: Story = { render: () => <DefaultDemo /> };
