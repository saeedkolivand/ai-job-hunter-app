import type { Preview } from '@storybook/react';

import './preview.css';

const preview: Preview = {
  parameters: {
    layout: 'centered',
    // Storybook 9/10 backgrounds API: `options` (record) + `initialGlobals`.
    // The dark canvas is also enforced in preview.css so it applies in docs view
    // and even if the backgrounds addon is toggled off.
    backgrounds: {
      options: {
        cinematic: { name: 'Cinematic', value: '#07060f' },
        dark: { name: 'Dark', value: '#0d0a1f' },
        white: { name: 'White', value: '#ffffff' },
      },
    },
  },
  initialGlobals: {
    backgrounds: { value: 'cinematic' },
  },
};

export default preview;
