import { describe, it } from 'vitest';

import { exerciseServiceHooks } from '@/test-support';

import * as mod from './use-data';

describe('use-data services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});
