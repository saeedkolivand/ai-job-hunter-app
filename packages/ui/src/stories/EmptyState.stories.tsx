import { Briefcase, FileText, Search, Sparkles } from 'lucide-react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../components/Button';
import { EmptyState } from '../components/EmptyState';

const meta = {
  component: EmptyState,
  tags: ['autodocs'],
} satisfies Meta<typeof EmptyState>;
export default meta;
type Story = StoryObj<typeof EmptyState>;

export const NoJobs: Story = {
  args: {
    icon: Briefcase,
    title: 'No jobs found',
    description: 'Try adjusting your search filters or check back later.',
  },
};

export const NoDocuments: Story = {
  args: {
    icon: FileText,
    title: 'No documents yet',
    description: 'Upload your resume to get started.',
    action: (
      <Button variant="glass" size="sm">
        Upload resume
      </Button>
    ),
  },
};

export const SearchEmpty: Story = {
  args: {
    icon: Search,
    title: 'No results',
    description: 'Nothing matched your query. Try a different search.',
  },
};

export const AINotReady: Story = {
  args: {
    icon: Sparkles,
    title: 'AI model not ready',
    description: 'Start Ollama and pull a model in Settings → AI.',
    action: (
      <Button variant="glass" size="sm">
        Open settings
      </Button>
    ),
  },
};
