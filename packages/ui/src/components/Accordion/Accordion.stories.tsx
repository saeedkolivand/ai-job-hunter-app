import type { Meta, StoryObj } from '@storybook/react-vite';

import { Accordion } from './index';

const meta = {
  component: Accordion,
  tags: ['autodocs'],
} satisfies Meta<typeof Accordion>;

export default meta;
type Story = StoryObj<typeof Accordion>;

export const Default: Story = {
  args: {
    title: 'What is this component?',
    content: 'This is an accordion component that can be expanded and collapsed.',
  },
};

export const DefaultOpen: Story = {
  args: {
    title: 'This accordion is open by default',
    content: 'You can set the defaultOpen prop to true to have it open initially.',
    defaultOpen: true,
  },
};

export const HtmlContent: Story = {
  args: {
    title: 'Can contain HTML',
    content:
      '<p>This content contains <strong>HTML</strong> markup.</p><p>It uses dangerouslySetInnerHTML internally.</p>',
  },
};

export const LongContent: Story = {
  args: {
    title: 'Long content example',
    content:
      'Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.',
  },
};
