import { FileText } from 'lucide-react';
import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { CollapsibleFileInput } from './CollapsibleFileInput';

const meta = {
  component: CollapsibleFileInput,
  tags: ['autodocs'],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof CollapsibleFileInput>;
export default meta;
type Story = StoryObj<typeof CollapsibleFileInput>;

function Demo({ initial = '' }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return (
    <div className="w-[28rem]">
      <CollapsibleFileInput
        label="Resume"
        icon={FileText}
        value={value}
        onChange={setValue}
        onUpload={() => {}}
        placeholder="Paste your résumé text, or upload a file…"
      />
    </div>
  );
}

export const Empty: Story = { render: () => <Demo /> };

export const WithValue: Story = {
  render: () => <Demo initial={'Jane Doe\nSenior Engineer\n\nExperience\n— Built things.'} />,
};
