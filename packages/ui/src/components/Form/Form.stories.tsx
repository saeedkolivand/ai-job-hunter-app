import type { Meta, StoryObj } from '@storybook/react-vite';

import { Button } from '../Button';
import { Input } from '../Input';
import { Form, FormField, useForm } from './index';

const meta = {
  component: Form,
  tags: ['autodocs'],
} satisfies Meta<typeof Form>;
export default meta;
// Render-only: Form requires a live `form` instance, so the story builds it
// inside <Demo/> rather than via static args.
type Story = StoryObj;

function Demo() {
  const form = useForm<{ name: string; email: string }>({
    defaultValues: { name: '', email: '' },
  });
  return (
    <Form
      form={form}
      onSubmit={(v) => alert(JSON.stringify(v))}
      className="flex w-80 flex-col gap-4"
    >
      <FormField name="name" label="Full name" required rules={{ required: 'Required' }}>
        <Input placeholder="Ada Lovelace" />
      </FormField>
      <FormField
        name="email"
        label="Email"
        rules={{ required: 'Required', pattern: { value: /.+@.+/, message: 'Invalid email' } }}
      >
        <Input placeholder="ada@example.com" />
      </FormField>
      <Button type="submit" variant="primary">
        Save
      </Button>
    </Form>
  );
}

export const Basic: Story = {
  render: () => <Demo />,
};
