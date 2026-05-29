import { FileText } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { CollapsibleFileInput } from './CollapsibleFileInput';

function setup(overrides: Partial<React.ComponentProps<typeof CollapsibleFileInput>> = {}) {
  const onChange = vi.fn();
  const onUpload = vi.fn();
  render(
    <CollapsibleFileInput
      label="Resume"
      icon={FileText}
      value=""
      onChange={onChange}
      onUpload={onUpload}
      placeholder="Paste resume"
      {...overrides}
    />
  );
  return { onChange, onUpload };
}

describe('CollapsibleFileInput', () => {
  it('renders the label and an editable textarea', async () => {
    const { onChange } = setup();
    expect(screen.getByText('Resume')).toBeInTheDocument();
    await userEvent.type(screen.getByPlaceholderText('Paste resume'), 'hi');
    expect(onChange).toHaveBeenCalled();
  });

  it('calls onUpload when a file is selected', () => {
    const { onUpload } = setup();
    const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(['data'], 'resume.pdf', { type: 'application/pdf' });
    fireEvent.change(fileInput, { target: { files: [file] } });
    expect(onUpload).toHaveBeenCalledWith(file);
  });

  it('hides upload controls when disabled', () => {
    setup({ disabled: true });
    expect(document.querySelector('input[type="file"]')).toBeNull();
  });

  it('shows a checkmark when a value is present', () => {
    const { container } = render(
      <CollapsibleFileInput
        label="Cover"
        icon={FileText}
        value="filled"
        onChange={() => {}}
        onUpload={() => {}}
      />
    );
    // value present → the brand-soft icon color is applied
    expect(container.querySelector('.text-emerald-400')).toBeTruthy();
  });
});
