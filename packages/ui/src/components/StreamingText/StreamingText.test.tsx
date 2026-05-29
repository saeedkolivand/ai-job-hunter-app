import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { StreamingText } from './StreamingText';

describe('StreamingText', () => {
  it('renders the full text', () => {
    render(<StreamingText text="Hello world" />);
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('shows only the tail when the text exceeds the tail length', () => {
    render(<StreamingText text="abcdefghij" tail={3} />);
    expect(screen.getByText('hij')).toBeInTheDocument();
  });

  it('renders a blinking cursor while streaming', () => {
    const { container } = render(<StreamingText text="typing" isStreaming />);
    expect(container.querySelector('[aria-hidden="true"]')).toBeTruthy();
  });
});
