import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { Form, FormField, useForm } from './index';

function Harness({ onSubmit }: { onSubmit?: (v: { name: string }) => void }) {
  const form = useForm<{ name: string }>({ defaultValues: { name: '' } });
  return (
    <Form form={form} onSubmit={onSubmit}>
      <FormField name="name" label="Name" rules={{ required: 'Required' }}>
        <input />
      </FormField>
      <button type="submit">save</button>
    </Form>
  );
}

describe('Form / FormField', () => {
  it('associates the label with the control and binds its value', () => {
    render(<Harness />);
    const input = screen.getByLabelText('Name') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'Ada' } });
    expect(input.value).toBe('Ada');
  });

  it('blocks submit and shows the validation error when a required field is empty', async () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);
    fireEvent.click(screen.getByRole('button', { name: 'save' }));
    expect(await screen.findByText('Required')).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('submits validated values', async () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Ada' } });
    fireEvent.click(screen.getByRole('button', { name: 'save' }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({ name: 'Ada' });
  });
});
