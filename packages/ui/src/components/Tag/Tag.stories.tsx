import { Sparkles } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Tag, type TagPresetColor } from '../Tag';

const meta = {
  component: Tag,
  tags: ['autodocs'],
} satisfies Meta<typeof Tag>;
export default meta;
type Story = StoryObj<typeof Tag>;

export const Default: Story = {
  args: { children: 'Tag' },
};

const PRESETS: TagPresetColor[] = [
  'magenta',
  'red',
  'volcano',
  'orange',
  'gold',
  'lime',
  'green',
  'cyan',
  'blue',
  'geekblue',
  'purple',
];

export const Presets: Story = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      {PRESETS.map((c) => (
        <Tag key={c} color={c}>
          {c}
        </Tag>
      ))}
    </div>
  ),
};

export const Status: Story = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      <Tag color="success">success</Tag>
      <Tag color="processing">processing</Tag>
      <Tag color="error">error</Tag>
      <Tag color="warning">warning</Tag>
      <Tag color="default">default</Tag>
    </div>
  ),
};

export const CustomColor: Story = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      <Tag color="#8b5cf6">#8b5cf6</Tag>
      <Tag color="#f59e0b">#f59e0b</Tag>
    </div>
  ),
};

export const WithIcon: Story = {
  args: { color: 'blue', icon: <Sparkles size={11} />, children: 'AI' },
};

export const Closable: Story = {
  args: { color: 'red', closable: true, children: 'removable' },
};

export const Borderless: Story = {
  args: { color: 'green', bordered: false, children: 'no border' },
};

function CheckableDemo() {
  const [on, setOn] = useState<string[]>(['recruiter']);
  const toggle = (id: string) =>
    setOn((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]));
  return (
    <div className="flex flex-wrap gap-2">
      {['recruiter', 'hiring manager', 'team', 'leadership'].map((id) => (
        <Tag.CheckableTag key={id} checked={on.includes(id)} onChange={() => toggle(id)}>
          {id}
        </Tag.CheckableTag>
      ))}
    </div>
  );
}

export const Checkable: Story = {
  render: () => <CheckableDemo />,
};
