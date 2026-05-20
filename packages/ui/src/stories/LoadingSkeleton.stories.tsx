import type { Meta, StoryObj } from '@storybook/react';

import { CardSkeleton, RowSkeleton, Skeleton } from '../components/LoadingSkeleton';

const meta: Meta = {
  title: 'Feedback/LoadingSkeleton',
  tags: ['autodocs'],
};
export default meta;
type Story = StoryObj;

export const SingleLine: Story = {
  render: () => (
    <div className="w-64 space-y-2">
      <Skeleton className="h-4 w-full" />
      <Skeleton className="h-4 w-4/5" />
      <Skeleton className="h-4 w-3/5" />
    </div>
  ),
};

export const Card: Story = {
  render: () => (
    <div className="w-80">
      <CardSkeleton />
    </div>
  ),
};

export const List: Story = {
  render: () => (
    <div className="w-80 space-y-2">
      <RowSkeleton />
      <RowSkeleton />
      <RowSkeleton />
      <RowSkeleton />
    </div>
  ),
};

export const Dashboard: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-4 w-[600px]">
      <CardSkeleton />
      <CardSkeleton />
      <div className="col-span-2 space-y-2">
        <RowSkeleton />
        <RowSkeleton />
        <RowSkeleton />
      </div>
    </div>
  ),
};
