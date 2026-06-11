import { useRef, useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { LocationDropdown } from './LocationDropdown';

const meta = {
  component: LocationDropdown,
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof LocationDropdown>;
export default meta;
type Story = StoryObj<typeof LocationDropdown>;

const SUGGESTIONS = [
  { display: 'Berlin, Germany' },
  { display: 'Bern, Switzerland' },
  { display: 'Bergen, Norway' },
];

function Demo() {
  const [query, setQuery] = useState('be');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  return (
    <div style={{ height: 340 }}>
      <LocationDropdown
        open
        position={{ top: 32, left: 32, width: 320 }}
        query={query}
        setQuery={setQuery}
        suggestions={SUGGESTIONS}
        activeIndex={activeIndex}
        setActiveIndex={setActiveIndex}
        onSelect={() => {}}
        inputRef={inputRef}
        dropdownRef={dropdownRef}
        onKeyDown={() => {}}
      />
    </div>
  );
}

export const Open: Story = { render: () => <Demo /> };
