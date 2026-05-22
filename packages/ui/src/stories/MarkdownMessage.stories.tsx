import type { Meta, StoryObj } from '@storybook/react-vite';

import { MarkdownMessage } from '../components/MarkdownMessage';

const meta = {
  component: MarkdownMessage,
  tags: ['autodocs'],
} satisfies Meta<typeof MarkdownMessage>;
export default meta;
type Story = StoryObj<typeof MarkdownMessage>;

export const Paragraph: Story = {
  args: { content: 'This is a plain paragraph with **bold** and *italic* and `code` inline.' },
};

export const Headings: Story = {
  args: {
    content: `# Heading 1\n## Heading 2\n### Heading 3\n\nBody text below headings.`,
  },
};

export const Lists: Story = {
  args: {
    content: `**Unordered:**\n- First item\n- Second item\n- Third item\n\n**Ordered:**\n1. Step one\n2. Step two\n3. Step three`,
  },
};

export const CodeBlock: Story = {
  args: {
    content:
      'Here is a code block:\n\n```ts\nconst greet = (name: string) => `Hello, ${name}!`;\n```',
  },
};

export const Blockquote: Story = {
  args: { content: '> This is a blockquote.\n> It can span multiple lines.' },
};

export const Full: Story = {
  args: {
    content: `# AI Analysis\n\nYour resume matches **85%** of the requirements.\n\n## Strengths\n- Strong TypeScript skills\n- 3 years React experience\n- Open source contributions\n\n## Suggestions\n1. Add quantified achievements\n2. Highlight leadership experience\n\n> Tip: Tailor your summary to each job description.\n\n\`\`\`json\n{ "score": 85, "match": "strong" }\n\`\`\``,
  },
};
