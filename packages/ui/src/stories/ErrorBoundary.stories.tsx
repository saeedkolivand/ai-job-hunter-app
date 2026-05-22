import React from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { ErrorBoundary } from '../components/ErrorBoundary';
import { ErrorState } from '../components/ErrorState';

const meta = {
  component: ErrorBoundary,
  tags: ['autodocs'],
} satisfies Meta;
export default meta;
type Story = StoryObj;

function BrokenComponent(): React.ReactElement {
  throw new Error('Simulated render error — component crashed.');
}

export const Caught: Story = {
  render: () => (
    <div className="w-96">
      <ErrorBoundary>
        <BrokenComponent />
      </ErrorBoundary>
    </div>
  ),
};

export const CustomFallback: Story = {
  render: () => (
    <div className="w-96">
      <ErrorBoundary
        fallback={(error, reset) => (
          <ErrorState title="Feature unavailable" description={error.message} onRetry={reset} />
        )}
      >
        <BrokenComponent />
      </ErrorBoundary>
    </div>
  ),
};

export const ErrorStateStandalone: Story = {
  render: () => (
    <div className="w-96 glass-card p-6">
      <ErrorState
        title="Failed to load jobs"
        description="Check your connection and try again."
        onRetry={() => alert('Retrying…')}
      />
    </div>
  ),
};
