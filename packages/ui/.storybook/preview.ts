import type { Preview } from '@storybook/react';

import '@ajh/ui/css';

const preview: Preview = {
  parameters: {
    backgrounds: {
      default: 'cinematic',
      values: [
        { name: 'cinematic', value: '#07060f' },
        { name: 'dark', value: '#0d0a1f' },
        { name: 'white', value: '#ffffff' },
      ],
    },
    layout: 'centered',
  },
};

export default preview;
