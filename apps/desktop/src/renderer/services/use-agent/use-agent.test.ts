import { describe, it } from 'vitest';

import { exerciseServiceHooks } from '@/test-support';

import * as mod from './use-agent';

describe('use-agent services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});
