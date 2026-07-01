import { describe, it } from 'vitest';

import { exerciseServiceHooks } from '@/test-support';

import * as mod from './use-documents';

describe('use-documents services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});
