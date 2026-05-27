import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { StreamingText } from '../StreamingText';

const meta = {
  component: StreamingText,
  tags: ['autodocs'],
  argTypes: {
    isStreaming: { control: 'boolean' },
    autoScroll: { control: 'boolean' },
  },
} satisfies Meta<typeof StreamingText>;
export default meta;
type Story = StoryObj<typeof StreamingText>;

export const Static: Story = {
  args: { text: 'This is static text that has already finished streaming.', isStreaming: false },
};

export const Streaming: Story = {
  args: { text: 'Analyzing your resume', isStreaming: true },
};

function LiveDemo() {
  const full = 'The quick brown fox jumps over the lazy dog. AI is transforming job hunting.';
  const [text, setText] = useState('');
  const [streaming, setStreaming] = useState(false);

  const start = () => {
    setText('');
    setStreaming(true);
    let i = 0;
    const id = setInterval(() => {
      i++;
      setText(full.slice(0, i));
      if (i >= full.length) {
        clearInterval(id);
        setStreaming(false);
      }
    }, 40);
  };

  return (
    <div className="flex flex-col gap-3">
      <StreamingText text={text} isStreaming={streaming} />
      <button
        type="button"
        onClick={start}
        className="self-start rounded-lg bg-brand/20 px-3 py-1 text-xs text-brand-soft"
      >
        Replay
      </button>
    </div>
  );
}

export const Live: Story = { render: () => <LiveDemo /> };
