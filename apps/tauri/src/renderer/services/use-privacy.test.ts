import { describe, it } from 'vitest';

import { exerciseServiceHooks } from '@/test-support';

import * as mod from './use-privacy';

describe('use-privacy services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});
