// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';

import { MC_CONFIG } from './config';
import { clearToken, isSignedIn, readToken, saveToken } from './pat';

const TOKEN = 'github_pat_11ABCDEFG_value';

afterEach(() => {
  localStorage.clear();
});

describe('PAT storage', () => {
  it('round-trips a token through localStorage under the documented key', () => {
    saveToken(TOKEN);
    expect(readToken()).toBe(TOKEN);
    expect(localStorage.getItem(MC_CONFIG.tokenKey)).toBe(TOKEN);
    expect(isSignedIn()).toBe(true);
  });

  it('trims whitespace and treats a blank value as sign-out', () => {
    saveToken(`  ${TOKEN}  `);
    expect(readToken()).toBe(TOKEN);
    saveToken('   ');
    expect(readToken()).toBe('');
    expect(isSignedIn()).toBe(false);
  });

  it('clearToken wipes the token entirely (sign-out)', () => {
    saveToken(TOKEN);
    clearToken();
    expect(localStorage.getItem(MC_CONFIG.tokenKey)).toBeNull();
    expect(readToken()).toBe('');
    expect(isSignedIn()).toBe(false);
  });

  it('reads an empty string when nothing is stored', () => {
    expect(readToken()).toBe('');
    expect(isSignedIn()).toBe(false);
  });
});
