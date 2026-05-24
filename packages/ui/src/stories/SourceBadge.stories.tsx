import type { Meta, StoryObj } from '@storybook/react-vite';

import { SourceBadge } from '../components/SourceBadge';

const meta = {
  component: SourceBadge,
  tags: ['autodocs'],
  argTypes: {
    source: { control: 'text' },
    url: { control: 'text' },
  },
} satisfies Meta<typeof SourceBadge>;
export default meta;
type Story = StoryObj<typeof SourceBadge>;

export const Default: Story = {
  args: { source: 'linkedin' },
};

export const AllPlatforms: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-3">
      <SourceBadge source="linkedin" />
      <SourceBadge source="indeed" />
      <SourceBadge source="xing" />
      <SourceBadge source="glassdoor" />
      <SourceBadge source="greenhouse" />
      <SourceBadge source="lever" />
      <SourceBadge source="workday" />
    </div>
  ),
};

export const WithUrl: Story = {
  args: {
    source: 'linkedin',
    url: 'https://linkedin.com',
  },
};

export const UnknownPlatform: Story = {
  args: { source: 'custom-platform' },
};

export const InContext: Story = {
  render: () => (
    <div className="space-y-3 w-80">
      {[
        { source: 'linkedin', title: 'Senior Frontend Engineer' },
        { source: 'indeed', title: 'Full Stack Developer' },
        { source: 'xing', title: 'Software Engineer' },
        { source: 'glassdoor', title: 'Product Manager' },
      ].map(({ source, title }) => (
        <div key={title} className="glass-surface rounded-xl p-4">
          <div className="flex items-start justify-between gap-3">
            <div className="flex-1">
              <h3 className="text-sm font-semibold text-foreground/90">{title}</h3>
              <p className="mt-1 text-xs text-foreground/50">Company Name</p>
            </div>
            <SourceBadge source={source} />
          </div>
        </div>
      ))}
    </div>
  ),
};
