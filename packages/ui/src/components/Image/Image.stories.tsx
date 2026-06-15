import type { Meta, StoryObj } from '@storybook/react-vite';

import { Image } from './index';

const meta = {
  component: Image,
  tags: ['autodocs'],
} satisfies Meta<typeof Image>;

export default meta;
type Story = StoryObj<typeof Image>;

const SAMPLE = 'https://picsum.photos/id/1018/400/300';
const SAMPLES = [
  'https://picsum.photos/id/1018/400/300',
  'https://picsum.photos/id/1025/400/300',
  'https://picsum.photos/id/1043/400/300',
];

export const Default: Story = {
  args: { src: SAMPLE, width: 240, alt: 'A scenic landscape' },
};

export const NoPreview: Story = {
  args: { src: SAMPLE, width: 240, preview: false },
};

export const WithFallback: Story = {
  args: {
    src: 'https://example.invalid/missing.png',
    width: 240,
    fallback: SAMPLE,
  },
};

export const PreviewGroup: Story = {
  render: () => (
    <Image.PreviewGroup>
      <div style={{ display: 'flex', gap: 8 }}>
        {SAMPLES.map((src) => (
          <Image key={src} src={src} width={140} />
        ))}
      </div>
    </Image.PreviewGroup>
  ),
};
