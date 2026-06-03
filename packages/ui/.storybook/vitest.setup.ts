import { beforeAll } from 'vitest';
import { setProjectAnnotations } from '@storybook/react-vite';

import * as previewAnnotations from './preview';

// Apply the shared Storybook preview (decorators, parameters, and the Tailwind +
// @ajh/ui CSS imported via preview.ts → preview.css) to every story rendered
// under the Vitest browser runner, so story tests render exactly like the
// Storybook UI — including the design-system styles the CssCheck story asserts.
const project = setProjectAnnotations([previewAnnotations]);

beforeAll(project.beforeAll);
