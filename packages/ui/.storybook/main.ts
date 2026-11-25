import type { StorybookConfig } from '@storybook/react-vite';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';

const config: StorybookConfig = {
  stories: ['../src/**/*.stories.@(ts|tsx)'],
  // Storybook 10 folded controls/actions/viewport/backgrounds/toolbars/measure/
  // outline and the interactions panel into core, so addon-essentials and
  // addon-interactions no longer exist as packages — they're built in.
  addons: ['@storybook/addon-vitest'],
  framework: {
    name: '@storybook/react-vite',
    options: {},
  },
  viteFinal: async (config) => {
    config.plugins = config.plugins ?? [];
    // No vite.config here, and react-vite doesn't add the React plugin itself:
    // without it Vite 8 warns on every library's "use client" directive (#1566).
    config.plugins.push(react(), tailwindcss());
    return config;
  },
};

export default config;
